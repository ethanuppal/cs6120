// Copyright (C) 2024 Ethan Uppal. All rights reserved.
//
// Please see the LICENSE file in the project root directory.

use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
    str::FromStr,
};

use argh::FromArgs;
use bril_rs::Program;
use build_cfg::{build_cfg, print::print_cfg_as_bril_text, Exit};
use inform::{common::IndentWriterCommon, io::IndentWriter};
use owo_colors::OwoColorize;
use snafu::{ResultExt, Whatever};

enum Mode {
    Passthrough,
    Pretty,
}

impl FromStr for Mode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "passthrough" => Ok(Self::Passthrough),
            "pretty" => Ok(Self::Pretty),
            other => Err(format!("Unknown printing mode '{}'", other)),
        }
    }
}

/// Extracts the control-flow graph from a Bril program.
#[derive(FromArgs)]
struct Opts {
    /// type of printing
    #[argh(option)]
    mode: Mode,

    /// input Bril file: omit for stdin
    #[argh(positional)]
    input: Option<PathBuf>,
}

fn print_reconstructed(program: Program) -> Result<(), Whatever> {
    for import in program.imports {
        println!("{}", import);
    }

    for function in &program.functions {
        let cfg = build_cfg(function).whatever_context(format!(
            "Failed to build control-flow graph for function `{}`",
            function.name
        ))?;
        print_cfg_as_bril_text(cfg);
    }

    Ok(())
}

fn print_pretty(program: Program) -> Result<(), Whatever> {
    let mut stdout = io::stdout();
    let mut f = IndentWriter::new(&mut stdout, 4);

    for function in &program.functions {
        let cfg = build_cfg(function).whatever_context(format!(
            "Failed to build control-flow graph for function `{}`",
            function.name
        ))?;

        writeln!(f, "{} {{", function.name.bold().white())
            .whatever_context("Writing to stdout failed")?;
        f.increase_indent();

        for (i, (block_idx, block)) in cfg.vertices.iter().enumerate() {
            if i > 0 {
                writeln!(f).whatever_context("Writing to stdout failed")?;
            }

            if block.is_entry {
                write!(f, "{} -> ", "<entry>".bold().bright_green())
                    .whatever_context("Writing to stdout failed")?;
            }

            if let Some(label) = &block.label {
                write!(f, ".{} ", label.name.on_truecolor(64, 64, 64))
                    .whatever_context("Writing to stdout failed")?;
            }
            writeln!(
                f,
                "[{}] {{",
                block_idx.as_number().to_string().bold().bright_green()
            )
            .whatever_context("Writing to stdout failed")?;

            f.increase_indent();
            for instruction in &block.instructions {
                writeln!(
                    f,
                    "{}",
                    instruction.to_string().truecolor(128, 128, 128)
                )
                .whatever_context("Writing to stdout failed")?;
            }
            f.decrease_indent();
            write!(f, "}} ").whatever_context("Writing to stdout failed")?;

            if let Some(exit) = cfg.edges.get(block_idx).cloned() {
                match exit {
                    Exit::Fallthrough(destination) => {
                        if let Some(destination) = destination {
                            write!(
                                f,
                                "-> {}",
                                destination
                                    .as_number()
                                    .to_string()
                                    .bold()
                                    .bright_green()
                            )
                            .whatever_context("Writing to stdout failed")?;
                            if let Some(label) =
                                &cfg.vertices[destination].label
                            {
                                write!(
                                    f,
                                    " (.{})",
                                    label.name.on_truecolor(64, 64, 64)
                                )
                                .whatever_context("Writing to stdout failed")?;
                            }
                            writeln!(f)
                                .whatever_context("Writing to stdout failed")?;
                        }
                    }
                    Exit::Unconditional(destination) => {
                        write!(
                            f,
                            "-> {}",
                            destination
                                .as_number()
                                .to_string()
                                .bold()
                                .bright_green()
                        )
                        .whatever_context("Writing to stdout failed")?;
                        if let Some(label) = &cfg.vertices[destination].label {
                            write!(
                                f,
                                " (.{})",
                                label.name.on_truecolor(64, 64, 64)
                            )
                            .whatever_context("Writing to stdout failed")?;
                        }
                        writeln!(f)
                            .whatever_context("Writing to stdout failed")?;
                    }
                    Exit::Conditional {
                        condition,
                        if_true,
                        if_false,
                    } => {
                        write!(
                            f,
                            "({}) -> {}",
                            condition.truecolor(128, 128, 128),
                            if_true
                                .as_number()
                                .to_string()
                                .bold()
                                .bright_green()
                        )
                        .whatever_context("Writing to stdout failed")?;
                        if let Some(label) = &cfg.vertices[if_true].label {
                            write!(
                                f,
                                " (.{})",
                                label.name.on_truecolor(64, 64, 64)
                            )
                            .whatever_context("Writing to stdout failed")?;
                        }
                        writeln!(f)
                            .whatever_context("Writing to stdout failed")?;
                        write!(
                            f,
                            "  ({}) -> {}",
                            format!("!{}", condition).truecolor(128, 128, 128),
                            if_false
                                .as_number()
                                .to_string()
                                .bold()
                                .bright_green()
                        )
                        .whatever_context("Writing to stdout failed")?;
                        if let Some(label) = &cfg.vertices[if_false].label {
                            write!(
                                f,
                                " (.{})",
                                label.name.on_truecolor(64, 64, 64)
                            )
                            .whatever_context("Writing to stdout failed")?;
                        }
                        writeln!(f)
                            .whatever_context("Writing to stdout failed")?;
                    }
                    Exit::Return(value) => {
                        writeln!(
                            f,
                            "-> {}{}",
                            "<return>".bold().bright_green(),
                            if let Some(value) = value {
                                format!(" {}", value)
                            } else {
                                "".into()
                            }
                            .truecolor(128, 128, 128)
                        )
                        .whatever_context("Writing to stdout failed")?;
                    }
                }
            } else {
                writeln!(f).whatever_context("Writing to stdout failed")?;
            }
        }

        f.decrease_indent();
        writeln!(f, "}}\n").whatever_context("Writing to stdout failed")?;
    }

    Ok(())
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

    match opts.mode {
        Mode::Passthrough => print_reconstructed(program)?,
        Mode::Pretty => print_pretty(program)?,
    };

    Ok(())
}
