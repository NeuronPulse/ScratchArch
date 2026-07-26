use crate::callgraph::CallGraph;
use crate::cfg::ControlFlowGraph;

pub trait DotOutput {
    fn to_dot(&self, name: &str) -> String;
}

impl DotOutput for ControlFlowGraph {
    fn to_dot(&self, name: &str) -> String {
        let mut lines = Vec::new();
        lines.push(format!("digraph {} {{", name));
        lines.push(format!("  label=\"CFG: {}\";", name));
        lines.push("  node [shape=box];".to_string());

        let node_labels: Vec<String> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| match n {
                crate::CfgNode::Entry => format!("  n{} [label=\"Entry\", shape=oval];", i),
                crate::CfgNode::Exit => format!("  n{} [label=\"Exit\", shape=oval];", i),
                crate::CfgNode::Statement { body_index } => {
                    format!("  n{} [label=\"Stmt {} \\n({:?})\"];", i, body_index, n)
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
            lines.push(format!("  \"{}\" [label=\"{}\"{}];", node, node, color));
        }

        for edge in &self.edges {
            let style = if edge.caller == edge.callee { " style=bold" } else { "" };
            lines.push(format!("  \"{}\" -> \"{}\"{};", edge.caller, edge.callee, style));
        }

        lines.push("}".to_string());
        lines.join("\n")
    }
}
