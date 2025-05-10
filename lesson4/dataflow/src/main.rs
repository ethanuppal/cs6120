use std::{fs, io, path::PathBuf, str::FromStr};

use argh::FromArgs;
use bril_rs::Program;
use dataflow::{
    live_variables::live_variables,
    reaching_definitions::{
        compute_reaching_definitions, definition_is_reachable,
    },
};
use snafu::{ResultExt, Whatever, whatever};

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
            Analysis::ReachingDefinitions => {
                let solution = compute_reaching_definitions(&cfg);
                println!("@{} {{", cfg.signature.name);
                for (block, solution) in solution {
                    if let Some(label) = &cfg.vertices[block].label {
                        println!("  .{}", label.name);
                    }
                    let mut printouts = solution
                        .iter()
                        .map(|definition| {
                            format!("    {} = {:?}", definition.0, definition.1)
                        })
                        .collect::<Vec<_>>();
                    printouts.sort();
                    for printout in printouts {
                        println!("{}", printout);
                    }

                    for definition in solution {
                        if !definition_is_reachable(&cfg, block, &definition) {
                            panic!(
                                "No reachable definition found for {:?} = {:?}",
                                definition.0, definition.1
                            );
                        }
                    }
                }
                println!("}}");
            }
            Analysis::LiveVariables => live_variables(&cfg),
        }
    }

    Ok(())
}
