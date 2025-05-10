// Copyright (C) 2024 Ethan Uppal. All rights reserved.
//
// Please see the LICENSE file in the project root directory.

use std::{collections::HashMap, mem};

use bril_rs::{
    Argument, Code, EffectOps, Function, Instruction, Position, Type,
};
use slotmap::{Key, SecondaryMap, SlotMap, new_key_type};
use snafu::{OptionExt, Whatever, whatever};

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

#[derive(Debug, Default)]
pub struct BasicBlock {
    pub is_entry: bool,
    pub label: Option<Label>,
    pub instructions: Vec<Instruction>,
    pub exit: LabeledExit,
}

impl BasicBlock {
    //pub fn last_assignments(&self) -> HashMap<String, usize> {
    //    let mut last_assignments = HashMap::new();
    //    for (i, instruction) in self.instructions.iter().enumerate() {
    //        if let Instruction::Constant { dest, .. }
    //        | Instruction::Value { dest, .. } = &instruction
    //        {
    //            last_assignments.insert(dest.clone(), i);
    //        }
    //    }
    //    last_assignments
    //}

    /// If there is no final jump, branch, or return instruction, the end index
    /// of the instructions, otherwise, one before that end index.
    pub fn index_before_exit(&self) -> usize {
        if matches!(self.exit, LabeledExit::Fallthrough) {
            self.instructions.len()
        } else {
            // there is a final jump, branch, or return instruction
            self.instructions.len() - 1
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
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
    pub rev_edges: SecondaryMap<BasicBlockIdx, Vec<BasicBlockIdx>>,
}

impl FunctionCfg {
    pub fn add_block(&mut self, block: BasicBlock) -> BasicBlockIdx {
        self.vertices.insert(block)
    }

    /// Replaces a `(start_block, old_end_block)` edge with `(start_block,
    /// end_block)` edge.
    ///
    /// Requires: there are no fallthrough edges.
    pub fn reorient_edge(
        &mut self,
        start_block: BasicBlockIdx,
        old_end_block: BasicBlockIdx,
        end_block: BasicBlockIdx,
    ) {
        if matches!(self.edges[start_block], Exit::Return(_)) {
            return;
        }

        let Some(end_label) = self.vertices[end_block].label.clone() else {
            panic!("Destination block does not have a label");
        };

        self.rev_edges[old_end_block]
            .retain(|predecessor| *predecessor != start_block);

        let new_end_rev_edges =
            self.rev_edges.entry(end_block).unwrap().or_default();
        if !new_end_rev_edges.contains(&start_block) {
            new_end_rev_edges.push(start_block);
        }

        match &self.vertices[start_block].exit {
            LabeledExit::Unconditional { .. } => {
                *self.vertices[start_block]
                    .instructions
                    .last_mut()
                    .expect("Call FunctionCfg::make_fallthroughs_explicit") =
                    Instruction::Effect {
                        args: vec![],
                        funcs: vec![],
                        labels: vec![end_label.name],
                        op: EffectOps::Jump,
                        pos: None,
                    };
                self.edges[start_block] = Exit::Unconditional(end_block);
            }
            LabeledExit::Conditional {
                if_true_label,
                if_false_label,
                ..
            } => {
                let Exit::Conditional {
                    if_true,
                    if_false,
                    condition,
                } = self.edges[start_block].clone()
                else {
                    unreachable!(
                        "LabeledExit should always correspond with Exit"
                    );
                };
                let (
                    new_if_true_label,
                    new_if_false_label,
                    new_if_true,
                    new_if_false,
                ) = if old_end_block == if_true {
                    (
                        end_label.name,
                        if_false_label.clone(),
                        end_block,
                        if_false,
                    )
                } else {
                    (if_true_label.clone(), end_label.name, if_true, end_block)
                };
                *self.vertices[start_block]
                    .instructions
                    .last_mut()
                    .expect("Call FunctionCfg::make_fallthroughs_explicit") =
                    Instruction::Effect {
                        args: vec![condition.clone()],
                        funcs: vec![],
                        labels: vec![new_if_true_label, new_if_false_label],
                        op: EffectOps::Branch,
                        pos: None,
                    };
                self.edges[start_block] = Exit::Conditional {
                    condition,
                    if_true: new_if_true,
                    if_false: new_if_false,
                }
            }
            _ => {}
        }
    }

    /// Overwrites an existing unconditional edge with the new one.
    ///
    /// Requires: there are no fallthrough edges.
    pub fn set_unconditional_edge(
        &mut self,
        start_block: BasicBlockIdx,
        end_block: BasicBlockIdx,
    ) {
        if !matches!(
            self.vertices[start_block].exit,
            LabeledExit::Fallthrough | LabeledExit::Unconditional { .. }
        ) && self.edges.contains_key(start_block)
        {
            panic!(
                "Was not already unconditional or absent: {:?}",
                self.vertices[start_block]
            );
        }

        let Some(end_label) = self.vertices[end_block].label.clone() else {
            panic!("Destination block does not have a label");
        };

        match &self.vertices[start_block].exit {
            LabeledExit::Fallthrough => {
                self.vertices[start_block].instructions.push(
                    Instruction::Effect {
                        args: vec![],
                        funcs: vec![],
                        labels: vec![end_label.name.clone()],
                        op: EffectOps::Jump,
                        pos: None,
                    },
                );
            }
            LabeledExit::Unconditional { .. } => {
                *self.vertices[start_block].instructions.last_mut().expect("branching LabeledExit implies existence of corresponding instruction at end of block") =
                    Instruction::Effect {
                        args: vec![],
                        funcs: vec![],
                        labels: vec![end_label.name.clone()],
                        op: EffectOps::Jump,
                        pos: None,
                    };
            }
            _ => unreachable!(),
        }

        self.vertices[start_block].exit = LabeledExit::Unconditional {
            label: end_label.name,
            pos: None,
        };

        self.edges
            .insert(start_block, Exit::Unconditional(end_block));
        if !self.rev_edges.contains_key(end_block) {
            self.rev_edges.insert(end_block, vec![]);
        }
        if !self.rev_edges[end_block].contains(&start_block) {
            self.rev_edges[end_block].push(start_block);
        }
    }

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

    /// Replaces al fallthroughs with unconditional jumps or returns.
    pub fn make_fallthroughs_explicit(&mut self) {
        for block_idx in self.vertices.keys().collect::<Vec<_>>() {
            if let Exit::Fallthrough(destination) = self.edges[block_idx] {
                if let Some(destination) = destination {
                    let Some(label) = self.vertices[destination].label.clone()
                    else {
                        unreachable!(
                            "cannot fallthrough to block without label since only the entry has no label"
                        );
                    };
                    self.vertices[block_idx].exit =
                        LabeledExit::Unconditional {
                            label: label.name.clone(),
                            pos: None,
                        };
                    self.edges[block_idx] = Exit::Unconditional(destination);
                    self.vertices[block_idx].instructions.push(
                        Instruction::Effect {
                            args: vec![],
                            funcs: vec![],
                            labels: vec![label.name],
                            op: EffectOps::Jump,
                            pos: None,
                        },
                    );
                } else {
                    self.vertices[block_idx].exit = LabeledExit::Return(None);
                    self.edges[block_idx] = Exit::Return(None);
                    self.vertices[block_idx].instructions.push(
                        Instruction::Effect {
                            args: vec![],
                            funcs: vec![],
                            labels: vec![],
                            op: EffectOps::Return,
                            pos: None,
                        },
                    );
                }
            }
        }

        self.assert_no_fallthroughs();
    }

    /// Asserts that this CFG has no fallthrough edges.
    pub fn assert_no_fallthroughs(&self) {
        for block_idx in self.vertices.keys() {
            assert!(!matches!(
                self.vertices[block_idx].exit,
                LabeledExit::Fallthrough
            ));
            if self.edges.contains_key(block_idx) {
                assert!(!matches!(self.edges[block_idx], Exit::Fallthrough(_)));
            }
        }
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
                        ),
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
