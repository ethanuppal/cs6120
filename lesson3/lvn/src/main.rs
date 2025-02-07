use std::{collections::HashMap, fs, hash::Hash, io, path::PathBuf};

use argh::FromArgs;
use bril_rs::{Instruction, Literal, Program, Type, ValueOps};
use build_cfg::{print::print_cfg_as_bril_text, BasicBlock};
use snafu::{ResultExt, Whatever};

/// does LVN
#[derive(FromArgs)]
struct Opts {
    /// input Bril file: omit for stdin
    #[argh(positional)]
    input: Option<PathBuf>,
}

#[derive(PartialEq, Eq, Hash, Clone)]
enum OpArg {
    Value(usize),
    Unknown(String),
}

#[derive(Clone)]
struct NeverEqual;

impl PartialEq for NeverEqual {
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

impl Eq for NeverEqual {}

impl Hash for NeverEqual {
    fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {}
}

#[derive(PartialEq, Eq, Hash, Clone)]
enum Value {
    Float(String),
    OtherConst(String),
    Op(ValueOps, Vec<OpArg>),
    LeftAlone(NeverEqual),
}

#[derive(Default)]
struct ValueTable {
    /// `(value, canonical_variable)` pairs
    values: Vec<(Value, String)>,
    intern: HashMap<Value, usize>,
    counter: usize,
    variables_to_values: HashMap<String, usize>,
}

impl ValueTable {
    fn add_value_and_get_existing_variable(
        &mut self,
        value: Value,
        current_variable: &str,
        is_overwritten: bool,
    ) -> (String, Option<String>) {
        if let Some(existing_value_index) = self.intern.get(&value).copied() {
            self.variables_to_values
                .insert(current_variable.to_owned(), existing_value_index);
            (
                current_variable.to_owned(),
                Some(self.values[existing_value_index].1.clone()),
            )
        } else {
            let new_name = if is_overwritten {
                self.counter += 1;
                format!("{}__t{}", current_variable, self.counter)
            } else {
                current_variable.to_owned()
            };

            self.values.push((value.clone(), new_name.clone()));
            let new_value_index = self.values.len() - 1;
            self.intern.insert(value, new_value_index);

            self.variables_to_values
                .insert(current_variable.to_owned(), new_value_index);
            (new_name, None)
        }
    }

    fn get_value(&self, variable: &str) -> Option<usize> {
        self.variables_to_values.get(variable).copied()
    }

