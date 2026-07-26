use scratcharch_scratchgraph::debug::SourceLocation;

use crate::callgraph::CallGraph;
use crate::cfg::ControlFlowGraph;

pub trait DotOutput {
    fn to_dot(&self, name: &str) -> String;
}

pub trait DotOutputRich: DotOutput {
    fn to_dot_with_metadata(&self, name: &str, source_map: &[SourceLocation]) -> String;
}

/// Enhanced DOT metadata for a CFG node.
pub struct CfgNodeMeta {
    pub block_id_prefix: Option<String>,
    pub statement_count: usize,
}

impl ControlFlowGraph {
    /// Compute node metadata for DOT enrichment.
    pub fn node_metadata(&self) -> Vec<CfgNodeMeta> {
        self
            .nodes
            .iter()
            .map(|n| match n {
                crate::CfgNode::Entry | crate::CfgNode::Exit => CfgNodeMeta {
                    block_id_prefix: None,
                    statement_count: 0,
                },
                crate::CfgNode::Statement { body_index } => CfgNodeMeta {
                    block_id_prefix: Some(format!("s{}", body_index)),
                    statement_count: 1,
                },
            })
            .collect()
    }
}

impl DotOutput for ControlFlowGraph {
    fn to_dot(&self, name: &str) -> String {
        let mut lines = Vec::new();
        lines.push(format!("digraph {} {{", name));
        lines.push(format!("  label=\"CFG: {}\";", name));
        lines.push("  node [shape=box];".to_string());

        let meta = self.node_metadata();
        let node_labels: Vec<String> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let m = &meta[i];
                match n {
                    crate::CfgNode::Entry =>
                        format!("  n{} [label=\"Entry\", shape=oval, tooltip=\"entry point\"];", i),
                    crate::CfgNode::Exit =>
                        format!("  n{} [label=\"Exit\", shape=oval, tooltip=\"exit point\"];", i),
                    crate::CfgNode::Statement { body_index } => {
                        let prefix = m.block_id_prefix.as_deref().unwrap_or("");
                        format!(
                            "  n{} [label=\"Stmt {}\\n({})\", tooltip=\"body_index={} stmts={}\"];",
                            i, body_index, prefix, body_index, m.statement_count
                        )
                    }
                }
            })
            .collect();
        for l in &node_labels {
            lines.push(l.clone());
        }

        for (from, to, edge) in &self.edges {
            let label = match edge {
                crate::CfgEdge::Sequence => "",
                crate::CfgEdge::TrueBranch => "true",
                crate::CfgEdge::FalseBranch => "false",
                crate::CfgEdge::BackEdge => "back",
            };
            if label.is_empty() {
                lines.push(format!("  n{} -> n{};", from, to));
            } else {
                lines.push(format!("  n{} -> n{} [label=\"{}\"];", from, to, label));
            }
        }

        lines.push("}".to_string());
        lines.join("\n")
    }
}

impl DotOutput for CallGraph {
    fn to_dot(&self, name: &str) -> String {
        let mut lines = Vec::new();
        lines.push(format!("digraph {} {{", name));
        lines.push(format!("  label=\"Call Graph: {}\";", name));
        lines.push("  node [shape=box];".to_string());

        for node in &self.nodes {
            let color = if self.is_recursive(node) { " style=filled fillcolor=lightcoral" } else { "" };
            let tooltip = if self.is_recursive(node) {
                format!(" recursive: {:?}", self.recursive.get(node))
            } else {
                "non-recursive".to_string()
            };
            lines.push(format!(
                "  \"{}\" [label=\"{}\", tooltip=\"{}\"{}];",
                node, node, tooltip, color
            ));
        }

        for edge in &self.edges {
            let style = if edge.caller == edge.callee { " style=bold" } else { "" };
            lines.push(format!(
                "  \"{}\" -> \"{}\" [tooltip=\"{}\"{}];",
                edge.caller, edge.callee, edge.callee, style
            ));
        }

        lines.push("}".to_string());
        lines.join("\n")
    }
}
