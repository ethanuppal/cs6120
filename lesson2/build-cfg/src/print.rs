use std::iter;

use crate::FunctionCfg;

/// The entry block will always be printed first.
pub fn print_cfg_as_bril_text(cfg: FunctionCfg) {
    println!(
        "@{}({}){} {{",
        cfg.signature.name,
        cfg.signature
            .arguments
            .iter()
            .map(|argument| argument.to_string())
            .collect::<Vec<_>>()
            .join(", "),
        if let Some(return_type) = cfg.signature.return_type {
            format!(": {}", return_type)
        } else {
            "".into()
        }
    );

    // we do this thing so that if a project introduces a new entry block it'll
    // always be guaranteed to be printed first, so they can end the block
    // with a `.jmp` for example
    let blocks = iter::once(&cfg.vertices[cfg.entry]).chain(
        cfg.vertices.iter().filter_map(|(idx, block)| {
            if idx == cfg.entry { None } else { Some(block) }
        }),
    );
    for block in blocks {
        if let Some(label) = &block.label {
            println!(".{}:", label.name);
        }
        for instruction in &block.instructions {
            println!("  {}", instruction);
        }
    }
    println!("}}");
}
