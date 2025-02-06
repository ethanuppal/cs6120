use std::{collections::HashMap, fs, io, path::PathBuf};

use argh::FromArgs;
use bril_rs::{Instruction, Program, ValueOps};
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
enum Value {
    Const(String),
    Op(ValueOps, Vec<usize>),
}

#[derive(Default)]
struct ValueTable {
    /// `(value, canonical_variable)` pairs
    values: Vec<(Value, String)>,
    intern: HashMap<Value, usize>,
    variables_to_values: HashMap<String, usize>,
}

impl ValueTable {
    fn add_value_and_get_existing_variable(
        &mut self,
        value: Value,
        current_variable: &str,
    ) -> Option<String> {
        if let Some(existing_value_index) = self.intern.get(&value).copied() {
            self.variables_to_values
                .insert(current_variable.to_owned(), existing_value_index);

            None
        } else {
            self.values
                .push((value.clone(), current_variable.to_owned()));
            let new_value_index = self.values.len() - 1;
            self.intern.insert(value, new_value_index);

            self.variables_to_values
                .insert(current_variable.to_owned(), new_value_index);

            Some(self.values[new_value_index].1.clone())
        }
    }

    fn get_value(&self, variable: &str) -> usize {
        self.variables_to_values
            .get(variable)
            .copied()
            .unwrap_or_else(|| panic!("could not get value for {variable}"))
    }

    fn get_canonical_name(&self, value: usize) -> String {
        self.values[value].1.clone()
    }
}

pub fn lvn(block: &mut BasicBlock) {
    let mut table = ValueTable::default();

    for instruction in &mut block.instructions {
        *instruction = match &instruction {
            Instruction::Constant {
                dest, pos, value, ..
            } => {
                if let Some(replacement_variable) = table
                    .add_value_and_get_existing_variable(
                        Value::Const(value.to_string()),
                        dest,
                    )
                {
                    Instruction::Value {
                        dest: dest.clone(),
                        op: ValueOps::Id,
                        pos: pos.clone(),
                        args: vec![replacement_variable.clone()],
                        funcs: vec![],
                        labels: vec![],
                        op_type: None,
                    }
                } else {
                    instruction.clone()
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
            } => todo!(),
            Instruction::Value {
                args,
                dest,
                funcs,
                labels,
                op,
                pos,
                op_type,
            } => {
                let new_args = args
                    .iter()
                    .map(|arg| table.get_value(arg))
                    .collect::<Vec<_>>();
                if let Some(replacement_variable) = table
                    .add_value_and_get_existing_variable(
                        Value::Op(*op, new_args.clone()),
                        dest,
                    )
                {
                    Instruction::Value {
                        dest: dest.clone(),
                        op: ValueOps::Id,
                        pos: pos.clone(),
                        args: vec![replacement_variable.clone()],
                        funcs: vec![],
                        labels: vec![],
                        op_type: None,
                    }
                } else {
                    Instruction::Value {
                        args: new_args
                            .into_iter()
                            .map(|value| table.get_canonical_name(value))
                            .collect(),
                        dest: dest.clone(),
                        funcs: funcs.clone(),
                        labels: labels.clone(),
                        op: *op,
                        pos: pos.clone(),
                        op_type: op_type.clone(),
                    }
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
                    .map(|arg| table.get_value(&arg))
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
