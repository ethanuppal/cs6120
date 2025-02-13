use std::{
    collections::{HashSet, VecDeque},
    fs,
    hash::Hash,
    io,
    path::PathBuf,
    str::FromStr,
};

use argh::FromArgs;
use bril_rs::Program;
use bril_util::{InstructionExt, InstructionValue};
use build_cfg::{
    slotmap::SecondaryMap, BasicBlock, BasicBlockIdx, FunctionCfg,
};
use snafu::{whatever, ResultExt, Whatever};

enum Analysis {
    ReachingDefinitions,
}

impl FromStr for Analysis {
    type Err = Whatever;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "def" => Self::ReachingDefinitions,
            _ => whatever!("Unknown analysis '{}'", s),
        })
    }
}

enum Direction {
    Forward,
    Backward,
}

/// Performs dataflow analysis on the given Bril program
#[derive(FromArgs)]
struct Opts {
    /// the type of dataflow analysis to perform
    #[argh(option)]
    analysis: Analysis,

    /// input Bril file; omit for stdin
    #[argh(positional)]
    input: Option<PathBuf>,
}

fn construct_postorder(cfg: &FunctionCfg) -> Vec<BasicBlockIdx> {
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

fn solve_dataflow<T: Clone + PartialEq + Eq + Hash>(
    cfg: &FunctionCfg,
    direction: Direction,
    entry_inputs: HashSet<T>,
    merge: impl Fn(HashSet<T>, &HashSet<T>) -> HashSet<T>,
    transfer: impl Fn(&BasicBlock, HashSet<T>) -> HashSet<T>,
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
        let new_out = transfer(&cfg.vertices[current], initial_in);
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

#[derive(PartialEq, Eq, Hash, Clone)]
struct Definition(String, InstructionValue);

fn reaching_definitions(cfg: &FunctionCfg) {
    fn transfer(
        block: &BasicBlock,
        mut inputs: HashSet<Definition>,
    ) -> HashSet<Definition> {
        for instruction in &block.instructions {
            if let Some(kill) = instruction.kill() {
                inputs.retain(|input| &input.0 != kill);
                inputs.insert(Definition(
                    kill.clone(),
                    instruction.value().expect("kill without value somehow"),
                ));
            }
        }
        inputs
    }
    for (block, solution) in solve_dataflow(
        cfg,
        Direction::Forward,
        cfg.signature
            .arguments
            .iter()
            .map(|argument| {
                Definition(argument.name.clone(), InstructionValue::Argument)
            })
            .collect(),
        |lhs, rhs| lhs.union(rhs).cloned().collect(),
        transfer,
    ) {
        if let Some(label) = &cfg.vertices[block].label {
            println!("{}:", label.name);
        }
        for definition in solution {
            println!("  {} = {:?}", definition.0, definition.1);
        }
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

    for function in program.functions {
        let cfg = build_cfg::build_cfg(&function)
            .whatever_context("Failed to build cfg")?;

        match opts.analysis {
            Analysis::ReachingDefinitions => reaching_definitions(&cfg),
        }
    }

    Ok(())
}
