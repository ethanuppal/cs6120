use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    path::PathBuf,
};

use argh::FromArgs;
use bril_rs::Program;
use build_cfg::slotmap::SecondaryMap;
use dataflow::construct_postorder;
use serde_json::json;
use snafu::{ResultExt, Whatever};

/// computes dominators and related stuff
#[derive(FromArgs)]
struct Opts {
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

        let mut printout = BTreeMap::new();
        for (block_idx, dominators) in dominators {
            if let Some(label) = cfg.vertices[block_idx]
                .label
                .as_ref()
                .map(|label| label.name.as_str())
            {
                let mut dominators = dominators
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

    Ok(())
}
