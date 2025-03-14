use std::{collections::HashSet, ops::Range};

use llvm_plugin::{
    inkwell::{
        basic_block::BasicBlock,
        builder::Builder,
        context::ContextRef,
        module::{Linkage, Module},
        types::{AnyType, ArrayType, BasicType, IntMathType, PointerType},
        values::{
            BasicValue, BasicValueEnum, FunctionValue, GlobalValue,
            InstructionOpcode, InstructionValue, IntValue, PointerValue,
        },
        AddressSpace, IntPredicate,
    },
    LlvmModulePass, ModuleAnalysisManager, PassBuilder, PipelineParsing,
    PreservedAnalyses,
};
use slotmap::{new_key_type, SecondaryMap, SlotMap};

#[llvm_plugin::plugin(name = "CustomPass", version = "0.1")]
fn plugin_registrar(builder: &mut PassBuilder) {
    builder.add_module_pipeline_parsing_callback(|name, manager| {
        if name == "auto-memoize" {
            manager.add_pass(AutoMemoizePass { verbose: false });
            PipelineParsing::Parsed
        } else if name == "auto-memoize:verbose" {
            manager.add_pass(AutoMemoizePass { verbose: true });
            PipelineParsing::Parsed
        } else {
            PipelineParsing::NotParsed
        }
    });
}

const LLVM_BUILTIN_ASSUME: &str = "llvm.assume";

fn get_callee_of_known_call(instruction: InstructionValue) -> String {
    instruction
        .get_operand(1)
        .unwrap()
        .unwrap_left()
        .into_pointer_value()
        .get_name()
        .to_string_lossy()
        .to_string()
}

fn is_conservatively_pure(function: FunctionValue) -> bool {
    let mut local_allocations = HashSet::new();
    for basic_block in function.get_basic_block_iter() {
        for instruction in basic_block.get_instructions() {
            if !match instruction.get_opcode() {
                InstructionOpcode::Add
                | InstructionOpcode::AddrSpaceCast
                | InstructionOpcode::And
                | InstructionOpcode::AShr
                | InstructionOpcode::BitCast
                | InstructionOpcode::Br
                | InstructionOpcode::ExtractValue
                | InstructionOpcode::FNeg
                | InstructionOpcode::FAdd
                | InstructionOpcode::FCmp
                | InstructionOpcode::FDiv
                | InstructionOpcode::Fence
                | InstructionOpcode::FMul
                | InstructionOpcode::FPExt
                | InstructionOpcode::FPToSI
                | InstructionOpcode::FPToUI
                | InstructionOpcode::FPTrunc
                | InstructionOpcode::FRem
                | InstructionOpcode::FSub
                | InstructionOpcode::GetElementPtr
                | InstructionOpcode::ICmp
                | InstructionOpcode::IndirectBr
                | InstructionOpcode::IntToPtr
                | InstructionOpcode::Load
                | InstructionOpcode::LShr
                | InstructionOpcode::Mul
                | InstructionOpcode::Or
                | InstructionOpcode::Phi
                | InstructionOpcode::PtrToInt
                | InstructionOpcode::Return
                | InstructionOpcode::SDiv
                | InstructionOpcode::Select
                | InstructionOpcode::SExt
                | InstructionOpcode::Shl
                | InstructionOpcode::ShuffleVector
                | InstructionOpcode::SIToFP
                | InstructionOpcode::SRem
                | InstructionOpcode::Sub
                | InstructionOpcode::Switch
                | InstructionOpcode::Trunc
                | InstructionOpcode::UDiv
                | InstructionOpcode::UIToFP
                | InstructionOpcode::URem
                | InstructionOpcode::Xor
                | InstructionOpcode::ZExt => true,
                InstructionOpcode::Alloca => {
                    local_allocations.insert(instruction);
                    true
                }
                InstructionOpcode::Store => {
                    let pointer = instruction.get_operand(1).and_then(|either| either.expect_left("expected value, not block, as argument to store").as_basic_value_enum().as_instruction_value()).expect("could not get pointer argument for store");
                    local_allocations.contains(&pointer)
                }
                InstructionOpcode::Call => {
                    if get_callee_of_known_call(instruction).as_str()
                        == LLVM_BUILTIN_ASSUME
                    {
                        return true;
                    }
                    // TODO
                    false
                }
                _ => false,
            } {
                return false;
            }
        }
    }

    true
}

