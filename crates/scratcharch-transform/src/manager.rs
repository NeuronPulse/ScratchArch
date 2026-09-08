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
    use crate::pass::PassReport;
    use scratcharch_scratchgraph::ir::Project;

    /// Minimal pass used to exercise the manager before concrete passes exist.
    struct CountingPass;

    impl TransformPass for CountingPass {
        fn name(&self) -> &str {
            "Counting Pass"
        }

        fn run(&mut self, _project: &mut Project) -> PassReport {
            PassReport {
                name: self.name().to_string(),
                modifications: 1,
                deleted_nodes: 0,
                saved_variables: 0,
            }
        }
    }

    #[test]
    fn test_pass_manager_runs_all_registered_passes() {
        let mut project = Project::default();
        let mut manager = PassManager::new();

        manager.add(CountingPass);
        manager.add(CountingPass);

        let report = manager.run(&mut project);

        assert_eq!(report.passes.len(), 2, "both passes should run");
        assert_eq!(manager.passes().len(), 2, "registered passes exposed");
        assert_eq!(report.total_modifications(), 2, "reports are aggregated");
    }
}
