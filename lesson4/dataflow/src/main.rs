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
use build_cfg::{
    slotmap::SecondaryMap, BasicBlock, BasicBlockIdx, FunctionCfg,
};
use dataflow::construct_postorder;
use live_variables::live_variables;
use reaching_definitions::reaching_definitions;
use snafu::{whatever, ResultExt, Whatever};

pub mod live_variables;
pub mod reaching_definitions;

enum Analysis {
    ReachingDefinitions,
    LiveVariables,
}

impl FromStr for Analysis {
    type Err = Whatever;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "def" => Self::ReachingDefinitions,
            "live" => Self::LiveVariables,
            _ => whatever!("Unknown analysis '{}'", s),
        })
    }
}

pub enum Direction {
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
        let cfg = build_cfg::build_cfg(&function, true)
            .whatever_context("Failed to build cfg")?;

        match opts.analysis {
            Analysis::ReachingDefinitions => reaching_definitions(&cfg),
            Analysis::LiveVariables => live_variables(&cfg),
        }
    }

    Ok(())
}
