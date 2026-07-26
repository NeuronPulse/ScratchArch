use std::fs;

use scratcharch_scratchgraph::ir::Project;

pub fn run(input: &str, output: &Option<String>) -> Result<(), String> {
    let json_str = fs::read_to_string(input).map_err(|e| e.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| e.to_string())?;
    let project = scratcharch_scratchgraph::parse_project_json(&value).map_err(|e| e.to_string())?;

    let output_path = output.clone().unwrap_or_else(|| input.replace(".json", ".sair"));
    let sair_text = project_to_sair_text(&project);
    fs::write(&output_path, &sair_text).map_err(|e| e.to_string())?;
    println!("wrote {} ({})", output_path, summary(&project));
    Ok(())
}

fn summary(project: &Project) -> String {
    let script_count = project.stage.scripts.len()
        + project.sprites.iter().map(|s| s.scripts.len()).sum::<usize>();
    let proc_count = project.stage.procedures.len()
        + project.sprites.iter().map(|s| s.procedures.len()).sum::<usize>();
    format!("{} scripts, {} procedures", script_count, proc_count)
}

fn project_to_sair_text(project: &Project) -> String {
    let mut out = String::new();
    out.push_str("; decompiled from Scratch project\n");
    out.push_str(&format!("; stage: {}\n", project.stage.name));

    let script_count = project.stage.scripts.len()
        + project.sprites.iter().map(|s| s.scripts.len()).sum::<usize>();
    let var_count = project.stage.variables.len()
        + project.sprites.iter().map(|s| s.variables.len()).sum::<usize>();
    let list_count = project.stage.lists.len()
        + project.sprites.iter().map(|s| s.lists.len()).sum::<usize>();
    let proc_count = project.stage.procedures.len()
        + project.sprites.iter().map(|s| s.procedures.len()).sum::<usize>();

    out.push_str(&format!("; variables: {}\n", var_count));
    out.push_str(&format!("; lists: {}\n", list_count));
    out.push_str(&format!("; scripts: {}\n", script_count));
    out.push_str(&format!("; procedures: {}\n", proc_count));
    out.push('\n');

    for (_ti, _target_name, procedures) in all_procedures(project) {
        for proc in procedures {
            out.push_str(&format!("func @{}({}) {{\n", proc.prototype.name,
                proc.prototype.params.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", ")));
            out.push_str(&format!("  frame_size: {}\n", proc.frame_size));
            for stmt in &proc.body {
                let line = format!("  {:?}\n", stmt);
                out.push_str(&line);
            }
            out.push_str("}\n\n");
        }
    }

    out
}

fn all_procedures(project: &Project) -> Vec<(usize, String, Vec<scratcharch_scratchgraph::ir::Procedure>)> {
    let mut result = Vec::new();
    result.push((0, project.stage.name.clone(), project.stage.procedures.clone()));
    for (i, sprite) in project.sprites.iter().enumerate() {
        result.push((i + 1, sprite.name.clone(), sprite.procedures.clone()));
    }
    result
}