struct AutoMemoizePass {
    verbose: bool,
}

macro_rules! local_log {
    ($self:ident, $($format:tt)*) => {
        if $self.verbose {
            eprintln!($($format)*);
        }
    };
}

struct RelevantBlocks<'a> {
    old_entry_block: BasicBlock<'a>,
    header_block: BasicBlock<'a>,
    check_if_ready_block: BasicBlock<'a>,
    fast_path_block: BasicBlock<'a>,
}

struct MemoizationGlobals<'a> {
    value_array_type: ArrayType<'a>,
    value_array: GlobalValue<'a>,
    ready_array_type: ArrayType<'a>,
    ready_array: GlobalValue<'a>,
}

new_key_type! {
    struct ParameterKey;
}

/// The subset of the parameter domain that is memoized.
struct MemoizationBounds<'a> {
    parameters: SlotMap<ParameterKey, IntValue<'a>>,
    cached_ranges: SecondaryMap<ParameterKey, Range<u32>>,
}

// Annoyingly, these are member functions because it is more convenient to store
// configuration in the pass object than passed through parameters. To keep
// style, I'm making other helper functions take `&self` even though I'd prefer
// them to be plain functions.
impl AutoMemoizePass {
    const TYPICAL_PAGE_SIZE: u32 = 4096;

