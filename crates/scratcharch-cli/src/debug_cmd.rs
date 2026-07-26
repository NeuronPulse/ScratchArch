use scratcharch_scratchgraph::SourceMapBuilder;

pub fn run(input: &str, _sair: Option<&str>) -> Result<(), String> {
    let raw = std::fs::read_to_string(input).map_err(|e| e.to_string())?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| e.to_string())?;

    let mut builder = SourceMapBuilder::new();
    let _project: scratcharch_scratchgraph::ir::Project =
        builder.parse(&value).map_err(|e: scratcharch_scratchgraph::ParseError| e.to_string())?;

    println!("Source map for: {}", input);
    for target in &builder.source_map {
        println!("  target: {}", target.target_name);
        for block in &target.blocks {
            println!(
                "    {} opcode={} top={} parent={:?}",
                block.block_id, block.opcode, block.is_top_level, block.parent
            );
        }
    }
    Ok(())
}
