use crate::FunctionCfg;

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
    for block in cfg.vertices.values() {
        if let Some(label) = &block.label {
            println!(".{}:", label.name);
        }
        for instruction in &block.instructions {
            println!("  {}", instruction);
        }
    }
    println!("}}");
}
