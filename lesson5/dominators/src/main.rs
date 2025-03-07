use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    path::PathBuf,
    str::FromStr,
};

use argh::FromArgs;
use bril_rs::Program;
use build_cfg::{slotmap::SecondaryMap, BasicBlockIdx, FunctionCfg};
use dominators::{
    compute_dominance_frontiers, compute_dominator_tree, compute_dominators,
};
use serde_json::json;
use snafu::{whatever, ResultExt, Whatever};

enum Algorithm {
    Dominators,
    DominatorTree,
    DominationFrontier,
}

impl FromStr for Algorithm {
    type Err = Whatever;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "dom" => Self::Dominators,
            "tree" => Self::DominatorTree,
            "front" => Self::DominationFrontier,
            _ => whatever!("Unknown algorithm '{}'", s),
        })
    }
}
/// computes dominators and related stuff
#[derive(FromArgs)]
struct Opts {
    /// algorithm
    #[argh(option)]
    algo: Algorithm,

    /// input Bril file: omit for stdin
    #[argh(positional)]
    input: Option<PathBuf>,
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
        let dominators = compute_dominators(&cfg);

        match &opts.algo {
            Algorithm::Dominators => {
                print_block_info_sorted(&cfg, dominators);
            }
            Algorithm::DominatorTree => {
                let tree = compute_dominator_tree(dominators);
                print_block_info_sorted(&cfg, tree);
            }
            Algorithm::DominationFrontier => {
                let frontiers = compute_dominance_frontiers(&cfg, dominators);
                print_block_info_sorted(&cfg, frontiers);
            }
        }
    }

    Ok(())
}

fn print_block_info_sorted(
    cfg: &FunctionCfg,
    blocks: SecondaryMap<BasicBlockIdx, HashSet<BasicBlockIdx>>,
) {
    let mut printout = BTreeMap::new();
    for (block_idx, block_info) in blocks {
        if let Some(label) = cfg.vertices[block_idx]
            .label
            .as_ref()
            .map(|label| label.name.as_str())
        {
            let mut dominators = block_info
                .into_iter()
                .flat_map(|idx| {
                    cfg.vertices[idx]
                        .label
                        .as_ref()
                        .map(|label| label.name.as_str())
                })
                .collect::<Vec<_>>();
            dominators.sort();
            printout.insert(label, dominators);
        }
    }
    println!("{}", json!(printout));
}
