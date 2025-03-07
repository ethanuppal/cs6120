// Copyright (C) 2024 Ethan Uppal. All rights reserved.
//
// Please see the LICENSE file in the project root directory.

use std::{collections::HashMap, mem};

use bril_rs::{
    Argument, Code, EffectOps, Function, Instruction, Position, Type,
};
use slotmap::{new_key_type, Key, SecondaryMap, SlotMap};
use snafu::{whatever, OptionExt, Whatever};

pub mod print;

pub use slotmap;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Label {
    pub name: String,
}

new_key_type! { pub struct BasicBlockIdx; }

impl BasicBlockIdx {
    pub fn as_number(&self) -> u64 {
        // hacky, but it doesn't really matter here
        self.data().as_ffi()
    }

    pub fn as_index_for_slotmap_version_1_0_7_only(&self) -> u64 {
        self.data().as_ffi() & 0xffff_ffff
    }
}

#[derive(Default)]
pub struct BasicBlock {
    pub is_entry: bool,
    pub label: Option<Label>,
    pub instructions: Vec<Instruction>,
    pub exit: LabeledExit,
}

#[derive(Default)]
pub enum LabeledExit {
    #[default]
    Fallthrough,
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
    Fallthrough(Option<BasicBlockIdx>),
    Unconditional(BasicBlockIdx),
    Conditional {
        condition: String,
        if_true: BasicBlockIdx,
        if_false: BasicBlockIdx,
    },
    Return(Option<String>),
}

#[derive(Default)]
pub struct FunctionSignature {
    pub name: String,
    pub arguments: Vec<Argument>,
    pub return_type: Option<Type>,
}

#[derive(Default)]
pub struct FunctionCfg {
    pub signature: FunctionSignature,
    pub entry: BasicBlockIdx,
    pub vertices: SlotMap<BasicBlockIdx, BasicBlock>,
    pub edges: SecondaryMap<BasicBlockIdx, Exit>,
    rev_edges: SecondaryMap<BasicBlockIdx, Vec<BasicBlockIdx>>,
}

impl FunctionCfg {
    pub fn successors(&self, block: BasicBlockIdx) -> Vec<BasicBlockIdx> {
        match &self.edges[block] {
            Exit::Fallthrough(destination_idx) => {
                destination_idx.iter().copied().collect()
            }
            Exit::Unconditional(destination_idx) => vec![*destination_idx],
            Exit::Conditional {
                condition: _,
                if_true,
                if_false,
            } => vec![*if_true, *if_false],
            Exit::Return(_) => vec![],
        }
    }

    pub fn predecessors(&self, block: BasicBlockIdx) -> &[BasicBlockIdx] {
        self.rev_edges
            .get(block)
            .map_or(&[] as &[BasicBlockIdx], |edges| edges.as_slice())
    }
}

struct FunctionCfgBuilder {
    cfg: FunctionCfg,
    /// Whether the entry point to the CFG has been initialized (in
    /// `cfg.entry`).
    entry_is_init: bool,
    current_block: BasicBlock,
    labels_to_blocks: HashMap<String, BasicBlockIdx>,
    previous_idx: Option<BasicBlockIdx>,
    input_block_order: SecondaryMap<BasicBlockIdx, BasicBlockIdx>,
}

impl FunctionCfgBuilder {
    pub fn new(
        name: String,
        arguments: Vec<Argument>,
        return_type: Option<Type>,
    ) -> Self {
        Self {
            cfg: FunctionCfg {
                signature: FunctionSignature {
                    name,
                    arguments,
                    return_type,
                },
                ..Default::default()
            },
            entry_is_init: false,
            current_block: BasicBlock::default(),
            labels_to_blocks: HashMap::default(),
            previous_idx: None,
            input_block_order: SecondaryMap::new(),
        }
    }

    pub fn add_to_current(&mut self, instruction: Instruction) {
        self.current_block.instructions.push(instruction);
    }

    pub fn set_current_label(&mut self, name: String) {
        self.current_block.label = Some(Label { name });
    }

