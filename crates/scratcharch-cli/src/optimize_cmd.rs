use std::fs;
use std::path::Path;

use scratcharch_sb3::{Sb3Archive, Sb3Reader, Sb3Writer};
use scratcharch_scratchgraph::{parse_project_json, JsonExporter, ScratchExporter};
use scratcharch_transform::{parse_pass_list, PassManager};

pub fn run(
    input: &str,
    output: &Option<String>,
    passes: &str,
    format: &str,
    report: Option<&str>,
) -> Result<(), String> {
    let input_path = Path::new(input);
    let is_sb3 = input_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("sb3"))
        .unwrap_or(false);

    // 1. Load as ScratchGraph Project.
    let (project, assets) = if is_sb3 {
        let archive = Sb3Archive::from_path(input)?;
        let assets = archive.assets.clone();
        let project = Sb3Reader::new().read(&archive)?;
        (project, Some(assets))
    } else {
        let json_str = fs::read_to_string(input).map_err(|e| e.to_string())?;
        let value: serde_json::Value =
            serde_json::from_str(&json_str).map_err(|e| e.to_string())?;
        let project = parse_project_json(&value).map_err(|e| e.to_string())?;
        (project, None)
    };

    // 2. Parse pass list and run transforms.
    let pass_names = parse_pass_list(passes)?;
    let mut manager = PassManager::from_pass_names(&pass_names);
    let mut project = project;
    let report_obj = manager.run(&mut project);

    // 3. Determine output path and write.
    let output_path = output.clone().unwrap_or_else(|| {
        let stem = input_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        let ext = input_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("sb3");
        if ext.eq_ignore_ascii_case("json") {
            format!("{}_opt.json", stem)
        } else {
            format!("{}_opt.sb3", stem)
        }
    });

    if output_path.to_ascii_lowercase().ends_with(".sb3") {
        let archive = Sb3Writer::new().write_with_assets(
            &project,
            assets.unwrap_or_default(),
        )?;
        archive.to_path(&output_path)?;
    } else {
        let json = JsonExporter::new()
            .export(&project)
            .map_err(|_| "JSON export failed".to_string())?;
        let json_str = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
        fs::write(&output_path, &json_str).map_err(|e| e.to_string())?;
    }

    // 4. Emit report.
    let report_text = match format.to_ascii_lowercase().as_str() {
        "json" => report_obj.to_json().to_string(),
        _ => report_obj.to_text(),
    };
    if let Some(report_path) = report {
        fs::write(report_path, &report_text).map_err(|e| e.to_string())?;
    } else {
        println!("{}", report_text);
    }

    println!("wrote {}", output_path);
    Ok(())
}
