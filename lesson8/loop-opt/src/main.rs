use std::{collections::BTreeSet, fs, io, path::PathBuf};

use argh::FromArgs;
use bril_rs::Program;
use build_cfg::{print, BasicBlock, BasicBlockIdx, Label};
use snafu::{ResultExt, Whatever};

#[repr(u32)]
enum Stage {
    InsertPreheader,
}

/// Performs loop optimization.
#[derive(FromArgs)]
struct Opts {
    /// input Bril file: omit for stdin
    #[argh(positional)]
    input: Option<PathBuf>,

    /// stage: 0 = insert preheader
    #[argh(option, default = "0")]
    stage: u32,
}

struct NaturalLoop {
    header: BasicBlockIdx,
    backedge_start: BasicBlockIdx,
    body: BTreeSet<BasicBlockIdx>,
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
        let mut cfg = build_cfg::build_cfg(&function, true)
            .whatever_context("Failed to build cfg")?;

        cfg.make_fallthroughs_explicit();

        let dominators = dominators::compute_dominators(&cfg);
        let dominance_tree = dominators::compute_dominator_tree(&dominators);

        let mut back_edges = vec![];
        for start in cfg.vertices.keys() {
            for end in cfg.successors(start) {
                if dominance_tree[end].contains(&start) {
                    back_edges.push((start, end));
                }
            }
        }

        let mut natural_loops = vec![];
        for (start, end) in back_edges {
            let mut natural_loop = BTreeSet::from_iter([end]);
            let mut stack = vec![start];
            while let Some(next) = stack.pop() {
                if !natural_loop.contains(&next) {
                    natural_loop.insert(next);
                    stack.extend(cfg.predecessors(next));
                }
            }

            // println!("new loop containing:");
            // println!(
            //     "* backedge {:?} -> {:?}",
            //     cfg.vertices[start].label, cfg.vertices[end].label
            // );
            // print!("contents:");
            // for block in &natural_loop {
            //     print!(" {:?}", cfg.vertices[*block].label);
            // }
            // println!();

            natural_loops.push(NaturalLoop {
                header: end,
                backedge_start: start,
                body: natural_loop,
            });
        }

        for NaturalLoop {
            header,
            backedge_start,
            body,
        } in natural_loops
        {
            let preheader = cfg.add_block(BasicBlock {
                label: Some(Label {
                    name: format!(
                        "{}_preheader",
                        cfg.vertices[header]
                            .label
                            .as_ref()
                            .map(|label| label.name.clone())
                            .unwrap_or_default()
                    ),
                }),
                ..Default::default()
            });
            for header_predecessor in cfg.predecessors(header).to_vec() {
                cfg.reorient_edge(header_predecessor, header, preheader);
            }
            cfg.set_unconditional_edge(preheader, header);
        }

        if opts.stage == Stage::InsertPreheader as u32 {
            print::print_cfg_as_bril_text(cfg);
            continue;
        }
    }

    Ok(())
}