    pub fn set_current_exit(&mut self, exit: LabeledExit) {
        assert!(!matches!(exit, LabeledExit::Fallthrough));
        self.current_block.exit = exit;
    }

    pub fn mark_current_as_entry(&mut self) {
        self.current_block.is_entry = true;
    }

    pub fn finish_current_and_start_new_block(&mut self) {
        let current_label = self.current_block.label.clone();
        let current_block = mem::take(&mut self.current_block);
        let block_idx = self.cfg.vertices.insert(current_block);

        if !self.entry_is_init {
            self.cfg.entry = block_idx;
            self.entry_is_init = true;
        }

        if let Some(previous_idx) = self.previous_idx {
            self.input_block_order.insert(previous_idx, block_idx);
        }
        self.previous_idx = Some(block_idx);

        if let Some(label) = current_label {
            self.labels_to_blocks.insert(label.name.clone(), block_idx);
        }
    }

    pub fn finish(mut self, prune: bool) -> Result<FunctionCfg, Whatever> {
        for (block_idx, block) in &self.cfg.vertices {
            match &block.exit {
                LabeledExit::Fallthrough => {
                    let after_idx_opt =
                        self.input_block_order.get(block_idx).copied();
                    let exit = after_idx_opt
                        .map(|after_idx| Exit::Fallthrough(Some(after_idx)))
                        .unwrap_or(Exit::Fallthrough(None));
                    self.cfg.edges.insert(block_idx, exit);
                    if let Some(after_idx) = after_idx_opt {
                        self.cfg
                            .rev_edges
                            .entry(after_idx)
                            .unwrap()
                            .or_default()
                            .push(block_idx);
                    }
                }
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
                    self.cfg
                        .rev_edges
                        .entry(destination_index)
                        .unwrap()
                        .or_default()
                        .push(block_idx);
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
                    self.cfg
                        .rev_edges
                        .entry(if_true_index)
                        .unwrap()
                        .or_default()
                        .push(block_idx);
                    self.cfg
                        .rev_edges
                        .entry(if_false_index)
                        .unwrap()
                        .or_default()
                        .push(block_idx);
                }
                LabeledExit::Return(value) => {
                    self.cfg
                        .edges
                        .insert(block_idx, Exit::Return(value.clone()));
                }
            }
        }

        if prune {
            self.cfg.vertices.retain(|idx, _| {
                if idx == self.cfg.entry {
                    true
                } else if let Some(rev_edges) = self.cfg.rev_edges.get(idx) {
                    !rev_edges.is_empty()
                } else {
                    false
                }
            });
            self.cfg
                .edges
                .retain(|idx, _| self.cfg.vertices.contains_key(idx));
            self.cfg
                .rev_edges
                .retain(|idx, _| self.cfg.vertices.contains_key(idx));
            for (_, rev_edges) in self.cfg.rev_edges.iter_mut() {
                rev_edges.retain(|idx| self.cfg.vertices.contains_key(*idx));
            }
        }

        Ok(self.cfg)
    }
}

fn pos_to_string(pos: Option<&Position>) -> String {
    pos.map(|pos| format!("{}:{}", pos.pos.row, pos.pos.col))
        .unwrap_or("<unknown>".into())
}

pub fn build_cfg(
    function: &Function,
    prune: bool,
) -> Result<FunctionCfg, Whatever> {
    let mut builder = FunctionCfgBuilder::new(
        function.name.clone(),
        function.args.clone(),
        function.return_type.clone(),
    );

    builder.mark_current_as_entry();

    for instruction in &function.instrs {
        match instruction {
            Code::Label { label, .. } => {
                if !builder.current_block.instructions.is_empty()
                    || builder.current_block.label.is_some()
                {
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
                    builder.add_to_current(instruction.clone());

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
                    builder.add_to_current(instruction.clone());

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
                    builder.add_to_current(instruction.clone());

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
    //if !builder.current_block_is_empty() {
    builder.finish_current_and_start_new_block();
    //}

    builder.finish(prune)
}
