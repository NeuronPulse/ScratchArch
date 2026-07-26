use serde::Serialize;

use scratcharch_ir::module::IrModule;

/// Summary of a single SAIR function for exploration tools.
#[derive(Debug, Clone, Serialize)]
pub struct SairFunctionSummary {
    pub name: String,
    pub return_type: String,
    pub param_count: usize,
    pub value_count: usize,
    pub block_count: usize,
    pub instruction_count: usize,
    pub blocks: Vec<SairBlockSummary>,
}

/// Summary of a single SAIR basic block.
#[derive(Debug, Clone, Serialize)]
pub struct SairBlockSummary {
    pub label: String,
    pub instruction_count: usize,
    pub terminator: String,
    pub source_location: Option<String>,
}

/// Summary of an SAIR module for exploration tools.
#[derive(Debug, Clone, Serialize)]
pub struct SairModuleSummary {
    pub entry: String,
    pub function_count: usize,
    pub total_instructions: usize,
    pub total_blocks: usize,
    pub functions: Vec<SairFunctionSummary>,
}

/// Explorer for SAIR modules.
#[derive(Default)]
pub struct SairExplorer;

impl SairExplorer {
    pub fn new() -> Self {
        Self
    }

    /// Produce a summary of an entire module.
    pub fn explore_module(&self, module: &IrModule) -> SairModuleSummary {
        let mut total_instructions = 0;
        let mut total_blocks = 0;
        let functions: Vec<SairFunctionSummary> = module
            .functions
            .iter()
            .map(|func| {
                let mut func_instructions = 0;
                let blocks: Vec<SairBlockSummary> = func
                    .blocks
                    .iter()
                    .map(|block| {
                        let count = block.instructions.len();
                        func_instructions += count;
                        let loc = block
                            .debug_locs
                            .iter()
                            .find_map(|d| d.as_ref())
                            .map(|l| format!("{}:{}", l.file, l.line));
                        SairBlockSummary {
                            label: block.label.clone(),
                            instruction_count: count,
                            terminator: format!("{:?}", block.terminator),
                            source_location: loc,
                        }
                    })
                    .collect();
                total_instructions += func_instructions;
                total_blocks += blocks.len();
                SairFunctionSummary {
                    name: func.name.clone(),
                    return_type: format!("{}", func.return_ty),
                    param_count: func.params.len(),
                    value_count: func.values.len(),
                    block_count: blocks.len(),
                    instruction_count: func_instructions,
                    blocks,
                }
            })
            .collect();

        SairModuleSummary {
            entry: module.entry.clone(),
            function_count: module.functions.len(),
            total_instructions,
            total_blocks,
            functions,
        }
    }

    /// Return a human-readable text report.
    pub fn to_text(&self, module: &IrModule) -> String {
        let summary = self.explore_module(module);
        let mut out = String::new();
        out.push_str(&format!("SAIR Module (entry: {})\n", summary.entry));
        out.push_str(&format!("Functions: {}\n", summary.function_count));
        out.push_str(&format!("Total blocks: {}\n", summary.total_blocks));
        out.push_str(&format!("Total instructions: {}\n\n", summary.total_instructions));

        for func in &summary.functions {
            out.push_str(&format!("  @{} -> {} ({} params, {} values)\n",
                func.name, func.return_type, func.param_count, func.value_count));
            out.push_str(&format!("    blocks: {}, instructions: {}\n",
                func.block_count, func.instruction_count));
            for block in &func.blocks {
                out.push_str(&format!("      {}: {} instrs, term={}",
                    block.label, block.instruction_count, block.terminator));
                if let Some(ref loc) = block.source_location {
                    out.push_str(&format!(" [{}]", loc));
                }
                out.push('\n');
            }
        }
        out
    }

    /// Return a JSON representation.
    pub fn to_json(&self, module: &IrModule) -> Result<String, serde_json::Error> {
        let summary = self.explore_module(module);
        serde_json::to_string_pretty(&summary)
    }
}
