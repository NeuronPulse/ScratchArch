//! Control-flow graph construction for ScratchGraph statement sequences.
//!
//! ScratchGraph scripts and procedures are flat sequences of statements with
//! nested bodies inside `If`, `Repeat`, `RepeatUntil`, and `Forever`. The CFG
//! treats each statement as a node and adds edges for sequential flow and
//! control-flow branching.

use scratcharch_scratchgraph::ir::{Expr, ScriptEntry, StopOption, Stmt, Value as SgValue};

/// A node in a control-flow graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CfgNode {
    /// A concrete statement at the given body index.
    Statement { body_index: usize },
    /// Synthetic entry node.
    Entry,
    /// Synthetic exit node.
    Exit,
}

/// A directed edge in a control-flow graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CfgEdge {
    /// Sequential fall-through.
    Sequence,
    /// Branch taken when the condition is true.
    TrueBranch,
    /// Branch taken when the condition is false.
    FalseBranch,
    /// Loop back edge (end of loop body to loop header).
    BackEdge,
}

/// Control-flow graph for a single script or procedure body.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ControlFlowGraph {
    pub nodes: Vec<CfgNode>,
    pub edges: Vec<(usize, usize, CfgEdge)>,
}

impl ControlFlowGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the index of the entry node.
    pub fn entry_index(&self) -> usize {
        0
    }

    /// Return the index of the exit node.
    pub fn exit_index(&self) -> usize {
        1
    }

    /// Return all successor node indices of `node`.
    pub fn successors(&self, node: usize) -> Vec<usize> {
        self.edges
            .iter()
            .filter(|(from, _, _)| *from == node)
            .map(|(_, to, _)| *to)
            .collect()
    }

    /// Return all predecessor node indices of `node`.
    pub fn predecessors(&self, node: usize) -> Vec<usize> {
        self.edges
            .iter()
            .filter(|(_, to, _)| *to == node)
            .map(|(from, _, _)| *from)
            .collect()
    }
}

/// CFG construction analysis.
#[derive(Debug, Clone, Default)]
pub struct CfgAnalysis;

impl CfgAnalysis {
    pub fn new() -> Self {
        Self
    }

    /// Build a CFG for a script entry.
    pub fn build_script(&self, entry: &ScriptEntry) -> ControlFlowGraph {
        self.build(&entry.body)
    }

    /// Build a CFG for a flat statement body.
    pub fn build(&self, body: &[Stmt]) -> ControlFlowGraph {
        let mut graph = ControlFlowGraph::new();
        graph.nodes.push(CfgNode::Entry);
        graph.nodes.push(CfgNode::Exit);

        let offset = graph.nodes.len();
        for (i, _stmt) in body.iter().enumerate() {
            graph.nodes.push(CfgNode::Statement { body_index: i });
        }

        if body.is_empty() {
            graph.edges.push((graph.entry_index(), graph.exit_index(), CfgEdge::Sequence));
            return graph;
        }

        graph.edges.push((graph.entry_index(), offset, CfgEdge::Sequence));
        Self::connect_body(body, offset, graph.exit_index(), &mut graph);

        graph
    }

    fn connect_body(
        body: &[Stmt],
        base: usize,
        exit: usize,
        graph: &mut ControlFlowGraph,
    ) {
        for (i, stmt) in body.iter().enumerate() {
            let node = base + i;
            let next = body.get(i + 1).map(|_| base + i + 1).unwrap_or(exit);

            match stmt {
                Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    let then_base = graph.nodes.len();
                    for (j, _) in then_body.iter().enumerate() {
                        graph.nodes.push(CfgNode::Statement { body_index: j });
                    }
                    graph.edges.push((node, then_base, CfgEdge::TrueBranch));
                    Self::connect_body(then_body, then_base, next, graph);

                    if else_body.is_empty() {
                        graph.edges.push((node, next, CfgEdge::FalseBranch));
                    } else {
                        let else_base = graph.nodes.len();
                        for (j, _) in else_body.iter().enumerate() {
                            graph.nodes.push(CfgNode::Statement { body_index: j });
                        }
                        graph.edges.push((node, else_base, CfgEdge::FalseBranch));
                        Self::connect_body(else_body, else_base, next, graph);
                    }
                }
                Stmt::Repeat { body: repeat_body, .. } => {
                    let loop_base = graph.nodes.len();
                    for (j, _) in repeat_body.iter().enumerate() {
                        graph.nodes.push(CfgNode::Statement { body_index: j });
                    }
                    graph.edges.push((node, loop_base, CfgEdge::Sequence));
                    Self::connect_body(repeat_body, loop_base, node, graph);
                    graph.edges.push((node, next, CfgEdge::FalseBranch));
                }
                Stmt::RepeatUntil {
                    condition,
                    body: repeat_body,
                } => {
                    let loop_base = graph.nodes.len();
                    for (j, _) in repeat_body.iter().enumerate() {
                        graph.nodes.push(CfgNode::Statement { body_index: j });
                    }
                    graph.edges.push((node, loop_base, CfgEdge::FalseBranch));
                    Self::connect_body(repeat_body, loop_base, node, graph);

                    // Detect trivial infinite loop: repeat until <false>
                    let always_false = matches!(condition, Expr::Literal(SgValue::Bool(false)));
                    if !always_false {
                        graph.edges.push((node, next, CfgEdge::TrueBranch));
                    }
                }
                Stmt::Forever { body: forever_body } => {
                    let loop_base = graph.nodes.len();
                    for (j, _) in forever_body.iter().enumerate() {
                        graph.nodes.push(CfgNode::Statement { body_index: j });
                    }
                    graph.edges.push((node, loop_base, CfgEdge::Sequence));
                    Self::connect_body(forever_body, loop_base, node, graph);
                }
                Stmt::Stop { option: StopOption::All } => {
                    graph.edges.push((node, exit, CfgEdge::Sequence));
                }
                _ => {
                    graph.edges.push((node, next, CfgEdge::Sequence));
                }
            }
        }
    }
}
