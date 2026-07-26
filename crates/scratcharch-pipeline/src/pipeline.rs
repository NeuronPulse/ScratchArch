use std::collections::HashMap;
use std::path::{Path, PathBuf};

use scratcharch_opt::manager::PassManager;
use scratcharch_opt::{cfg_simplify::CfgSimplify, constant_fold::ConstantFold, dce::DeadCodeElimination};
use scratcharch_scratchgraph::{JsonExporter, ScratchExporter, ScratchGraphLowerer};

use crate::decompile::Decompiler;
use crate::diagnostic::{Diagnostic, DiagnosticSink};

/// Identifies which stage of the pipeline is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompilationStage {
    Input,
    LlvmParse,
    SairModule,
    SairOptimize,
    ScratchGraphLower,
    ScratchGraphExport,
    ScratchGraphParse,
    Decompile,
    DumpOutput,
}

impl CompilationStage {
    pub fn name(&self) -> &'static str {
        match self {
            CompilationStage::Input => "input",
            CompilationStage::LlvmParse => "llvm-parse",
            CompilationStage::SairModule => "sair-module",
            CompilationStage::SairOptimize => "sair-optimize",
            CompilationStage::ScratchGraphLower => "scratchgraph-lower",
            CompilationStage::ScratchGraphExport => "scratchgraph-export",
            CompilationStage::ScratchGraphParse => "scratchgraph-parse",
            CompilationStage::Decompile => "decompile",
            CompilationStage::DumpOutput => "dump-output",
        }
    }
}

/// Configuration for a pipeline run.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Optimization level (none, basic, aggressive).
    pub opt_level: String,
    /// If set, dump intermediate files to this directory.
    pub dump_dir: Option<PathBuf>,
    /// Enable optimization passes.
    pub optimize: bool,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        PipelineConfig {
            opt_level: "basic".to_string(),
            dump_dir: None,
            optimize: true,
        }
    }
}

/// The result of a pipeline run.
#[derive(Debug)]
pub struct PipelineResult {
    /// Final output value (project.json Value or SAIR text).
    pub output: PipelineOutput,
    /// Intermediate files produced during --dump.
    pub intermediates: HashMap<String, String>,
    /// Diagnostics collected during the run.
    pub diagnostics: DiagnosticSink,
}

#[derive(Debug)]
pub enum PipelineOutput {
    Json(serde_json::Value),
    SairText(String),
}

/// Unified compiler pipeline.
///
/// Orchestrates the full ScratchArch pipeline without the caller needing to
/// know which crates are involved:
///
/// ```text
/// LLVM IR → SAIR → Optimize → ScratchGraph → project.json
/// project.json → ScratchGraph → SAIR
/// ```
pub struct Pipeline {
    config: PipelineConfig,
    diagnostics: DiagnosticSink,
}

impl Pipeline {
    pub fn new(config: PipelineConfig) -> Self {
        Pipeline {
            config,
            diagnostics: DiagnosticSink::new(),
        }
    }

    /// Run the full LLVM IR → project.json pipeline.
    pub fn run_llvm_to_json(&mut self, llvm_text: &str) -> Result<PipelineResult, Vec<Diagnostic>> {
        let mut intermediates = HashMap::new();

        // Stage 1: Parse LLVM IR → SAIR
        let module = match scratcharch_llvm::translate_llvm(llvm_text) {
            Ok(m) => m,
            Err(e) => {
                self.diagnostics
                    .push_error(format!("LLVM parse failed: {}", e));
                return Err(std::mem::take(&mut self.diagnostics).diagnostics);
            }
        };
        if let Some(dir) = &self.config.dump_dir {
            let sair_text = scratcharch_ir::text::serialize(&module);
            let path = dir.join("stage2-sair.txt");
            let _ = std::fs::write(&path, &sair_text);
            intermediates.insert("sair".to_string(), sair_text);
        }

        // Stage 2: Validate
        if let Err(e) = module.validate() {
            self.diagnostics.push_error(format!("SAIR validation: {}", e));
            return Err(std::mem::take(&mut self.diagnostics).diagnostics);
        }

        // Stage 3: Optimize
        let mut module = module;
        if self.config.optimize {
            let mut pm = build_pass_manager(&self.config.opt_level);
            pm.run(&mut module);
        }
        if let Some(dir) = &self.config.dump_dir {
            let opt_text = scratcharch_ir::text::serialize(&module);
            let path = dir.join("stage3-optimized.sair");
            let _ = std::fs::write(&path, &opt_text);
            intermediates.insert("optimized-sair".to_string(), opt_text);
        }

        // Stage 4: Lower SAIR → ScratchGraph
        let lowerer = ScratchGraphLowerer::new();
        let project = match lowerer.lower(&module) {
            Ok(p) => p,
            Err(e) => {
                self.diagnostics.push_error(format!("ScratchGraph lower: {}", e));
                return Err(std::mem::take(&mut self.diagnostics).diagnostics);
            }
        };
        if let Some(dir) = &self.config.dump_dir {
            let sg_json = serde_json::to_string_pretty(
                &scratcharch_explorer::scratch::ScratchExplorer::new().explore_project(&project),
            )
            .unwrap_or_default();
            let path = dir.join("stage1-scratchgraph.json");
            let _ = std::fs::write(&path, &sg_json);
            intermediates.insert("scratchgraph".to_string(), sg_json);
        }

        // Stage 5: Export project.json
        let json = JsonExporter::new().export(&project).map_err(|_| {
            vec![Diagnostic::error("JSON export failed")]
        })?;

        self.diagnostics.push_note(format!(
            "Pipeline complete: {} procedures, {} scripts",
            project.stage.procedures.len(),
            project.stage.scripts.len(),
        ));

        Ok(PipelineResult {
            output: PipelineOutput::Json(json),
            intermediates,
            diagnostics: std::mem::take(&mut self.diagnostics),
        })
    }

