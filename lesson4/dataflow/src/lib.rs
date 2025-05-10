use std::{
    collections::{HashSet, VecDeque},
    hash::Hash,
};

use build_cfg::{
    BasicBlock, BasicBlockIdx, FunctionCfg, slotmap::SecondaryMap,
};

pub mod live_variables;
pub mod reaching_definitions;

pub enum Direction {
    Forward,
    Backward,
}

pub fn construct_postorder(cfg: &FunctionCfg) -> Vec<BasicBlockIdx> {
    fn helper(
        cfg: &FunctionCfg,
        current: BasicBlockIdx,
        visited: &mut SecondaryMap<BasicBlockIdx, bool>,
        traversal: &mut Vec<BasicBlockIdx>,
    ) {
        visited.insert(current, true);
        for successor in cfg.successors(current) {
            if !visited.contains_key(successor) {
                helper(cfg, successor, visited, traversal);
            }
        }
        traversal.push(current);
    }

    let mut traversal = vec![];
    let mut visited = SecondaryMap::with_capacity(cfg.vertices.capacity());
    helper(cfg, cfg.entry, &mut visited, &mut traversal);
    traversal
}

pub fn solve_dataflow<T: Clone + PartialEq + Eq + Hash>(
    cfg: &FunctionCfg,
    direction: Direction,
    entry_inputs: HashSet<T>,
    merge: impl Fn(HashSet<T>, &HashSet<T>) -> HashSet<T>,
    transfer: impl Fn(&BasicBlock, BasicBlockIdx, HashSet<T>) -> HashSet<T>,
) -> SecondaryMap<BasicBlockIdx, HashSet<T>> {
    let postorder_traversal = construct_postorder(cfg);
    let mut blocks = match direction {
        Direction::Forward => {
            VecDeque::from_iter(postorder_traversal.into_iter().rev())
        }
        Direction::Backward => VecDeque::from_iter(postorder_traversal),
    };

    let mut solution = SecondaryMap::with_capacity(cfg.vertices.capacity());
    for block_idx in cfg.vertices.keys() {
        solution.insert(block_idx, HashSet::new());
    }
    let mut initial_in = entry_inputs;
    while let Some(current) = blocks.pop_front() {
        match direction {
            Direction::Forward => {
                for predecessor in cfg.predecessors(current) {
                    initial_in = merge(initial_in, &solution[*predecessor]);
                }
            }
            Direction::Backward => {
                for predecessor in cfg.successors(current) {
                    initial_in = merge(initial_in, &solution[predecessor]);
                }
            }
        }

        let previous_out = solution[current].clone();
        let new_out = transfer(&cfg.vertices[current], current, initial_in);
        if new_out != previous_out {
            solution[current] = new_out;
            match direction {
                Direction::Forward => {
                    blocks.extend(cfg.successors(current));
                }
                Direction::Backward => {
                    blocks.extend(cfg.predecessors(current).iter().copied());
                }
            }
        }

        initial_in = HashSet::new();
    }
    solution
}
