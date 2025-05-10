use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs, io,
    path::PathBuf,
};

use argh::FromArgs;
use bril_rs::{Instruction, Program};
use bril_util::InstructionExt;
use build_cfg::{
    print, slotmap::SecondaryMap, BasicBlock, BasicBlockIdx, FunctionCfg, Label,
};
use dataflow::reaching_definitions::{
    self, compute_reaching_definitions, Definition,
};
use snafu::{ResultExt, Whatever};

#[repr(u32)]
enum Stage {
    InsertPreheader,
    LoopInvariantCodeMotion,
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

struct NaturalLoopWithPreheader {
    preheader: BasicBlockIdx,
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

        let mut natural_loops_with_preheaders = vec![];
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

            natural_loops_with_preheaders.push(NaturalLoopWithPreheader {
                preheader,
                header,
                backedge_start,
                body,
            });
        }

        if opts.stage == Stage::InsertPreheader as u32 {
            print::print_cfg_as_bril_text(cfg);
            continue;
        }

        for NaturalLoopWithPreheader {
            preheader,
            header,
            backedge_start,
            body,
        } in natural_loops_with_preheaders
        {
            eprintln!("==== PART 1 ====");
            let reaching_definitions = compute_reaching_definitions(&cfg);
            let mut loop_invariant =
                SecondaryMap::<BasicBlockIdx, BTreeSet<usize>>::new();

            let mut changed = true;
            while changed {
                changed = false;
                for block in &body {
                    for (i, instruction) in
                        cfg.vertices[*block].instructions.iter().enumerate()
                    {
                        match instruction {
                            Instruction::Value { dest, args, .. } => {
                                if args.iter().all(|arg| {
                                    let reaching_definitions_of_arg =
                                        reaching_definitions[*block]
                                            .iter()
                                            .filter(|definition| {
                                                definition.0.as_str()
                                                    == arg.as_str()
                                            })
                                            .collect::<Vec<_>>();

                                    reaching_definitions_of_arg.iter().all(
                                        |definition| {
                                            !body.contains(&definition.2)
                                        },
                                    ) || (reaching_definitions_of_arg.len()
                                        == 1
                                        && {
                                            let definition =
                                                reaching_definitions_of_arg[0];
                                            loop_invariant
                                                .entry(definition.2)
                                                .unwrap()
                                                .or_default()
                                                .contains(
                                                    &(definition.3 as usize),
                                                )
                                        })
                                }) {
                                    eprintln!(
                                        "{instruction:?} is loop invariant"
                                    );
                                    changed |= loop_invariant
                                        .entry(*block)
                                        .unwrap()
                                        .or_default()
                                        .insert(i);
                                }
                            }
                            Instruction::Constant { .. } => {
                                eprintln!("{instruction:?} is loop invariant");
                                changed |= loop_invariant
                                    .entry(*block)
                                    .unwrap()
                                    .or_default()
                                    .insert(i);
                            }
                            _ => {}
                        }
                    }
                }
            }

            fn is_unique_definition(
                definition: (BasicBlockIdx, usize),
                loop_body: &BTreeSet<BasicBlockIdx>,
                cfg: &FunctionCfg,
            ) -> bool {
                let definition_name = cfg.vertices[definition.0].instructions
                    [definition.1]
                    .kill()
                    .expect("should be a Value or Constant instruction");

                loop_body.iter().fold(0, |acc, block| {
                    acc + cfg.vertices[*block]
                        .instructions
                        .iter()
                        .filter(|instruction| {
                            instruction.kill() == Some(&definition_name)
                        })
                        .count()
                }) == 1
            }

            let exit_blocks: BTreeSet<BasicBlockIdx> = body
                .iter()
                .flat_map(|&block_idx| cfg.successors(block_idx))
                .filter(|successor| !body.contains(successor))
                .collect();

            fn dominates_uses(
                definition_block: BasicBlockIdx,
                use_blocks: &[BasicBlockIdx],
                dominators: &SecondaryMap<
                    BasicBlockIdx,
                    HashSet<BasicBlockIdx>,
                >,
            ) -> bool {
                use_blocks.iter().all(|&use_block| {
                    dominators[use_block].contains(&definition_block)
                })
            }

            fn dominates_exits(
                definition_block: BasicBlockIdx,
                exit_blocks: &BTreeSet<BasicBlockIdx>,
                dominators: &SecondaryMap<
                    BasicBlockIdx,
                    HashSet<BasicBlockIdx>,
                >,
            ) -> bool {
                exit_blocks.iter().all(|&exit_block| {
                    dominators
                        .get(exit_block)
                        .map(|exit_block| {
                            exit_block.contains(&definition_block)
                        })
                        .unwrap_or(true)
                })
            }

            eprintln!("==== PART 2 ====");

            for (block, instructions) in loop_invariant {
                let mut to_move = vec![];
                for instruction_idx in instructions {
                    let mut use_blocks = vec![];

                    let kill = cfg.vertices[block].instructions
                        [instruction_idx]
                        .kill()
                        .unwrap();
                    for other_block in &body {
                        for other_instruction in
                            &cfg.vertices[*other_block].instructions
                        {
                            if other_instruction.gen_set().contains(&kill) {
                                use_blocks.push(*other_block);
                                break;
                            }
                        }
                    }

                    // eprintln!(
                    //     "{:?} is a unique def?: {}",
                    //     cfg.vertices[block].instructions[instruction_idx],
                    //     is_unique_definition(
                    //         (block, instruction_idx),
                    //         &body,
                    //         &cfg
                    //     )
                    // );
                    // eprintln!(
                    //     "dominates uses?: {}",
                    //     dominates_uses(block, &use_blocks, &dominators)
                    // );
                    // eprintln!(
                    //     "dominates exits?: {}",
                    //     dominates_exits(block, &exit_blocks, &dominators)
                    // );
                    if is_unique_definition(
                        (block, instruction_idx),
                        &body,
                        &cfg,
                    ) && dominates_uses(block, &use_blocks, &dominators)
                        && dominates_exits(block, &exit_blocks, &dominators)
                    {
                        eprintln!(
                            "moving {:?}",
                            cfg.vertices[block].instructions[instruction_idx],
                        );
                        to_move.push(instruction_idx);
                    }
                }

                while let Some(to_move) = to_move.pop() {
                    let instruction =
                        cfg.vertices[block].instructions.remove(to_move);
                    cfg.vertices[preheader].instructions.insert(0, instruction);
                }
            }

            // Finally, since the back edge has been reoriented, we bring it
            // back to the original header
            cfg.reorient_edge(backedge_start, preheader, header);
        }

        if opts.stage == Stage::LoopInvariantCodeMotion as u32 {
            print::print_cfg_as_bril_text(cfg);
            continue;
        }
    }

    Ok(())
}
