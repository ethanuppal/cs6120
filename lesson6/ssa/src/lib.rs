use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use bril_rs::{EffectOps, Instruction, Type, ValueOps};
use build_cfg::{slotmap::SecondaryMap, BasicBlockIdx, FunctionCfg};
use snafu::{whatever, OptionExt, Whatever};

pub struct DefinitionSites(pub BTreeMap<String, (Type, Vec<BasicBlockIdx>)>);

/// The basic blocks a given variable was assigned in along with its type.
pub fn compute_definition_sites(cfg: &FunctionCfg) -> DefinitionSites {
    DefinitionSites(
        cfg.vertices
            .iter()
            .map(|(block_idx, block)| {
                block.instructions.iter().filter_map(move |instruction| {
                    match instruction {
                        bril_rs::Instruction::Constant {
                            dest: definition,
                            const_type: ty,
                            ..
                        }
                        | bril_rs::Instruction::Value {
                            dest: definition,
                            op_type: ty,
                            ..
                        } => Some((definition.clone(), ty.clone(), block_idx)),
                        _ => None,
                    }
                })
            })
            .fold(BTreeMap::new(), |mut definitions, some_definitions| {
                for (definition, ty, block_idx) in some_definitions {
                    definitions
                        .entry(definition)
                        .or_insert_with(|| (ty, Vec::default()))
                        .1
                        .push(block_idx);
                }
                definitions
            }),
    )
}

pub struct PhiInsertionPoints(
    pub BTreeMap<String, (Type, BTreeSet<BasicBlockIdx>)>,
);

/// For each variable, its type and a set of basic blocks that need a Phi node
/// for it.
pub fn determine_phi_insertion_points(
    definition_sites: DefinitionSites,
    dominance_frontiers: SecondaryMap<BasicBlockIdx, HashSet<BasicBlockIdx>>,
) -> PhiInsertionPoints {
    let mut insertion_points = BTreeMap::new();

    for (variable, (ty, mut definition_blocks)) in definition_sites.0 {
        while let Some(block_idx) = definition_blocks.pop() {
            for frontier_idx in &dominance_frontiers[block_idx] {
                let ty = ty.clone();
                insertion_points
                    .entry(variable.clone())
                    .or_insert_with(move || {
                        (ty, BTreeSet::<BasicBlockIdx>::default())
                    })
                    .1
                    .insert(*frontier_idx);
            }
        }
    }

    PhiInsertionPoints(insertion_points)
}

pub fn insert_phis(
    cfg: &mut FunctionCfg,
    phi_insertion_points: PhiInsertionPoints,
) {
    let mut phis_to_insert = SecondaryMap::new();
    for (variable, (ty, places_to_insert)) in phi_insertion_points.0 {
        for place_to_insert in places_to_insert {
            phis_to_insert
                .entry(place_to_insert)
                .unwrap()
                .or_insert_with(Vec::default)
                .push(Instruction::Value {
                    args: vec![],
                    dest: variable.clone(),
                    funcs: vec![],
                    labels: vec![],
                    op: ValueOps::Get,
                    pos: None,
                    op_type: ty.clone(),
                });
        }
    }
    for (block_idx, phis) in phis_to_insert {
        cfg.vertices[block_idx].instructions.splice(0..0, phis);
    }
}

#[derive(Default)]
pub struct DominatingDefinitionsStacks {
    /// A stack for each definition that dominates the current block; immediate
    /// dominators will overtake prior ones on the stack. Each stack entry
    /// consists of the most recently-dominating block defining a variable
    /// and the local numbering of the last definition.
    inner: HashMap<String, Vec<(BasicBlockIdx, usize)>>,
}

impl DominatingDefinitionsStacks {
    pub fn lookup_latest_dominator_of(
        &self,
        definition: &str,
    ) -> Option<(BasicBlockIdx, usize)> {
        self.inner
            .get(definition)
            .and_then(|stack| stack.last())
            .copied()
    }

    pub fn with_new_definitions<T>(
        &mut self,
        local_renamer: LocalRenamer,
        then: impl FnOnce(&mut Self) -> T,
    ) -> T {
        for (new_definition, number) in local_renamer.latest_definitions() {
            self.inner
                .entry(new_definition.clone())
                .or_default()
                .push((local_renamer.current_idx, number));
        }
        let result = then(self);
        for (new_definition, _) in local_renamer.latest_definitions() {
            if let Some(stack) = self.inner.get_mut(new_definition) {
                let popped_idx = stack.pop().map(|(idx, _)| idx);
                assert_eq!(
                    popped_idx.expect("We just pushed to this key"),
                    local_renamer.current_idx
                );
            }
        }
        result
    }
}

