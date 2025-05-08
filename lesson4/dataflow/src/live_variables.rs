use std::collections::HashSet;

use bril_util::InstructionExt;
use build_cfg::{BasicBlock, BasicBlockIdx, FunctionCfg};

use crate::{Direction, solve_dataflow};

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Variable(String);

fn transfer(
    block: &BasicBlock,
    _block_idx: BasicBlockIdx,
    mut outputs: HashSet<Variable>,
) -> HashSet<Variable> {
    let mut kill_set = HashSet::new();
    let mut gen_set = HashSet::new();
    for instruction in &block.instructions {
        gen_set.extend(
            instruction
                .gen_set()
                .iter()
                .filter(|variable| !kill_set.contains(variable))
                .map(|variable| Variable(variable.to_string())),
        );
        if let Some(kill) = instruction.kill() {
            kill_set.insert(kill);
            outputs.remove(&Variable(kill.clone()));
        }
    }
    outputs.extend(gen_set);
    outputs
}

pub fn live_variables(cfg: &FunctionCfg) {
    println!("@{} {{", cfg.signature.name);
    for (block, solution) in solve_dataflow(
        cfg,
        Direction::Backward,
        HashSet::new(),
        |lhs, rhs| lhs.union(rhs).cloned().collect(),
        transfer,
    ) {
        if let Some(label) = &cfg.vertices[block].label {
            println!("  .{}", label.name);
        }
        let mut variables = solution
            .into_iter()
            .map(|variable| variable.0)
            .collect::<Vec<_>>();
        variables.sort();
        println!(
            "  in:  {}",
            if variables.is_empty() {
                "∅".to_string()
            } else {
                variables.join(", ")
            }
        );
    }
}