    fn get_canonical_name(&self, value: OpArg) -> String {
        match value {
            OpArg::Value(value) => self.values[value].1.clone(),
            OpArg::Unknown(other) => other,
        }
    }
}

pub fn lvn(block: &mut BasicBlock) {
    let mut table = ValueTable::default();

    let mut last_assignment = HashMap::new();

    for (i, instruction) in block.instructions.iter().enumerate() {
        if let Instruction::Constant { dest, .. }
        | Instruction::Value { dest, .. } = &instruction
        {
            last_assignment.insert(dest.clone(), i);
        }
    }

    for (i, instruction) in block.instructions.iter_mut().enumerate() {
        *instruction = match &instruction {
            Instruction::Constant {
                dest,
                pos,
                value,
                const_type,
                op,
            } => {
                let is_overwritten =
                    last_assignment.get(dest).copied().unwrap() > i;
                match table.add_value_and_get_existing_variable(
                    if matches!(const_type, Type::Float) {
                        Value::Float(value.to_string())
                    } else {
                        Value::OtherConst(value.to_string())
                    },
                    dest,
                    is_overwritten,
                ) {
                    (destination, Some(replacement_variable)) => {
                        Instruction::Value {
                            dest: destination,
                            op: ValueOps::Id,
                            pos: pos.clone(),
                            args: vec![replacement_variable.clone()],
                            funcs: vec![],
                            labels: vec![],
                            op_type: const_type.clone(),
                        }
                    }

                    (destination, None) => Instruction::Constant {
                        dest: destination,
                        op: *op,
                        pos: pos.clone(),
                        const_type: const_type.clone(),
                        value: value.clone(),
                    },
                }
            }
            Instruction::Value {
                args,
                dest,
                funcs,
                labels,
                op: ValueOps::Call,
                pos,
                op_type,
            } => {
                let is_overwritten =
                    last_assignment.get(dest).copied().unwrap() > i;
                let new_args = args
                    .iter()
                    .map(|arg| {
                        table
                            .get_value(arg)
                            .map(OpArg::Value)
                            .unwrap_or(OpArg::Unknown(arg.clone()))
                    })
                    .collect::<Vec<_>>();
                match table.add_value_and_get_existing_variable(
                    Value::LeftAlone(NeverEqual),
                    dest,
                    is_overwritten,
                ) {
                    (destination, None) => Instruction::Value {
                        args: new_args
                            .into_iter()
                            .map(|value| table.get_canonical_name(value))
                            .collect(),
                        dest: destination,
                        funcs: funcs.clone(),
                        labels: labels.clone(),
                        op: ValueOps::Call,
                        pos: pos.clone(),
                        op_type: op_type.clone(),
                    },
                    (destination, Some(replacement_variable)) => {
                        unreachable!("call values should never be recovered")
                    }
                }
            }
            Instruction::Value {
                args,
                dest,
                funcs,
                labels,
                op,
                pos,
                op_type,
            } => {
                let is_overwritten =
                    last_assignment.get(dest).copied().unwrap() > i;
                let new_args = args
                    .iter()
                    .map(|arg| {
                        table
                            .get_value(arg)
                            .map(OpArg::Value)
                            .unwrap_or(OpArg::Unknown(arg.clone()))
                    })
                    .collect::<Vec<_>>();
                match table.add_value_and_get_existing_variable(
                    Value::Op(*op, new_args.clone()),
                    dest,
                    is_overwritten,
                ) {
                    (destination, Some(replacement_variable)) => {
                        Instruction::Value {
                            dest: destination,
                            op: ValueOps::Id,
                            pos: pos.clone(),
                            args: vec![replacement_variable.clone()],
                            funcs: vec![],
                            labels: vec![],
                            op_type: op_type.clone(),
                        }
                    }
                    (destination, None) => Instruction::Value {
                        args: new_args
                            .into_iter()
                            .map(|value| table.get_canonical_name(value))
                            .collect(),
                        dest: destination,
                        funcs: funcs.clone(),
                        labels: labels.clone(),
                        op: *op,
                        pos: pos.clone(),
                        op_type: op_type.clone(),
                    },
                }
            }
            Instruction::Effect {
                args,
                funcs,
                labels,
                op,
                pos,
            } => {
                let new_args = args
                    .iter()
                    .map(|arg| {
                        table
                            .get_value(arg)
                            .map(OpArg::Value)
                            .unwrap_or(OpArg::Unknown(arg.clone()))
                    })
                    .map(|value| table.get_canonical_name(value))
                    .collect();
                Instruction::Effect {
                    args: new_args,
                    funcs: funcs.clone(),
                    labels: labels.clone(),
                    op: *op,
                    pos: pos.clone(),
                }
            }
        };
    }
}

#[snafu::report]
fn main() -> Result<(), Whatever> {
    let opts = argh::from_env::<Opts>();

    let program: Program = if let Some(path) = opts.input {
        let contents = fs::read_to_string(&path).whatever_context(format!(
            "Failed to read the contents of {}",
            path.to_string_lossy()
        ))?;
        serde_json::from_str(&contents).whatever_context(
            "Failed to parse input file as a valid Bril program",
        )?
    } else {
        serde_json::from_reader(io::stdin()).whatever_context(
            "Failed to parse standard input as a valid Bril program",
        )?
    };

    for import in program.imports {
        println!("{}", import);
    }
    for function in program.functions {
        let mut cfg = build_cfg::build_cfg(&function)
            .whatever_context("Failed to build cfg")?;

        for block in cfg.vertices.values_mut() {
            lvn(block);
        }

        print_cfg_as_bril_text(cfg);
    }

    Ok(())
}
