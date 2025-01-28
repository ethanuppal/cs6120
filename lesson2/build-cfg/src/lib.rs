// Copyright (C) 2024 Ethan Uppal. All rights reserved.
//
// Please see the LICENSE file in the project root directory.

use std::{collections::HashMap, mem};

use bril_rs::{Code, EffectOps, Function, Instruction, Position};
use slotmap::{new_key_type, Key, SecondaryMap, SlotMap};
use snafu::{whatever, OptionExt, Whatever};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Label {
    pub name: String,
}

new_key_type! { pub struct BasicBlockIdx; }

impl BasicBlockIdx {
    pub fn as_number(&self) -> u64 {
        // hacky, but it doesn't really matter here
        self.data().as_ffi()
    }
}

#[derive(Default)]
pub struct BasicBlock {
    pub is_entry: bool,
    pub label: Option<Label>,
    pub instructions: Vec<Instruction>,
    pub exit: Option<LabeledExit>,
}

pub enum LabeledExit {
    Unconditional {
        label: String,
        pos: Option<Position>,
    },
    Conditional {
        condition: String,
        if_true_label: String,
        if_false_label: String,
        pos: Option<Position>,
    },
    Return(Option<String>),
}

#[derive(Clone)]
pub enum Exit {
    Unconditional(BasicBlockIdx),
    Conditional {
        condition: String,
        if_true: BasicBlockIdx,
        if_false: BasicBlockIdx,
    },
    Return(Option<String>),
}

#[derive(Default)]
pub struct ControlFlowGraph {
    pub vertices: SlotMap<BasicBlockIdx, BasicBlock>,
    pub edges: SecondaryMap<BasicBlockIdx, Exit>,
}

#[derive(Default)]
struct ControlFlowGraphBuilder {
    cfg: ControlFlowGraph,
    current_block: BasicBlock,
    labels_to_blocks: HashMap<String, BasicBlockIdx>,
}

impl ControlFlowGraphBuilder {
    pub fn current_block_is_empty(&self) -> bool {
        self.current_block.instructions.is_empty()
    }

    pub fn add_to_current(&mut self, instruction: Instruction) {
        self.current_block.instructions.push(instruction);
    }

    pub fn set_current_label(&mut self, name: String) {
        self.current_block.label = Some(Label { name });
    }

    pub fn set_current_exit(&mut self, exit: LabeledExit) {
        self.current_block.exit = Some(exit);
    }

    pub fn mark_current_as_entry(&mut self) {
        self.current_block.is_entry = true;
    }

    pub fn finish_current_and_start_new_block(&mut self) {
        let current_label = self.current_block.label.clone();
        let current_block = mem::take(&mut self.current_block);
        let block_idx = self.cfg.vertices.insert(current_block);
        if let Some(label) = current_label {
            self.labels_to_blocks.insert(label.name.clone(), block_idx);
        }
    }

    pub fn finish(mut self) -> Result<ControlFlowGraph, Whatever> {
        for (block_idx, block) in &self.cfg.vertices {
            if let Some(exit) = &block.exit {
                match exit {
                    LabeledExit::Unconditional { label: always, pos } => {
                        let destination_index = *self
                            .labels_to_blocks
                            .get(always)
                            .whatever_context(format!(
                                "Unknown label {} referenced at {}",
                                always,
                                pos_to_string(pos.as_ref())
                            ))?;
                        self.cfg.edges.insert(
                            block_idx,
                            Exit::Unconditional(destination_index),
                        );
                    }
                    LabeledExit::Conditional {
                        condition,
                        if_true_label,
                        if_false_label,
                        pos,
                    } => {
                        let if_true_index = *self
                            .labels_to_blocks
                            .get(if_true_label)
                            .whatever_context(format!(
                                "Unknown label {} referenced at {}",
                                if_true_label,
                                pos_to_string(pos.as_ref())
                            ))?;
                        let if_false_index = *self
                            .labels_to_blocks
                            .get(if_false_label)
                            .whatever_context(format!(
                                "Unknown label {} referenced at {}",
                                if_false_label,
                                pos_to_string(pos.as_ref())
                            ))?;
                        self.cfg.edges.insert(
                            block_idx,
                            Exit::Conditional {
                                condition: condition.clone(),
                                if_true: if_true_index,
                                if_false: if_false_index,
                            },
                        );
                    }
                    LabeledExit::Return(value) => {
                        self.cfg
                            .edges
                            .insert(block_idx, Exit::Return(value.clone()));
                    }
                }
            }
        }

        Ok(self.cfg)
    }
}

fn pos_to_string(pos: Option<&Position>) -> String {
    pos.map(|pos| format!("{}:{}", pos.pos.row, pos.pos.col))
        .unwrap_or("<unknown>".into())
}

pub fn build_cfg(function: &Function) -> Result<ControlFlowGraph, Whatever> {
    let mut builder = ControlFlowGraphBuilder::default();

    builder.mark_current_as_entry();

    for instruction in &function.instrs {
        match instruction {
            Code::Label { label, .. } => {
                if !builder.current_block_is_empty() {
                    builder.finish_current_and_start_new_block();
                }
                builder.set_current_label(label.clone());
            }
            Code::Instruction(instruction) => match instruction {
                Instruction::Effect {
                    labels,
                    op: EffectOps::Jump,
                    pos,
                    ..
                } => {
                    let [destination_label] = labels.as_slice() else {
                        whatever!(
                            "Jump operation at {} should take one label",
                            pos_to_string(pos.as_ref())
                        );
                    };

                    builder.set_current_exit(LabeledExit::Unconditional {
                        label: destination_label.clone(),
                        pos: pos.clone(),
                    });

                    builder.finish_current_and_start_new_block();
                }
                Instruction::Effect {
                    args,
                    labels,
                    op: EffectOps::Branch,
                    pos,
                    ..
                } => {
                    let [condition] = args.as_slice() else {
                        whatever!(
                            "Branch operation at {} should take one condition argument",
                            pos_to_string(pos.as_ref())
                        );
                    };
                    let [if_true_label, if_false_label] = labels.as_slice()
                    else {
                        whatever!(
                            "Branch operation at {} should take two labels",
                            pos_to_string(pos.as_ref())
                        );
                    };

                    builder.set_current_exit(LabeledExit::Conditional {
                        condition: condition.clone(),
                        if_true_label: if_true_label.clone(),
                        if_false_label: if_false_label.clone(),
                        pos: pos.clone(),
                    });

                    builder.finish_current_and_start_new_block();
                }
                Instruction::Effect {
                    args,
                    labels,
                    op: EffectOps::Return,
                    pos,
                    ..
                } => {
                    let value = match args.as_slice() {
                        [] => None,
                        [value] => Some(value.clone()),
                        _ => whatever!(
                            "Branch operation at {} should take one condition argument",
                            pos_to_string(pos.as_ref())
                        )
                    };
                    if !labels.is_empty() {
                        whatever!(
                            "Return operation at {} should take no labels",
                            pos_to_string(pos.as_ref())
                        );
                    };

                    builder.set_current_exit(LabeledExit::Return(value));

                    builder.finish_current_and_start_new_block();
                }
                other => {
                    builder.add_to_current(other.clone());
                }
            },
        }
    }

    // for anything leftover
    if !builder.current_block_is_empty() {
        builder.finish_current_and_start_new_block();
    }

    builder.finish()
}
