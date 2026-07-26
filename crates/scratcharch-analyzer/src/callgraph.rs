//! Procedure call-graph analysis for ScratchGraph projects.
//!
//! Builds a directed graph where nodes are procedures and edges represent
//! `Stmt::Call` sites. The analysis detects direct and mutual recursion.

use std::collections::{HashMap, HashSet};

use scratcharch_scratchgraph::ir::{Procedure, Project, Sprite, Stage, Stmt};

/// Kind of recursion detected in the call graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecursionKind {
    /// A procedure calls itself directly.
    Direct,
    /// Two or more procedures form a cycle.
    Mutual,
}

/// Edge in the call graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallEdge {
    pub caller: String,
    pub callee: String,
}

/// Call graph of a ScratchGraph project.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CallGraph {
    /// All procedure names known to the project.
    pub nodes: HashSet<String>,
    /// Directed call edges.
    pub edges: Vec<CallEdge>,
    /// Procedures involved in recursion, mapped to the kind of recursion.
    pub recursive: HashMap<String, RecursionKind>,
}

impl CallGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the set of callees for a given caller.
    pub fn callees(&self, caller: &str) -> Vec<String> {
        self.edges
            .iter()
            .filter(|e| e.caller == caller)
            .map(|e| e.callee.clone())
            .collect()
    }

    /// Return true if the named procedure is recursive.
    pub fn is_recursive(&self, name: &str) -> bool {
        self.recursive.contains_key(name)
    }
}

/// Call-graph analysis.
#[derive(Debug, Clone, Default)]
pub struct CallGraphAnalysis;

impl CallGraphAnalysis {
    pub fn new() -> Self {
        Self
    }

    /// Build a call graph from a complete project.
    pub fn analyze(&self, project: &Project) -> CallGraph {
        let mut graph = CallGraph::new();

        // Collect all procedure names.
        for proc in &project.stage.procedures {
            graph.nodes.insert(proc.prototype.name.clone());
        }
        for sprite in &project.sprites {
            for proc in &sprite.procedures {
                graph.nodes.insert(proc.prototype.name.clone());
            }
        }

        // Add edges from stage scripts and procedures.
        self.add_stage_edges(&project.stage, &mut graph);
        for sprite in &project.sprites {
            self.add_sprite_edges(sprite, &mut graph);
        }

        // Detect recursion via DFS.
        self.detect_recursion(&mut graph);

        graph
    }

    fn add_stage_edges(&self, stage: &Stage, graph: &mut CallGraph) {
        for script in &stage.scripts {
            self.collect_calls(&script.entry.body, "<script>", graph);
        }
        for proc in &stage.procedures {
            self.collect_calls(&proc.body, &proc.prototype.name, graph);
        }
    }

    fn add_sprite_edges(&self, sprite: &Sprite, graph: &mut CallGraph) {
        for script in &sprite.scripts {
            self.collect_calls(&script.entry.body, "<script>", graph);
        }
        for proc in &sprite.procedures {
            self.collect_calls(&proc.body, &proc.prototype.name, graph);
        }
    }

    fn collect_calls(&self, body: &[Stmt], caller: &str, graph: &mut CallGraph) {
        for stmt in body {
            match stmt {
                Stmt::Call { proc, .. } => {
                    graph.edges.push(CallEdge {
                        caller: caller.to_string(),
                        callee: proc.clone(),
                    });
                }
                Stmt::If { then_body, else_body, .. } => {
                    self.collect_calls(then_body, caller, graph);
                    self.collect_calls(else_body, caller, graph);
                }
                Stmt::Repeat { body: b, .. }
                | Stmt::RepeatUntil { body: b, .. }
                | Stmt::Forever { body: b } => {
                    self.collect_calls(b, caller, graph);
                }
                _ => {}
            }
        }
    }

    fn detect_recursion(&self, graph: &mut CallGraph) {
        let adjacency: HashMap<String, HashSet<String>> = graph
            .nodes
            .iter()
            .map(|n| (n.clone(), HashSet::new()))
            .collect();
        let mut adjacency = adjacency;
        for edge in &graph.edges {
            if edge.caller != "<script>" {
                adjacency
                    .entry(edge.caller.clone())
                    .or_default()
                    .insert(edge.callee.clone());
            }
        }

        for node in &graph.nodes {
            if adjacency.get(node).map(|s| s.contains(node)).unwrap_or(false) {
                graph.recursive.insert(node.clone(), RecursionKind::Direct);
            }
        }

        // Simple cycle detection for mutual recursion (length 2 cycles).
        for a in &graph.nodes {
            for b in adjacency.get(a).unwrap_or(&HashSet::new()).clone() {
                if a != &b && adjacency.get(&b).map(|s| s.contains(a)).unwrap_or(false) {
                    graph.recursive.entry(a.clone()).or_insert(RecursionKind::Mutual);
                    graph.recursive.entry(b.clone()).or_insert(RecursionKind::Mutual);
                }
            }
        }
    }
}

/// Convenience: build a call graph for a list of procedures (ignoring scripts).
pub fn call_graph_for_procedures(procedures: &[Procedure]) -> CallGraph {
    let mut graph = CallGraph::new();
    for proc in procedures {
        graph.nodes.insert(proc.prototype.name.clone());
    }
    let analysis = CallGraphAnalysis::new();
    for proc in procedures {
        analysis.collect_calls(&proc.body, &proc.prototype.name, &mut graph);
    }
    analysis.detect_recursion(&mut graph);
    graph
}