pub struct LocalRenamer {
    current_idx: BasicBlockIdx,
    current_id: u64,
    numbering: HashMap<String, usize>,
}

impl LocalRenamer {
    pub fn new(current_idx: BasicBlockIdx) -> Self {
        Self {
            current_idx,
            current_id: current_idx.as_index_for_slotmap_version_1_0_7_only(),
            numbering: HashMap::default(),
        }
    }

    pub fn rewrite_destination(&mut self, name: String) -> String {
        let entry = self.numbering.entry(name.clone()).or_insert(0);
        *entry += 1;
        format!("{}.{}.{}", name, self.current_id, *entry)
    }

    pub fn rewrite_argument(
        &self,
        dominating_definitions_stacks: &DominatingDefinitionsStacks,
        name: &str,
    ) -> Option<String> {
        if let Some(current_number) = self.numbering.get(name).copied() {
            Some(format!("{name}.{}.{current_number}", self.current_id))
        } else if let Some((defining_dominator, previous_number)) =
            dominating_definitions_stacks.lookup_latest_dominator_of(name)
        {
            Some(format!(
                "{name}.{}.{previous_number}",
                defining_dominator.as_index_for_slotmap_version_1_0_7_only()
            ))
        } else {
            //todo!("LocalRenamer::rewrite_argument: Could not rewrite `{name}`
            // since it was not defined locally or from a dominator. Don't know
            // what to do here")
            // lol a variable is undefined if its definitions do not dominate
            // its uses right?
            None
        }
    }

    pub fn rewrite_arguments(
        &mut self,
        dominating_definitions_stacks: &DominatingDefinitionsStacks,
        names: impl IntoIterator<Item = String>,
    ) -> Vec<String> {
        names
            .into_iter()
            .map(|name| {
                self.rewrite_argument(dominating_definitions_stacks, &name)
                    .expect(
                        "Definitions of arguments did not dominate their uses",
                    )
            })
            .collect()
    }

    /// This function is very cheap.
    pub fn latest_definitions(
        &self,
    ) -> impl Iterator<Item = (&String, usize)> + '_ {
        self.numbering
            .iter()
            .map(|(definition, current_number)| (definition, *current_number))
    }

    //
    ///// Whether `name` refers to a function parameter or whether it is
    ///// currently defined or defined in a dominator.
    //fn resolves_to_parameter(
    //    &self,
    //    dominating_definitions_stacks: &DominatingDefinitionsStacks
    //    name: &str,
    //) -> bool {
    //}
}

