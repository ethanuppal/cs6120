use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::PathBuf,
};

use argh::FromArgs;
use bril_rs::{Instruction, Program};
use build_cfg::{
    BasicBlock, BasicBlockIdx, print::print_cfg_as_bril_text, slotmap::SlotMap,
};
use snafu::{ResultExt, Whatever};

/// does trivial dead code elimination
#[derive(FromArgs)]
struct Opts {
    /// input Bril file: omit for stdin
    #[argh(positional)]
    input: Option<PathBuf>,
}

fn trivial_dead_code_elimination(
    blocks: &mut SlotMap<BasicBlockIdx, BasicBlock>,
) -> bool {
    let mut used_variables = HashSet::new();

    for block in blocks.values() {
        for instruction in &block.instructions {
            if let Instruction::Value { args, .. }
            | Instruction::Effect { args, .. } = &instruction
            {
                used_variables.extend(args.clone());
            }
        }
    }

    let mut changed = false;
    for block in blocks.values_mut() {
        let old_length = block.instructions.len();
        block.instructions.retain(|instruction| match instruction {
            Instruction::Constant { dest, .. }
            | Instruction::Value { dest, .. } => used_variables.contains(dest),
            Instruction::Effect { .. } => true,
        });
        changed |= old_length != block.instructions.len();
    }
    changed
}

fn drop_killed_locals(block: &mut BasicBlock) -> bool {
    let mut unused_definitions = HashMap::new();
    let mut dead_instructions = vec![];

    for (i, instruction) in block.instructions.iter().enumerate() {
        let (kill, gen_set): (Option<&String>, &[String]) = match instruction {
            Instruction::Constant { dest, .. } => (Some(dest), &[]),
            Instruction::Value { args, dest, .. } => {
                (Some(dest), args.as_slice())
            }
            Instruction::Effect { args, .. } => (None, args.as_slice()),
        };

        for usage in gen_set {
            unused_definitions.remove(usage);
        }
        if let Some(kill) = kill {
            if let Some(dead_instruction_index) =
                unused_definitions.get(kill).copied()
            {
                dead_instructions.push(dead_instruction_index);
            }
            unused_definitions.insert(kill.clone(), i);
        }
    }

    dead_instructions.sort_unstable();
    for i in dead_instructions.iter().rev().copied() {
        block.instructions.remove(i);
    }

    !dead_instructions.is_empty()
}

fn drop_lots_of_killed_local(
    blocks: &mut SlotMap<BasicBlockIdx, BasicBlock>,
) -> bool {
    let mut changed = false;
    for block in blocks.values_mut() {
        changed |= drop_killed_locals(block);
    }
    changed
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

    for import in program.imports {
        println!("{}", import);
    }
    for function in program.functions {
        let mut cfg = build_cfg::build_cfg(&function, false)
            .whatever_context("Failed to build cfg")?;

        //trivial_dead_code_elimination(&mut cfg.vertices);
        while trivial_dead_code_elimination(&mut cfg.vertices)
            || drop_lots_of_killed_local(&mut cfg.vertices)
        {}

        print_cfg_as_bril_text(cfg);
    }

    Ok(())
}
