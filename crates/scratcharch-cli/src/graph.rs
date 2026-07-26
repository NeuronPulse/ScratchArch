use std::fs;

use scratcharch_analyzer::{dot::DotOutput, CallGraphAnalysis, CfgAnalysis};

pub fn run(input: &str, kind: &str, output: &Option<String>) -> Result<(), String> {
    let json_str = fs::read_to_string(input).map_err(|e| e.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| e.to_string())?;
    let project = scratcharch_scratchgraph::parse_project_json(&value).map_err(|e| e.to_string())?;

    let dot = match kind {
        "cfg" => {
            let mut dots = Vec::new();
            for script in &project.stage.scripts {
                let cfg = CfgAnalysis::new().build_script(&script.entry);
                dots.push(cfg.to_dot(&format!("cfg_stage_{}", script_name(script))));
            }
            for proc in &project.stage.procedures {
                let cfg = CfgAnalysis::new().build(&proc.body);
                dots.push(cfg.to_dot(&format!("cfg_proc_{}", proc.prototype.name)));
            }
            dots.join("\n")
        }
        "callgraph" => {
            let callgraph = CallGraphAnalysis::new().analyze(&project);
            callgraph.to_dot("callgraph")
        }
        _ => {
            let mut dots = Vec::new();
            dots.push("digraph project {".to_string());
            dots.push(format!("  label=\"ScratchGraph: {}\";", project.stage.name));
            dots.push("  node [shape=box];".to_string());
            for script in &project.stage.scripts {
                let label = format!("{:?}", script.entry.hat);
                dots.push(format!("  script_stage_{:?} [label=\"{}\"];", script.entry.hat, label));
            }
            for proc in &project.stage.procedures {
                dots.push(format!("  proc_stage_{} [label=\"proc: {}\"];", proc.prototype.name, proc.prototype.name));
            }
            for sprite in &project.sprites {
                for script in &sprite.scripts {
                    let label = format!("{:?}", script.entry.hat);
                    dots.push(format!("  script_{}_{:?} [label=\"{}\"];", sprite.name, script.entry.hat, label));
                }
                for proc in &sprite.procedures {
                    dots.push(format!("  proc_{}_{} [label=\"proc: {}\"];", sprite.name, proc.prototype.name, proc.prototype.name));
                }
            }
            dots.push("}".to_string());
            dots.join("\n")
        }
    };

    if let Some(path) = output {
        fs::write(path, &dot).map_err(|e| e.to_string())?;
        println!("wrote {}", path);
    } else {
        println!("{}", dot);
    }
    Ok(())
}

fn script_name(script: &scratcharch_scratchgraph::ir::Script) -> String {
    format!("{:?}", script.entry.hat).to_lowercase()
}
