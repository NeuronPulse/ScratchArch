use std::fs;

use scratcharch_analyzer::{
    CallGraphAnalysis, ReachabilityAnalysis, Report, VariableUsageAnalyzer,
};

pub fn run(input: &str, format: &str, output: &Option<String>) -> Result<(), String> {
    let json_str = fs::read_to_string(input).map_err(|e| e.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| e.to_string())?;
    let project = scratcharch_scratchgraph::parse_project_json(&value).map_err(|e| e.to_string())?;

    let callgraph = CallGraphAnalysis::new().analyze(&project);
    let reachable = ReachabilityAnalysis::new().analyze(&project);
    let usage = VariableUsageAnalyzer::new().analyze(&project);
    let report = Report::build(&project, &callgraph, &reachable, &usage);

    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
            if let Some(path) = output {
                fs::write(path, &json).map_err(|e| e.to_string())?;
            } else {
                println!("{}", json);
            }
        }
        _ => {
            let text = report.to_text();
            if let Some(path) = output {
                fs::write(path, &text).map_err(|e| e.to_string())?;
            } else {
                println!("{}", text);
            }
        }
    }
    Ok(())
}
