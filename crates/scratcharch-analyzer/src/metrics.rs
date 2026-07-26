use scratcharch_scratchgraph::ir::{Project, Stmt};

pub struct ComplexityMetrics {
    pub script_count: usize,
    pub procedure_count: usize,
    pub total_statements: usize,
    pub max_nesting_depth: usize,
    pub variable_count: usize,
    pub list_count: usize,
    pub call_count: usize,
    pub broadcast_count: usize,
}

impl ComplexityMetrics {
    pub fn new(project: &Project) -> Self {
        let mut metrics = ComplexityMetrics {
            script_count: 0,
            procedure_count: 0,
            total_statements: 0,
            max_nesting_depth: 0,
            variable_count: 0,
            list_count: 0,
            call_count: 0,
            broadcast_count: 0,
        };

        metrics.script_count += project.stage.scripts.len();
        metrics.procedure_count += project.stage.procedures.len();
        metrics.variable_count += project.stage.variables.len();
        metrics.list_count += project.stage.lists.len();

        for script in &project.stage.scripts {
            metrics.total_statements += script.entry.body.len();
            metrics.max_nesting_depth = metrics.max_nesting_depth.max(compute_depth(&script.entry.body));
            count_ops(&script.entry.body, &mut metrics);
        }
        for proc in &project.stage.procedures {
            metrics.total_statements += proc.body.len();
            metrics.max_nesting_depth = metrics.max_nesting_depth.max(compute_depth(&proc.body));
            count_ops(&proc.body, &mut metrics);
        }

        for sprite in &project.sprites {
            metrics.script_count += sprite.scripts.len();
            metrics.procedure_count += sprite.procedures.len();
            metrics.variable_count += sprite.variables.len();
            metrics.list_count += sprite.lists.len();

            for script in &sprite.scripts {
                metrics.total_statements += script.entry.body.len();
                metrics.max_nesting_depth = metrics.max_nesting_depth.max(compute_depth(&script.entry.body));
                count_ops(&script.entry.body, &mut metrics);
            }
            for proc in &sprite.procedures {
                metrics.total_statements += proc.body.len();
                metrics.max_nesting_depth = metrics.max_nesting_depth.max(compute_depth(&proc.body));
                count_ops(&proc.body, &mut metrics);
            }
        }

        metrics
    }
}

fn compute_depth(body: &[Stmt]) -> usize {
    let mut max_depth = 0usize;
    for stmt in body {
        let depth = 1 + match stmt {
            Stmt::If { then_body, else_body, .. } => {
                compute_depth(then_body).max(compute_depth(else_body))
            }
            Stmt::Repeat { body: b, .. }
            | Stmt::RepeatUntil { body: b, .. }
            | Stmt::Forever { body: b } => compute_depth(b),
            _ => 0,
        };
        max_depth = max_depth.max(depth);
    }
    max_depth
}

fn count_ops(body: &[Stmt], metrics: &mut ComplexityMetrics) {
    for stmt in body {
        match stmt {
            Stmt::Call { .. } => metrics.call_count += 1,
            Stmt::Broadcast { .. } => metrics.broadcast_count += 1,
            Stmt::If { then_body, else_body, .. } => {
                count_ops(then_body, metrics);
                count_ops(else_body, metrics);
            }
            Stmt::Repeat { body: b, .. }
            | Stmt::RepeatUntil { body: b, .. }
            | Stmt::Forever { body: b } => count_ops(b, metrics),
            _ => {}
        }
    }
}
