use std::fs;
use std::path::Path;

use scratcharch_driver::{CompileConfig, CompileDriver, OptLevel};
use scratcharch_scratchgraph::{JsonExporter, ScratchExporter, ScratchGraphLowerer};

pub fn run(input: &str, output: &Option<String>, optimize: &str) -> Result<(), String> {
    let ext = Path::new(input)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "ll" => build_from_llvm(input, output, optimize),
        _ => build_from_scratchgraph(input, output),
    }
}

fn build_from_llvm(input: &str, output: &Option<String>, optimize: &str) -> Result<(), String> {
    let opt_level = match optimize {
        "none" => OptLevel::None,
        "aggressive" => OptLevel::Aggressive,
        _ => OptLevel::Basic,
    };

    let config = CompileConfig {
        opt_level,
        ..CompileConfig::default()
    };
    let driver = CompileDriver::new(config);
    let compiled = driver.compile_file(input).map_err(|e| e.to_string())?;

    let lowerer = ScratchGraphLowerer::new();
    let project = lowerer.lower(&compiled.module).map_err(|e| e.to_string())?;

    let json = JsonExporter::new().export(&project).map_err(|_| "export failed".to_string())?;
    let output_path = output.clone().unwrap_or_else(|| input.replace(".ll", ".json"));
    let json_str = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
    fs::write(&output_path, &json_str).map_err(|e| e.to_string())?;
    println!("wrote {}", output_path);
    Ok(())
}

fn build_from_scratchgraph(input: &str, output: &Option<String>) -> Result<(), String> {
    let json_str = fs::read_to_string(input).map_err(|e| e.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| e.to_string())?;
    let project = scratcharch_scratchgraph::parse_project_json(&value).map_err(|e| e.to_string())?;
    let json = JsonExporter::new().export(&project).map_err(|_| "export failed".to_string())?;
    let output_path = output.clone().unwrap_or_else(|| input.replace(".json", ".rebuilt.json"));
    let json_str = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
    fs::write(&output_path, &json_str).map_err(|e| e.to_string())?;
    println!("wrote {}", output_path);
    Ok(())
}
