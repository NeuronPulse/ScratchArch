use serde::Serialize;
use scratcharch_scratchgraph::ir::Project;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct OptimizationReport {
    pub passes: Vec<PassReport>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PassReport {
    pub name: String,
    pub modifications: u32,
    pub deleted_nodes: u32,
    pub saved_variables: u32,
}

impl OptimizationReport {
    pub fn new() -> Self {
        Self { passes: Vec::new() }
    }

    pub fn add(&mut self, report: PassReport) {
        self.passes.push(report);
    }

    pub fn total_modifications(&self) -> u32 {
        self.passes.iter().map(|p| p.modifications).sum()
    }

    pub fn total_deletions(&self) -> u32 {
        self.passes.iter().map(|p| p.deleted_nodes).sum()
    }

    pub fn total_saved_variables(&self) -> u32 {
        self.passes.iter().map(|p| p.saved_variables).sum()
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str("Optimization Report\n");
        out.push_str("====================\n");
        for p in &self.passes {
            out.push_str(&format!("  {}: {} modifications, {} deletions, {} saved vars\n",
                p.name, p.modifications, p.deleted_nodes, p.saved_variables));
        }
        out.push_str(&format!("\nTotal: {} modifications, {} deletions, {} saved variables\n",
            self.total_modifications(), self.total_deletions(), self.total_saved_variables()));
        out
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

pub trait TransformPass {
    fn name(&self) -> &str;
    fn run(&mut self, project: &mut Project) -> PassReport;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_to_text() {
        let report = OptimizationReport::new();
        let text = report.to_text();
        assert!(text.contains("Optimization Report"));
    }

    #[test]
    fn test_report_to_json() {
        let mut report = OptimizationReport::new();
        report.add(PassReport {
            name: "Test".into(),
            modifications: 5,
            deleted_nodes: 3,
            saved_variables: 0,
        });
        let json = report.to_json();
        assert_eq!(json["passes"].as_array().unwrap().len(), 1);
    }
}
