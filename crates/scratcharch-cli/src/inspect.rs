use std::fs;

use scratcharch_explorer::sair::SairExplorer;
use scratcharch_explorer::scratch::ScratchExplorer;

pub fn run(input: &str, json: bool) -> Result<(), String> {
    let raw = fs::read_to_string(input).map_err(|e| e.to_string())?;

    // Try to parse as SAIR first.
    if let Ok(module) = scratcharch_ir::text::deserialize(&raw) {
        let explorer = SairExplorer::new();
        if json {
            let s = explorer.to_json(&module).map_err(|e| e.to_string())?;
            println!("{}", s);
        } else {
            print!("{}", explorer.to_text(&module));
        }
        return Ok(());
    }

    // Fallback: parse as Scratch project JSON.
    let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let project = scratcharch_scratchgraph::parse_project_json(&value).map_err(|e| e.to_string())?;

    let explorer = ScratchExplorer::new();
    if json {
        let s = explorer.to_json(&project).map_err(|e| e.to_string())?;
        println!("{}", s);
    } else {
        print!("{}", explorer.to_text(&project));
    }

    Ok(())
}
