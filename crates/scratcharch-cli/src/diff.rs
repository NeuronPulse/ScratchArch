use std::fs;

use scratcharch_analyzer::diff::{semantic_diff, DiffFormat};

pub fn run(a: &str, b: &str, format: &str) -> Result<(), String> {
    let project_a = parse_project(a)?;
    let project_b = parse_project(b)?;

    let diff_fmt = match format {
        "json" => DiffFormat::Json,
        _ => DiffFormat::Text,
    };

    let diffs = semantic_diff(&project_a, &project_b);
    let output = diffs.format(diff_fmt);
    println!("{}", output);
    if diffs.is_empty() {
        println!("Projects are semantically equivalent.");
    }
    Ok(())
}

fn parse_project(path: &str) -> Result<scratcharch_scratchgraph::ir::Project, String> {
    let json_str = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| e.to_string())?;
    scratcharch_scratchgraph::parse_project_json(&value).map_err(|e| e.to_string())
}
