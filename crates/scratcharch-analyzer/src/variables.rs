//! Variable and list usage analysis.
//!
//! Computes which variables are read, written, or unused in a ScratchGraph
//! project. This is a foundational analysis for dead-store elimination and
//! live-variable tracking.

use std::collections::HashMap;

use scratcharch_scratchgraph::ir::{Expr, Project, Sprite, Stage, Stmt};

/// Usage summary for a single variable or list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VariableUsage {
    pub reads: usize,
    pub writes: usize,
}

/// Variable usage analysis result.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VariableUsageAnalysis {
    pub variables: HashMap<String, VariableUsage>,
    pub lists: HashMap<String, VariableUsage>,
}

impl VariableUsageAnalysis {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return variables that are written but never read.
    pub fn dead_variables(&self) -> Vec<String> {
        self.variables
            .iter()
            .filter(|(_, u)| u.writes > 0 && u.reads == 0)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Return variables that are read but never written.
    pub fn unread_initializations(&self) -> Vec<String> {
        self.variables
            .iter()
            .filter(|(_, u)| u.reads > 0 && u.writes == 0)
            .map(|(name, _)| name.clone())
            .collect()
    }
}

/// Analyze variable and list usage across a project.
#[derive(Debug, Clone, Default)]
pub struct VariableUsageAnalyzer;

impl VariableUsageAnalyzer {
    pub fn new() -> Self {
        Self
    }

    pub fn analyze(&self, project: &Project) -> VariableUsageAnalysis {
        let mut result = VariableUsageAnalysis::new();
        self.analyze_stage(&project.stage, &mut result);
        for sprite in &project.sprites {
            self.analyze_sprite(sprite, &mut result);
        }
        result
    }

    fn analyze_stage(&self, stage: &Stage, result: &mut VariableUsageAnalysis) {
        for script in &stage.scripts {
            self.analyze_body(&script.entry.body, result);
        }
        for proc in &stage.procedures {
            self.analyze_body(&proc.body, result);
        }
    }

    fn analyze_sprite(&self, sprite: &Sprite, result: &mut VariableUsageAnalysis) {
        for script in &sprite.scripts {
            self.analyze_body(&script.entry.body, result);
        }
        for proc in &sprite.procedures {
            self.analyze_body(&proc.body, result);
        }
    }

    fn analyze_body(&self, body: &[Stmt], result: &mut VariableUsageAnalysis) {
        for stmt in body {
            self.analyze_stmt(stmt, result);
        }
    }

    fn analyze_stmt(&self, stmt: &Stmt, result: &mut VariableUsageAnalysis) {
        match stmt {
            Stmt::SetVariable { var, value } => {
                self.analyze_expr(value, result);
                result.variables.entry(var.clone()).or_default().writes += 1;
            }
            Stmt::ChangeVariable { var, delta } => {
                self.analyze_expr(delta, result);
                result.variables.entry(var.clone()).or_default().reads += 1;
                result.variables.entry(var.clone()).or_default().writes += 1;
            }
            Stmt::AddToList { list, value } | Stmt::InsertListItem { list, value, .. } => {
                self.analyze_expr(value, result);
                result.lists.entry(list.clone()).or_default().writes += 1;
            }
            Stmt::DeleteAllOfList { list } | Stmt::DeleteListItem { list, .. } => {
                result.lists.entry(list.clone()).or_default().writes += 1;
            }
            Stmt::SetListItem { list, index, value } => {
                self.analyze_expr(index, result);
                self.analyze_expr(value, result);
                result.lists.entry(list.clone()).or_default().writes += 1;
            }
            Stmt::Broadcast { message } => {
                self.analyze_expr(message, result);
            }
            Stmt::Call { args, .. } => {
                for arg in args {
                    self.analyze_expr(arg, result);
                }
            }
            Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                self.analyze_expr(condition, result);
                self.analyze_body(then_body, result);
                self.analyze_body(else_body, result);
            }
            Stmt::Repeat { times, body: b }
            | Stmt::RepeatUntil { condition: times, body: b } => {
                self.analyze_expr(times, result);
                self.analyze_body(b, result);
            }
            Stmt::Forever { body: b } => {
                self.analyze_body(b, result);
            }
            Stmt::Expr(e) => self.analyze_expr(e, result),
            _ => {}
        }
    }

    fn analyze_expr(&self, expr: &Expr, result: &mut VariableUsageAnalysis) {
        match expr {
            Expr::Variable(name) => {
                result.variables.entry(name.clone()).or_default().reads += 1;
            }
            Expr::List(name) => {
                result.lists.entry(name.clone()).or_default().reads += 1;
            }
            Expr::ListItem { list, index } => {
                result.lists.entry(list.clone()).or_default().reads += 1;
                self.analyze_expr(index, result);
            }
            Expr::ListLength { list } => {
                result.lists.entry(list.clone()).or_default().reads += 1;
            }
            Expr::Operator { args, .. } => {
                for arg in args {
                    self.analyze_expr(arg, result);
                }
            }
            Expr::HeapLoad { addr } | Expr::HeapIndex { base: addr, .. } => {
                self.analyze_expr(addr, result);
            }
            _ => {}
        }
    }
}
