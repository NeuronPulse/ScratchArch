use std::fs;
use std::path::PathBuf;

use scratcharch_pipeline::{DiagnosticSink, Pipeline, PipelineConfig, PipelineOutput};

pub fn run(
    input: &str,
    output: &Option<String>,
    dump: &Option<String>,
    mode: &str,
) -> Result<(), String> {
    let raw = fs::read_to_string(input).map_err(|e| e.to_string())?;

    let dump_dir = dump.as_ref().map(|d| {
        let dir = PathBuf::from(d);
        let _ = fs::create_dir_all(&dir);
        // Write input file
        let ext = std::path::Path::new(input)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("txt");
        let input_path = dir.join(format!("stage0-input.{}", ext));
        let _ = fs::write(&input_path, &raw);
        dir
    });

    let config = PipelineConfig {
        dump_dir,
        ..PipelineConfig::default()
    };

    let mut pipeline = Pipeline::new(config);
    let result = match mode {
        "roundtrip" => {
            let value: serde_json::Value =
                serde_json::from_str(&raw).map_err(|e| e.to_string())?;
            pipeline.run_json_roundtrip(&value)
        }
        "decompile" => {
            let value: serde_json::Value =
                serde_json::from_str(&raw).map_err(|e| e.to_string())?;
            pipeline.run_decompile(&value)
        }
        _ => {
            // "compile" mode: auto-detect based on extension
            let path = std::path::Path::new(input);
            match path.extension().and_then(|e| e.to_str()) {
                Some("ll") => pipeline.run_llvm_to_json(&raw),
                _ => {
                    let value: serde_json::Value =
                        serde_json::from_str(&raw).map_err(|e| e.to_string())?;
                    pipeline.run_json_roundtrip(&value)
                }
            }
        }
    }
    .map_err(|diags| {
        let sink = DiagnosticSink { diagnostics: diags };
        sink.to_text()
    })?;

    // Write final output
    let output_path = output.clone().unwrap_or_else(|| {
        let stem = std::path::Path::new(input)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        match result.output {
            PipelineOutput::Json(_) => format!("{}.json", stem),
            PipelineOutput::SairText(_) => format!("{}.sair", stem),
        }
    });

    match &result.output {
        PipelineOutput::Json(value) => {
            let json_str = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
            fs::write(&output_path, &json_str).map_err(|e| e.to_string())?;
        }
        PipelineOutput::SairText(text) => {
            fs::write(&output_path, text).map_err(|e| e.to_string())?;
        }
    }

    // Print diagnostics
    if !result.diagnostics.diagnostics.is_empty() {
        eprintln!("{}", result.diagnostics.to_text());
    }

    println!("wrote {}", output_path);
    Ok(())
}
