use std::collections::HashMap;

use scratcharch_scratchgraph::ir::Project;
use serde::Serialize;

use crate::callgraph::{CallGraph, RecursionKind};
use crate::metrics::ComplexityMetrics;
use crate::reachability::UnreachableScript;
use crate::variables::VariableUsageAnalysis;

#[derive(Debug, Clone, Serialize)]
pub struct AnalysisReport {
    pub project_name: String,
    pub sprite_count: usize,
    pub metrics: MetricsSummary,
    pub call_graph: CallGraphSummary,
    pub unreachable_scripts: Vec<UnreachableEntry>,
    pub variable_usage: HashMap<String, VarUsageEntry>,
    pub lists: HashMap<String, VarUsageEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricsSummary {
    pub scripts: usize,
    pub procedures: usize,
    pub total_statements: usize,
    pub max_nesting_depth: usize,
    pub variables: usize,
    pub lists: usize,
    pub calls: usize,
    pub broadcasts: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CallGraphSummary {
    pub procedures: Vec<String>,
    pub edges: Vec<String>,
    pub direct_recursion: Vec<String>,
    pub mutual_recursion: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnreachableEntry {
    pub target: String,
    pub hat: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct VarUsageEntry {
    pub reads: usize,
    pub writes: usize,
}

impl Report {
    pub fn build(
        project: &Project,
        callgraph: &CallGraph,
        unreachable: &[UnreachableScript],
        usage: &VariableUsageAnalysis,
    ) -> AnalysisReport {
        let metrics = ComplexityMetrics::new(project);

        let direct_recursion: Vec<String> = callgraph
            .recursive
            .iter()
            .filter(|(_, k)| matches!(k, RecursionKind::Direct))
            .map(|(n, _)| n.clone())
            .collect();

        let edges: Vec<String> = callgraph
            .edges
            .iter()
            .map(|e| format!("{} -> {}", e.caller, e.callee))
            .collect();

        AnalysisReport {
            project_name: project.stage.name.clone(),
            sprite_count: project.sprites.len(),
            metrics: MetricsSummary {
                scripts: metrics.script_count,
                procedures: metrics.procedure_count,
                total_statements: metrics.total_statements,
                max_nesting_depth: metrics.max_nesting_depth,
                variables: metrics.variable_count,
                lists: metrics.list_count,
                calls: metrics.call_count,
                broadcasts: metrics.broadcast_count,
            },
            call_graph: CallGraphSummary {
                procedures: callgraph.nodes.iter().cloned().collect(),
                edges,
                direct_recursion,
                mutual_recursion: Vec::new(),
            },
            unreachable_scripts: unreachable
                .iter()
                .map(|u| UnreachableEntry {
                    target: u.target_name.clone(),
                    hat: format!("{:?}", u.hat),
                })
                .collect(),
            variable_usage: usage
                .variables
                .iter()
                .map(|(k, v)| {
                    (k.clone(), VarUsageEntry {
                        reads: v.reads,
                        writes: v.writes,
                    })
                })
                .collect(),
            lists: usage
                .lists
                .iter()
                .map(|(k, v)| {
                    (k.clone(), VarUsageEntry {
                        reads: v.reads,
                        writes: v.writes,
                    })
                })
                .collect(),
        }
    }
}

pub struct Report;

impl AnalysisReport {
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("Analysis Report: {}\n", self.project_name));
        out.push_str(&format!("Sprites: {}\n", self.sprite_count));
        out.push('\n');
        out.push_str("--- Metrics ---\n");
        out.push_str(&format!("  Scripts: {}\n", self.metrics.scripts));
        out.push_str(&format!("  Procedures: {}\n", self.metrics.procedures));
        out.push_str(&format!("  Total statements: {}\n", self.metrics.total_statements));
        out.push_str(&format!("  Max nesting depth: {}\n", self.metrics.max_nesting_depth));
        out.push_str(&format!("  Variables: {}\n", self.metrics.variables));
        out.push_str(&format!("  Lists: {}\n", self.metrics.lists));
        out.push_str(&format!("  Calls: {}\n", self.metrics.calls));
        out.push_str(&format!("  Broadcasts: {}\n", self.metrics.broadcasts));

        if !self.call_graph.edges.is_empty() {
            out.push_str("\n--- Call Graph ---\n");
            for edge in &self.call_graph.edges {
                out.push_str(&format!("  {}\n", edge));
            }
        }

        if !self.call_graph.direct_recursion.is_empty() {
            out.push_str("\n--- Direct Recursion ---\n");
            for name in &self.call_graph.direct_recursion {
                out.push_str(&format!("  {}\n", name));
            }
        }

        if !self.unreachable_scripts.is_empty() {
            out.push_str("\n--- Unreachable Scripts ---\n");
            for u in &self.unreachable_scripts {
                out.push_str(&format!("  {}: {}\n", u.target, u.hat));
            }
        }

        let mut dead: Vec<_> = self.variable_usage.iter()
            .filter(|(_, v)| v.writes > 0 && v.reads == 0)
            .map(|(k, _)| k.clone())
            .collect();
        dead.sort();
        if !dead.is_empty() {
            out.push_str("\n--- Dead Variables ---\n");
            for v in &dead {
                out.push_str(&format!("  {} (written but never read)\n", v));
            }
        }
        out
    }
}