pub fn rename_and_insert_upsilons(
    cfg: &mut FunctionCfg,
    block_idx: BasicBlockIdx,
    dominance_tree: &SecondaryMap<BasicBlockIdx, HashSet<BasicBlockIdx>>,
    dominating_definitions_stacks: &mut DominatingDefinitionsStacks,
    undefined_names: &mut BTreeMap<String, Type>,
) {
    let mut local_renamer = LocalRenamer::new(block_idx);

    for instruction in &mut cfg.vertices[block_idx].instructions {
        *instruction = match instruction.clone() {
            Instruction::Constant {
                dest,
                op,
                pos,
                const_type,
                value,
            } => Instruction::Constant {
                dest: local_renamer.rewrite_destination(dest),
                op,
                pos,
                const_type,
                value,
            },
            Instruction::Value {
                args,
                dest,
                funcs,
                labels,
                op,
                pos,
                op_type,
            } => Instruction::Value {
                args: local_renamer
                    .rewrite_arguments(dominating_definitions_stacks, args),
                dest: local_renamer.rewrite_destination(dest),
                funcs,
                labels,
                op,
                pos,
                op_type,
            },
            Instruction::Effect {
                args,
                funcs,
                labels,
                op,
                pos,
            } if !matches!(op, EffectOps::Set) => Instruction::Effect {
                args: local_renamer
                    .rewrite_arguments(dominating_definitions_stacks, args),
                funcs,
                labels,
                op,
                pos,
            },
            other => other,
        };
    }

    let mut locally_required_sets = BTreeMap::new();
    for successor_idx in cfg.successors(block_idx) {
        let successor = &cfg.vertices[successor_idx];

        #[derive(Debug)]
        struct Phi<'a>(&'a String, &'a Type);

        for phi_node in
            successor.instructions.iter().filter_map(|instruction| {
                if let Instruction::Value {
                    dest,
                    op: ValueOps::Get,
                    op_type,
                    ..
                } = instruction
                {
                    Some(Phi(dest, op_type))
                } else {
                    None
                }
            })
        {
            // TODO: I really hate this. It shouldn't be dependent on how
            // variables are named.
            let original_name = phi_node
                .0
                .split_once(".")
                .map(|(first, _)| first)
                .unwrap_or(phi_node.0);
            let phi_name = format!(
                "{original_name}.{}.1",
                successor_idx.as_index_for_slotmap_version_1_0_7_only()
            );
            locally_required_sets.insert(
                phi_name,
                (original_name.to_string(), phi_node.1.to_owned()),
            );
        }
    }
    let set_insertion_point = cfg.vertices[block_idx].index_before_exit();
    cfg.vertices[block_idx].instructions.splice(
        set_insertion_point..set_insertion_point,
        locally_required_sets.into_iter().map(
            |(phi_name, (original_name, phi_type))| {
                let current_name = local_renamer
                    .rewrite_argument(
                        dominating_definitions_stacks,
                        &original_name,
                    )
                    .unwrap_or_else(|| {
                        let undefined_name = format!("{original_name}.undef");
                        undefined_names
                            .insert(undefined_name.clone(), phi_type);
                        undefined_name
                    });
                Instruction::Effect {
                    args: vec![phi_name, current_name],
                    funcs: vec![],
                    labels: vec![],
                    op: EffectOps::Set,
                    pos: None,
                }
            },
        ),
    );

    dominating_definitions_stacks.with_new_definitions(
        local_renamer,
        |dominating_definitions_stacks| {
            for imm_idx in &dominance_tree[block_idx] {
                rename_and_insert_upsilons(
                    cfg,
                    *imm_idx,
                    dominance_tree,
                    dominating_definitions_stacks,
                    undefined_names,
                );
            }
        },
    )
}

pub fn insert_undefined_names_at_entry(
    cfg: &mut FunctionCfg,
    mut undefined_names: BTreeMap<String, Type>,
) {
    for block in cfg.vertices.values_mut() {
        for instruction in &block.instructions {
            if let Instruction::Constant { dest, .. }
            | Instruction::Value { dest, .. } = &instruction
            {
                undefined_names.remove(dest);
            }
        }
    }
    for (other, ty) in undefined_names {
        cfg.vertices[cfg.entry].instructions.insert(
            0,
            Instruction::Value {
                args: vec![],
                dest: other,
                funcs: vec![],
                labels: vec![],
                op: ValueOps::Undef,
                pos: None,
                op_type: ty,
            },
        );
    }
}

pub fn is_ssa(cfg: &FunctionCfg) -> bool {
    let mut definitions = HashSet::new();
    for block in cfg.vertices.values() {
        for instruction in &block.instructions {
            if let Instruction::Constant { dest, .. }
            | Instruction::Value { dest, .. } = &instruction
            {
                if !definitions.insert(dest) {
                    return false;
                }
            }
        }
    }
    true
}

pub fn from_ssa(cfg: &mut FunctionCfg) -> Result<(), Whatever> {
    if !is_ssa(cfg) {
        whatever!("Input was not in SSA already");
    }

    let mut set_operation_types = HashMap::new();
    for block in cfg.vertices.values() {
        for instruction in &block.instructions {
            if let Instruction::Value {
                dest,
                op: ValueOps::Get,
                op_type,
                ..
            } = instruction
            {
                set_operation_types.insert(dest.clone(), op_type.clone());
            }
        }
    }

    for block in cfg.vertices.values_mut() {
        block.instructions.retain(|instruction| {
            !matches!(
                instruction,
                Instruction::Value {
                    op: ValueOps::Get,
                    ..
                },
            )
        });
        for instruction in &mut block.instructions {
            if let Some(replacement) = if let Instruction::Effect {
                args,
                op: EffectOps::Set,
                ..
            } = instruction
            {
                assert!(
                    args.len() == 2,
                    "EffectOps::Set should have two arguments"
                );
                Some(Instruction::Value {
                    args: vec![args[1].clone()],
                    dest: args[0].clone(),
                    funcs: vec![],
                    labels: vec![],
                    op: ValueOps::Id,
                    pos: None,
                    op_type: set_operation_types.get(&args[0]).whatever_context("The corresponding `get` instruction does not exist or did not specify a type")?.clone(),
                })
            } else {
                None
            } {
                *instruction = replacement;
            }
        }
    }

    Ok(())
}
