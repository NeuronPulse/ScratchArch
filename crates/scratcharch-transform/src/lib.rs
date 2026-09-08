pub mod pass;
pub mod manager;
pub mod pass_selection;
pub mod dce;
pub mod const_fold;
pub mod variable;
pub mod empty_block;

pub use pass::{OptimizationReport, PassReport, TransformPass};
pub use manager::PassManager;
pub use pass_selection::{parse_pass_list, PassName};
pub use dce::DeadScriptElimination;
pub use const_fold::ConstantFolding;
pub use variable::VariableAnalysis;
pub use empty_block::EmptyBlockRemoval;
