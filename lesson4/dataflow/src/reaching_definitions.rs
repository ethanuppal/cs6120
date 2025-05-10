use std::collections::{HashSet, VecDeque};

use bril_util::{InstructionExt, InstructionValue};
use build_cfg::{
    BasicBlock, BasicBlockIdx, FunctionCfg, slotmap::SecondaryMap,
};

use crate::{Direction, solve_dataflow};

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct Definition(pub String, pub InstructionValue, pub BasicBlockIdx);

/// Whether `definition` is reachable backward from `block`.
pub fn definition_is_reachable(
    cfg: &FunctionCfg,
    block: BasicBlockIdx,
    definition: &Definition,
) -> bool {
    if matches!(definition.1, InstructionValue::Argument) {
        return true;
    }

    let mut bfs = VecDeque::new();
    let mut visited = HashSet::new();
    bfs.push_back(block);
    while let Some(current) = bfs.pop_front() {
        if cfg.vertices[current]
            .instructions
            .iter()
            .rev()
            .any(|instruction| {
                if let (Some(kill), Some(value)) =
                    (instruction.kill(), instruction.value())
                {
                    definition == &Definition(kill.clone(), value, current)
                } else {
                    false
                }
            })
        {
            return true;
        }
        for predecessor in cfg.predecessors(current) {
            if !visited.contains(predecessor) {
                bfs.push_back(*predecessor);
            }
        }
        visited.insert(current);
    }

    false
}

pub fn compute_reaching_definitions(
    cfg: &FunctionCfg,
) -> SecondaryMap<BasicBlockIdx, HashSet<Definition>> {
    fn transfer(
        block: &BasicBlock,
        block_idx: BasicBlockIdx,
        mut inputs: HashSet<Definition>,
    ) -> HashSet<Definition> {
        for instruction in &block.instructions {
            if let Some(kill) = instruction.kill() {
                inputs.retain(|input| &input.0 != kill);
                inputs.insert(Definition(
                    kill.clone(),
                    instruction.value().expect("kill without value somehow"),
                    block_idx,
                ));
            }
        }
        inputs
    }

    solve_dataflow(
        cfg,
        Direction::Forward,
        cfg.signature
            .arguments
            .iter()
            .map(|argument| {
                Definition(
                    argument.name.clone(),
                    InstructionValue::Argument,
                    cfg.entry,
                )
            })
            .collect(),
        |lhs, rhs| lhs.union(rhs).cloned().collect(),
        transfer,
    )
}
