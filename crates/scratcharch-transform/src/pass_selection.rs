use std::str::FromStr;

use crate::manager::PassManager;
use crate::{
    ConstantFolding, DeadScriptElimination, EmptyBlockRemoval, VariableAnalysis,
};

/// Names of the built-in ScratchGraph optimization passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassName {
    /// Remove unreachable scripts and uncalled procedures.
    DeadScriptElimination,
    /// Fold constant expressions.
    ConstantFolding,
    /// Remove empty control blocks.
    EmptyBlockRemoval,
    /// Remove variables that are never referenced by any script or procedure.
    VariableAnalysis,
}

impl PassName {
    /// All passes in the default execution order.
    pub fn default_pipeline() -> &'static [PassName] {
        &[
            PassName::DeadScriptElimination,
            PassName::ConstantFolding,
            PassName::EmptyBlockRemoval,
            PassName::VariableAnalysis,
        ]
    }
}

impl FromStr for PassName {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "dce" | "dead-script-elimination" | "deadscriptelimination" => {
                Ok(PassName::DeadScriptElimination)
            }
            "constfold" | "constant-folding" | "constantfolding" => Ok(PassName::ConstantFolding),
            "emptyblocks" | "empty-block-removal" | "emptyblockremoval" => {
                Ok(PassName::EmptyBlockRemoval)
            }
            "variables" | "variable-analysis" | "variableanalysis" => {
                Ok(PassName::VariableAnalysis)
            }
            _ => Err(format!("unknown pass name: {}", s)),
        }
    }
}

/// Parse a comma-separated pass list.
///
/// The special value `"all"` expands to the default pipeline.
pub fn parse_pass_list(s: &str) -> Result<Vec<PassName>, String> {
    let trimmed = s.trim();
    if trimmed.eq_ignore_ascii_case("all") {
        return Ok(PassName::default_pipeline().to_vec());
    }

    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    trimmed
        .split(',')
        .map(|part| part.trim().parse())
        .collect()
}

impl PassManager {
    /// Build a pass manager from a list of pass names.
    pub fn from_pass_names(names: &[PassName]) -> Self {
        let mut manager = PassManager::new();
        for name in names {
            match name {
                PassName::DeadScriptElimination => manager.add(DeadScriptElimination::new()),
                PassName::ConstantFolding => manager.add(ConstantFolding::new()),
                PassName::EmptyBlockRemoval => manager.add(EmptyBlockRemoval::new()),
                PassName::VariableAnalysis => manager.add(VariableAnalysis::new()),
            }
        }
        manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_all() {
        let passes = parse_pass_list("all").unwrap();
        assert_eq!(passes, PassName::default_pipeline().to_vec());
    }

    #[test]
    fn test_parse_short_names() {
        let passes = parse_pass_list("dce,constfold,emptyblocks,variables").unwrap();
        assert_eq!(
            passes,
            vec![
                PassName::DeadScriptElimination,
                PassName::ConstantFolding,
                PassName::EmptyBlockRemoval,
                PassName::VariableAnalysis,
            ]
        );
    }

    #[test]
    fn test_parse_unknown_pass() {
        let result = parse_pass_list("dce,unknown");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown"));
    }

    #[test]
    fn test_pass_manager_from_names() {
        let manager = PassManager::from_pass_names(&[PassName::ConstantFolding]);
        assert_eq!(manager.passes().len(), 1);
        assert_eq!(manager.passes()[0].name(), "Constant Folding");
    }
}
