use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    path::PathBuf,
    str::FromStr,
};

use argh::FromArgs;
use bril_rs::Program;
use build_cfg::{slotmap::SecondaryMap, BasicBlockIdx, FunctionCfg};
use dataflow::construct_postorder;
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

        let mut reverse_postorder = construct_postorder(&cfg);
        reverse_postorder.reverse();
        reverse_postorder.retain(|idx| *idx != cfg.entry);

        let all_blocks = cfg.vertices.keys().collect::<HashSet<_>>();
        let mut dominators = SecondaryMap::new();
        for block_idx in cfg.vertices.keys() {
            dominators.insert(block_idx, all_blocks.clone());
        }
        dominators[cfg.entry] = HashSet::from_iter([cfg.entry]);

        let mut needs_update = true;
        while needs_update {
            needs_update = false;
            for block_idx in reverse_postorder.iter().copied() {
                let previous = dominators[block_idx].clone();
                let mut new = HashSet::new();
                for (i, pred_idx) in
                    cfg.predecessors(block_idx).iter().copied().enumerate()
                {
                    if i == 0 {
                        new = dominators[pred_idx].clone();
                    } else {
                        new = new
                            .intersection(&dominators[pred_idx])
                            .copied()
                            .collect();
                    }
                }
                new.insert(block_idx);
                if new != previous {
                    needs_update = true;
                }
                dominators[block_idx] = new;
            }
        }

        match &opts.algo {
            Algorithm::Dominators => {
                print_block_info_sorted(&cfg, dominators);
            }
            Algorithm::DominatorTree => {
                let mut rev = SecondaryMap::<_, HashSet<_>>::new();
                for (idx, edge) in dominators {
                    for dest_idx in edge {
                        let entry = rev.entry(dest_idx).unwrap().or_default();
                        if idx != dest_idx {
                            entry.insert(idx);
                        }
                    }
                }

                let mut tree = SecondaryMap::<_, HashSet<_>>::new();

                for (idx, mut dominated) in rev.clone() {
                    for (other_idx, other_dominated) in &rev {
                        if other_idx != idx && !other_dominated.contains(&idx) {
                            dominated.retain(|dominated_idx| {
                                !other_dominated.contains(dominated_idx)
                            });
                        }
                    }
                    tree.insert(idx, dominated);
                }

                print_block_info_sorted(&cfg, tree);

                //
                ////let mut visited = SecondaryMap::new();
                ////let mut bfs = VecDeque::new();
                ////bfs.push_back(cfg.entry);
                ////
                ////let mut tree = SecondaryMap::<_, HashSet<_>>::new();
                ////
                ////while let Some(block_idx) = bfs.pop_front() {
                ////    visited.insert(block_idx, ());
                ////
                ////    for neighbor_idx in &rev[block_idx] {
                ////        if !visited.contains_key(*neighbor_idx) {
                ////            bfs.push_back(*neighbor_idx);
                ////            tree.entry(block_idx)
                ////                .unwrap()
                ////                .or_default()
                ////                .insert(*neighbor_idx);
                ////        }
                ////    }
                ////}
                //
            }
            _ => todo!(),
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
