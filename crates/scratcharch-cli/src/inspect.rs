use std::fs;

use scratcharch_analyzer::{
    CallGraphAnalysis, ReachabilityAnalysis, VariableUsageAnalyzer,
};

pub fn run(input: &str) -> Result<(), String> {
    let json_str = fs::read_to_string(input).map_err(|e| e.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| e.to_string())?;
    let project = scratcharch_scratchgraph::parse_project_json(&value).map_err(|e| e.to_string())?;

    let callgraph = CallGraphAnalysis::new().analyze(&project);
    let reachable = ReachabilityAnalysis::new().analyze(&project);
    let usage = VariableUsageAnalyzer::new().analyze(&project);

    println!("Project: {}", project.stage.name);
    println!("Sprites: {}", project.sprites.len());
    println!();

    println!("--- Stage ---");
    println!("  scripts: {}", project.stage.scripts.len());
    println!("  procedures: {}", project.stage.procedures.len());
    println!("  variables: {}", project.stage.variables.len());
    println!("  lists: {}", project.stage.lists.len());
    println!("  broadcasts: {}", project.stage.broadcasts.len());

    for sprite in &project.sprites {
        println!();
        println!("--- Sprite: {} ---", sprite.name);
        println!("  scripts: {}", sprite.scripts.len());
        println!("  procedures: {}", sprite.procedures.len());
        println!("  variables: {}", sprite.variables.len());
        println!("  lists: {}", sprite.lists.len());
    }

    if !callgraph.edges.is_empty() {
        println!();
        println!("--- Call Graph ---");
        println!("  procedures: {}", callgraph.nodes.len());
        println!("  calls: {}", callgraph.edges.len());
        for edge in &callgraph.edges {
            let suffix = if callgraph.is_recursive(&edge.callee) { " (recursive)" } else { "" };
            println!("    {} -> {}{}", edge.caller, edge.callee, suffix);
        }
    }

    if !reachable.is_empty() {
        println!();
        println!("--- Unreachable Scripts ---");
        for u in &reachable {
            println!("  {}: {:?}", u.target_name, u.hat);
        }
    }

    let dead = usage.dead_variables();
    if !dead.is_empty() {
        println!();
        println!("--- Dead Variables ---");
        for v in &dead {
            println!("  {} (written but never read)", v);
        }
    }

    println!();
    println!("--- Recursion ---");
    let has_recursion = callgraph.recursive.keys().any(|_| true);
    if has_recursion {
        for (name, kind) in &callgraph.recursive {
            println!("  {:?}: {}", kind, name);
        }
    } else {
        println!("  none");
    }

    Ok(())
}
