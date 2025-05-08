use std::{collections::BTreeMap, fs, io, path::PathBuf};

use argh::FromArgs;
use bril_rs::Program;
use build_cfg::print;
use snafu::{ResultExt, Whatever, whatever};

/// Performs loop optimization.
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
        let mut cfg = build_cfg::build_cfg(&function, true)
            .whatever_context("Failed to build cfg")?;
    }

    Ok(())
}
