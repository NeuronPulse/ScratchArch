use crate::pass::{OptimizationReport, PassReport, TransformPass};
use scratcharch_scratchgraph::ir::Project;

pub struct PassManager {
    passes: Vec<Box<dyn TransformPass>>,
}

impl PassManager {
    pub fn new() -> Self {
        Self { passes: Vec::new() }
    }

    pub fn add<P: TransformPass + 'static>(&mut self, pass: P) {
        self.passes.push(Box::new(pass));
    }

    pub fn run(&mut self, project: &mut Project) -> OptimizationReport {
        let mut report = OptimizationReport::new();
        for pass in &mut self.passes {
            let pass_report: PassReport = pass.run(project);
            report.add(pass_report);
        }
        report
    }

    pub fn passes(&self) -> &[Box<dyn TransformPass>] {
        &self.passes
    }
}

impl Default for PassManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConstantFolding, DeadScriptElimination, EmptyBlockRemoval, VariableAnalysis};
    use scratcharch_scratchgraph::ir::*;

    fn make_project() -> Project {
        Project {
            stage: Stage {
                name: "Stage".into(),
                variables: vec![
                    Variable::new("x_id", "x"),
                    Variable::new("unused_id", "unused"),
                ],
                lists: vec![],
                broadcasts: vec![],
                scripts: vec![Script::new(
                    EventHat::GreenFlag,
                    vec![Stmt::SetVariable {
                        var: "x".into(),
                        value: Expr::Operator {
                            opcode: "operator_add".into(),
                            args: vec![
                                Expr::Literal(Value::Number(1.0)),
                                Expr::Literal(Value::Number(2.0)),
                            ],
                        },
                    }],
                )],
                procedures: vec![],
                costumes: vec![],
                sounds: vec![],
            },
            sprites: vec![],
        }
    }

    #[test]
    fn test_pass_manager_runs_default_pipeline() {
        let mut project = make_project();
        let mut manager = PassManager::new();

        manager.add(ConstantFolding::new());
        manager.add(DeadScriptElimination::new());
        manager.add(VariableAnalysis::new());
        manager.add(EmptyBlockRemoval::new());

        let report = manager.run(&mut project);

        assert_eq!(report.passes.len(), 4, "all 4 passes should run");
        assert!(
            report.total_modifications() > 0,
            "should have some modifications"
        );
        assert!(
            report.total_saved_variables() > 0,
            "should save some variables"
        );
        assert!(
            report.to_text().contains("Optimization Report"),
            "text report should have header"
        );
    }
}