    fn construct_memoization_bounds<'a>(
        &self,
        context: ContextRef<'a>,
        input_parameters: Vec<IntValue<'a>>,
        old_entry_block: BasicBlock<'a>,
    ) -> MemoizationBounds<'a> {
        let bool_type = context.bool_type();

        // LLVM generates code like this for a __builtin_assume with a parameter
        // in a constant comparison right at the start of the function:
        //
        // %2 = alloca i32, align 4
        // store i32 %0, ptr %2, align 4
        // %3 = load i32, ptr %2, align 4
        // %4 = icmp sge i32 %3, 0
        // call void @llvm.assume(i1 %4)
        //
        // Thus we will try to infer the bounds on the variable by pattern
        // matching for this kind of code. This is likely unsustainable for
        // future LLVM versions.

        let comparison_assumptions = old_entry_block
            .get_instructions()
            .filter_map(|instruction| {
                if instruction.get_opcode() == InstructionOpcode::Call
                    && get_callee_of_known_call(instruction).as_str()
                        == LLVM_BUILTIN_ASSUME
                {
                    let assumption = instruction
                        .get_operand(0)
                        .unwrap()
                        .unwrap_left()
                        .into_int_value();
                    Some(assumption)
                } else {
                    None
                }
            })
            .filter(|assumption| assumption.get_type() == bool_type)
            .inspect(|assumption| {
                eprintln!("{assumption:?}");
            })
            .collect::<Vec<_>>();

        let mut parameters = SlotMap::<ParameterKey, _>::with_key();
        let mut cached_ranges = SecondaryMap::new();
        for input_parameter in input_parameters {
            let parameter_key = parameters.insert(input_parameter);
            cached_ranges.insert(parameter_key, 0..64);
        }
        MemoizationBounds {
            parameters,
            cached_ranges,
        }
    }

    /// Adds a static variable (that is, internal to `function`) with the given
    /// `name` and type `ty`.
    fn add_static<'a>(
        &self,
        module: &Module<'a>,
        function: FunctionValue,
        ty: impl BasicType<'a>,
        name: impl AsRef<str>,
        alignment: u32,
    ) -> GlobalValue<'a> {
        let global = module.add_global(
            ty,
            None,
            &format!(
                "{}.{}",
                function.get_name().to_string_lossy(),
                name.as_ref()
            ),
        );
        global.set_linkage(Linkage::Internal);
        global.set_alignment(alignment);
        global
    }

    fn create_memoization_globals<'a>(
        &self,
        module: &Module<'a>,
        context: ContextRef<'a>,
        function: FunctionValue,
        flattened_array_length: u32,
    ) -> MemoizationGlobals<'a> {
        let i32_type = context.i32_type();
        let value_array_type = i32_type.array_type(flattened_array_length);

        let value_array = self.add_static(
            module,
            function,
            value_array_type,
            "memo_value_array",
            Self::TYPICAL_PAGE_SIZE,
        );
        value_array.set_initializer(&i32_type.const_array(&vec![
            i32_type.const_int(0, false);
            flattened_array_length as usize
        ]));

        let bool_type = context.bool_type();
        let ready_array_type = bool_type.array_type(flattened_array_length);

        let ready_array = self.add_static(
            module,
            function,
            ready_array_type,
            "memo_ready_array",
            Self::TYPICAL_PAGE_SIZE,
        );
        ready_array.set_initializer(&bool_type.const_array(&vec![
                bool_type.const_int(0, false);
                flattened_array_length as usize
            ]));

        MemoizationGlobals {
            value_array_type,
            value_array,
            ready_array_type,
            ready_array,
        }
    }

    fn insert_memoization_basic_blocks<'a>(
        &self,
        context: ContextRef<'a>,
        function: FunctionValue<'a>,
    ) -> RelevantBlocks<'a> {
        let fast_path_block =
            context.append_basic_block(function, "memo_fast_path");
        let old_entry_block = function
            .get_first_basic_block()
            .expect("Function has no entry block");
        fast_path_block.move_before(old_entry_block).unwrap();

        let header_block = context.append_basic_block(function, "memo_header");
        header_block.move_before(fast_path_block).unwrap();

        let check_if_ready_block =
            context.append_basic_block(function, "memo_check_if_ready");
        check_if_ready_block.move_before(fast_path_block).unwrap();

        RelevantBlocks {
            old_entry_block,
            header_block,
            check_if_ready_block,
            fast_path_block,
        }
    }

    fn build_flattened_index_from_parameters<'a>(
        &self,
        context: ContextRef<'a>,
        builder: &Builder<'a>,
        bounds: &MemoizationBounds<'a>,
    ) -> IntValue<'a> {
        let i32_type = context.i32_type();

        let mut flattened_index = i32_type.const_int(0, false);

        let mut cached_ranges_iter = bounds.cached_ranges.values();
        for (i, parameter) in bounds.parameters.values().copied().enumerate() {
            if i > 0 {
                let width = i32_type.const_int(
                    cached_ranges_iter.next().unwrap().end as u64,
                    false,
                );

                flattened_index = builder
                    .build_int_mul(flattened_index, width, "flattened_index")
                    .unwrap();
            }

            flattened_index = builder
                .build_int_add(flattened_index, parameter, "flattened_index")
                .unwrap();
        }

        flattened_index
    }

    fn build_pointer_for_array_index<'a>(
        &self,
        builder: &Builder<'a>,
        array_type: ArrayType<'a>,
        array: GlobalValue<'a>,
        offset: IntValue<'a>,
        name: &str,
    ) -> PointerValue<'a> {
        unsafe {
            builder.build_gep(
                array_type,
                array.as_pointer_value(),
                &[offset],
                name,
            )
        }
        .unwrap()
    }

    fn build_checks_for_within_memoization_bounds<'a>(
        &self,
        context: ContextRef<'a>,
        builder: &Builder<'a>,
        bounds: &MemoizationBounds<'a>,
    ) -> impl Iterator<Item = IntValue<'a>> {
        let i32_type = context.i32_type();

        bounds.parameters.iter().map(move |(key, parameter)| {
            // TODO: figure out how to make this work without fixing
            // "signed"
            let lower_bound_check = builder
                .build_int_compare(
                    IntPredicate::SGE,
                    *parameter,
                    i32_type.const_int(
                        bounds.cached_ranges[key].start as u64,
                        false,
                    ),
                    "",
                )
                .unwrap();
            let upper_bound_check = builder
                .build_int_compare(
                    IntPredicate::SLT,
                    *parameter,
                    i32_type
                        .const_int(bounds.cached_ranges[key].end as u64, false),
                    "",
                )
                .unwrap();
            builder
                .build_and(lower_bound_check, upper_bound_check, "")
                .unwrap()
        })
    }

    fn maybe_memoize<'a>(
        &self,
        module: &Module<'a>,
        context: ContextRef<'a>,
        builder: &Builder,
        function: FunctionValue,
    ) {
        let i32_type = context.i32_type();
        let bool_type = context.bool_type();

        let Some(int_parameters) = function
            .get_params()
            .into_iter()
            .map(|parameter| match parameter {
                BasicValueEnum::IntValue(int_value) => {
                    if int_value.get_type() == i32_type {
                        Some(int_value)
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
        else {
            local_log!(
                self,
                "[auto-memoize] Skipping memoization for {:?} because it does not only have integer (that is, LLVM i32) parameters",
                function.get_name()
            );
            return;
        };
        if int_parameters.len() > 3 {
            local_log!(
                self,
                "[auto-memoize] Skipping memoization for {:?} because it has more than 3 integer parameters",
                function.get_name()
            );
            return;
        }

        local_log!(self, "[auto-memoize] Memoizing {:?}", function.get_name());

        let RelevantBlocks {
            old_entry_block,
            header_block,
            check_if_ready_block,
            fast_path_block,
        } = self.insert_memoization_basic_blocks(context, function);

        let bounds = self.construct_memoization_bounds(
            context,
            int_parameters,
            old_entry_block,
        );

        let flattened_array_length: u32 =
            bounds.cached_ranges.values().map(|range| range.end).sum();

        let MemoizationGlobals {
            value_array_type,
            value_array,
            ready_array_type,
            ready_array,
        } = self.create_memoization_globals(
            module,
            context,
            function,
            flattened_array_length,
        );

        builder.position_at_end(header_block);

        let flattened_index = self
            .build_flattened_index_from_parameters(context, builder, &bounds);

        let memoization_bounds_checks = self
            .build_checks_for_within_memoization_bounds(
                context, builder, &bounds,
            );

        let ready_pointer = self.build_pointer_for_array_index(
            builder,
            ready_array_type,
            ready_array,
            flattened_index,
            "ready_pointer",
        );

        let mut parameters_in_bounds = bool_type.const_zero();
        for condition in memoization_bounds_checks {
            parameters_in_bounds = builder
                .build_and(parameters_in_bounds, condition, "can_memoize")
                .unwrap();
        }

        let _ = builder
            .build_conditional_branch(
                parameters_in_bounds,
                check_if_ready_block,
                old_entry_block,
            )
            .unwrap();

        builder.position_at_end(check_if_ready_block);

        let is_ready = builder
            .build_load(context.bool_type(), ready_pointer, "is_ready")
            .unwrap()
            .into_int_value();
        let can_memoize = builder
            .build_and(parameters_in_bounds, is_ready, "can_memoize")
            .unwrap();

        let _ = builder
            .build_conditional_branch(
                can_memoize,
                fast_path_block,
                old_entry_block,
            )
            .unwrap();

        builder.position_at_end(fast_path_block);

        let value_pointer = self.build_pointer_for_array_index(
            builder,
            value_array_type,
            value_array,
            flattened_index,
            "value_pointer",
        );
        let cached_value = builder
            .build_load(i32_type, value_pointer, "memo_value")
            .unwrap();

        builder.build_return(Some(&cached_value)).unwrap();
    }
}

impl LlvmModulePass for AutoMemoizePass {
    fn run_pass(
        &self,
        module: &mut Module,
        _manager: &ModuleAnalysisManager,
    ) -> PreservedAnalyses {
        let mut preserved_analyses = PreservedAnalyses::All;

        let context = module.get_context();
        let builder = context.create_builder();

        for function in module.get_functions() {
            local_log!(
                self,
                "[auto-memoize] Visiting function {:?}",
                function.get_name()
            );

            let scuffed_is_defined = function.count_basic_blocks() > 0;
            if scuffed_is_defined && is_conservatively_pure(function) {
                local_log!(
                    self,
                    "[auto-memoize] Function {:?} is pure",
                    function.get_name()
                );
                self.maybe_memoize(&module, context, &builder, function);

                preserved_analyses = PreservedAnalyses::None;
            }
        }

        preserved_analyses
    }
}