    /// Run the project.json → project.json roundtrip pipeline.
    pub fn run_json_roundtrip(&mut self, json: &serde_json::Value) -> Result<PipelineResult, Vec<Diagnostic>> {
        let mut intermediates = HashMap::new();

        // Parse project.json → ScratchGraph
        let project = match scratcharch_scratchgraph::parse_project_json(json) {
            Ok(p) => p,
            Err(e) => {
                self.diagnostics.push_error(format!("Parse failed: {}", e));
                return Err(std::mem::take(&mut self.diagnostics).diagnostics);
            }
        };
        if let Some(dir) = &self.config.dump_dir {
            let sg_json = serde_json::to_string_pretty(
                &scratcharch_explorer::scratch::ScratchExplorer::new().explore_project(&project),
            )
            .unwrap_or_default();
            let path = dir.join("stage1-scratchgraph.json");
            let _ = std::fs::write(&path, &sg_json);
            intermediates.insert("scratchgraph".to_string(), sg_json);
        }

        // Export ScratchGraph → project.json
        let output = JsonExporter::new().export(&project).map_err(|_| {
            vec![Diagnostic::error("JSON export failed")]
        })?;

        self.diagnostics.push_note(format!(
            "Roundtrip complete: {} procedures, {} scripts",
            project.stage.procedures.len(),
            project.stage.scripts.len(),
        ));

        Ok(PipelineResult {
            output: PipelineOutput::Json(output),
            intermediates,
            diagnostics: std::mem::take(&mut self.diagnostics),
        })
    }

    /// Run the project.json → SAIR decompile pipeline.
    pub fn run_decompile(&mut self, json: &serde_json::Value) -> Result<PipelineResult, Vec<Diagnostic>> {
        let mut intermediates = HashMap::new();

        // Parse project.json → ScratchGraph
        let project = match scratcharch_scratchgraph::parse_project_json(json) {
            Ok(p) => p,
            Err(e) => {
                self.diagnostics.push_error(format!("Parse failed: {}", e));
                return Err(std::mem::take(&mut self.diagnostics).diagnostics);
            }
        };
        if let Some(dir) = &self.config.dump_dir {
            let sg_json = serde_json::to_string_pretty(
                &scratcharch_explorer::scratch::ScratchExplorer::new().explore_project(&project),
            )
            .unwrap_or_default();
            let path = dir.join("stage1-scratchgraph.json");
            let _ = std::fs::write(&path, &sg_json);
            intermediates.insert("scratchgraph".to_string(), sg_json);
        }

        // Decompile ScratchGraph → SAIR
        let decompiler = Decompiler::new();
        let module = match decompiler.decompile(&project) {
            Ok(m) => m,
            Err(e) => {
                self.diagnostics.push_error(format!("Decompile: {}", e));
                return Err(std::mem::take(&mut self.diagnostics).diagnostics);
            }
        };

        let sair_text = scratcharch_ir::text::serialize(&module);
        if let Some(dir) = &self.config.dump_dir {
            let path = dir.join("stage2-sair.txt");
            let _ = std::fs::write(&path, &sair_text);
            intermediates.insert("sair".to_string(), sair_text.clone());
        }

        self.diagnostics.push_note(format!(
            "Decompile complete: {} functions produced",
            module.functions.len(),
        ));

        Ok(PipelineResult {
            output: PipelineOutput::SairText(sair_text),
            intermediates,
            diagnostics: std::mem::take(&mut self.diagnostics),
        })
    }

    /// Run the full LLVM IR → ScratchGraph → project.json pipeline from a file.
    pub fn run_file(&mut self, input: &Path) -> Result<PipelineResult, Vec<Diagnostic>> {
        let content = std::fs::read_to_string(input)
            .map_err(|e| vec![Diagnostic::error(format!("Cannot read {}: {}", input.display(), e))])?;

        match input.extension().and_then(|e| e.to_str()) {
            Some("ll") => self.run_llvm_to_json(&content),
            _ => {
                // Try as JSON
                let value: serde_json::Value = serde_json::from_str(&content)
                    .map_err(|e| vec![Diagnostic::error(format!("JSON parse: {}", e))])?;
                self.run_decompile(&value)
            }
        }
    }
}

fn build_pass_manager(opt_level: &str) -> PassManager {
    let mut pm = PassManager::new();
    match opt_level {
        "none" => {}
        "basic" => {
            pm.add(ConstantFold);
            pm.add(DeadCodeElimination);
        }
        "aggressive" => {
            pm.add(ConstantFold);
            pm.add(DeadCodeElimination);
            pm.add(CfgSimplify);
            pm.add(ConstantFold);
            pm.add(DeadCodeElimination);
        }
        _ => {}
    }
    pm
}
