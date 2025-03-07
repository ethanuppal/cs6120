use bril_rs::{Instruction, Literal};

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub enum InstructionValue {
    Argument,
    /// literal string + is float
    Constant(String, bool),
    Op(String, Vec<String>, Vec<String>, Vec<String>),
}

pub trait InstructionExt {
    fn kill(&self) -> Option<&String>;

    fn gen_set(&self) -> &[String];

    fn value(&self) -> Option<InstructionValue>;
}

impl InstructionExt for Instruction {
    fn kill(&self) -> Option<&String> {
        match self {
            Instruction::Constant { dest, .. }
            | Instruction::Value { dest, .. } => Some(dest),
            Instruction::Effect { .. } => None,
        }
    }

    fn gen_set(&self) -> &[String] {
        match self {
            Instruction::Value { args, .. }
            | Instruction::Effect { args, .. } => args,
            Instruction::Constant { .. } => &[],
        }
    }

    fn value(&self) -> Option<InstructionValue> {
        match self {
            Instruction::Constant { value, .. } => {
                Some(InstructionValue::Constant(
                    value.to_string(),
                    matches!(value, Literal::Float(_)),
                ))
            }
            Instruction::Value {
                op,
                args,
                funcs,
                labels,
                ..
            } => Some(InstructionValue::Op(
                op.to_string(),
                args.clone(),
                funcs.clone(),
                labels.clone(),
            )),
            Instruction::Effect { .. } => None,
        }
    }
}
