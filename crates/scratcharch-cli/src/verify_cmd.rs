use std::fs;
use std::path::Path;

use scratcharch_sb3::{Sb3Archive, Sb3Reader};
use scratcharch_scratchgraph::{parse_project_json, Project};
use scratcharch_validation::verify_project;

/// Load a project from `.sb3` (archive) or `.json` (project.json).
fn load_project(input: &str) -> Result<Project, String> {
    let input_path = Path::new(input);
    let is_sb3 = input_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("sb3"))
        .unwrap_or(false);
    if is_sb3 {
        let archive = Sb3Archive::from_path(input)?;
        Sb3Reader::new().read(&archive)
    } else {
        let json_str = fs::read_to_string(input).map_err(|e| e.to_string())?;
        let value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| e.to_string())?;
        parse_project_json(&value).map_err(|e| e.to_string())
    }
}

/// `scratcharch verify <project.sb3>`.
///
/// Verifies a project end-to-end: parse, graph validation, `.sb3` roundtrip,
/// semantic preservation, and per-pass transform preservation. Exits
/// non-zero if any check reports FAIL.
pub fn run(input: &str, json: bool) -> Result<(), String> {
    let project = match load_project(input) {
        Ok(p) => p,
        Err(e) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "parse_ok": false, "error": e })
                );
            } else {
                println!("Parse: FAIL\n  {e}");
            }
            return Err(format!("parse failed: {e}"));
        }
    };

    let report = verify_project(&project);
    if json {
        let mut value = report.to_json();
        if let serde_json::Value::Object(map) = &mut value {
            map.insert("parse_ok".into(), serde_json::Value::Bool(true));
        }
        println!("{}", serde_json::to_string_pretty(&value).unwrap_or_else(|_| "null".into()));
    } else {
        print!("{}", report.to_text());
    }

    if report.passed() {
        Ok(())
    } else {
        Err("verification failed: one or more checks reported FAIL".into())
    }
}
