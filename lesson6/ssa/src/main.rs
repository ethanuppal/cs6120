use std::{collections::BTreeMap, fs, io, path::PathBuf};

use argh::FromArgs;
use bril_rs::Program;
use build_cfg::print;
use snafu::{ResultExt, Whatever, whatever};

/// Transforms Bril into and out of SSA
#[derive(FromArgs)]
struct Opts {
    /// translate into SSA
    #[argh(switch)]
    into_ssa: bool,

    /// translate from SSA
    #[argh(switch)]
    from_ssa: bool,

    /// skip the step after inserting Phi nodes. ignored unless --into-ssa is
    /// passed
    #[argh(switch)]
    skip_post_phi_insertion: bool,

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
        match (opts.into_ssa, opts.from_ssa) {
            (true, false) => {
                let mut cfg = build_cfg::build_cfg(&function, true)
                    .whatever_context("Failed to build cfg")?;

                ssa::insert_new_empty_entry_block(&mut cfg);

                let dominators = dominators::compute_dominators(&cfg);
                let dominance_tree =
                    dominators::compute_dominator_tree(&dominators);
                let dominance_frontiers =
                    dominators::compute_dominance_frontiers(&cfg, dominators);

                // 1: Insert phi nodes

                let definition_sites = ssa::compute_definition_sites(&cfg);
                let phi_insertion_points = ssa::determine_phi_insertion_points(
                    definition_sites,
                    dominance_frontiers,
                );
                ssa::insert_phis(&mut cfg, phi_insertion_points);

                if !opts.skip_post_phi_insertion {
                    // 2: Rename variables and insert upsilon nodes

                    ssa::simulate_parameters_as_locals(&mut cfg);

                    let entry = cfg.entry;
                    let mut dominating_definitiions_stacks =
                        ssa::DominatingDefinitionsStacks::default();
                    let mut undefined_names = BTreeMap::new();
                    ssa::rename_and_insert_upsilons(
                        &mut cfg,
                        entry,
                        &dominance_tree,
                        &mut dominating_definitiions_stacks,
                        &mut undefined_names,
                    );

                    ssa::insert_undefined_names_at_entry(
                        &mut cfg,
                        undefined_names,
                    );

                    assert!(
                        ssa::is_ssa(&cfg),
                        "Result of SSA transformation was not SSA"
                    );
                }

                print::print_cfg_as_bril_text(cfg);
            }
            (false, true) => {
                let mut cfg = build_cfg::build_cfg(&function, true)
                    .whatever_context("Failed to build cfg")?;

                ssa::from_ssa(&mut cfg)
                    .whatever_context("Failed to convert out of SSA form")?;

                print::print_cfg_as_bril_text(cfg);
            }
            _ => whatever!("Pass only one of --into-ssa or --from-ssa"),
        }
    }

    Ok(())
}
