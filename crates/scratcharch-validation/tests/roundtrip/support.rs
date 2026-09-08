//! Shared helpers for the roundtrip test matrix.
//!
//! `assert_roundtrip_preserves` is the *strict* assertion: one SB3 byte
//! roundtrip must produce a project whose semantic diff against the original
//! is empty. `assert_roundtrip_detects_loss` is used only for documented
//! limitations (see `memory`): it proves the checker *refuses* to silently
//! bless a lossy conversion instead of loosening the comparison.

use scratcharch_analyzer::{semantic_diff, DiffFormat};
use scratcharch_scratchgraph::Project;
use scratcharch_validation::roundtrip_sb3;

/// Run a full SB3 byte roundtrip; panic on format-level failures so a
/// serialization/parse bug fails loudly and visibly.
pub fn roundtrip(project: &Project) -> Project {
    roundtrip_sb3(project)
        .unwrap_or_else(|e| panic!("SB3 roundtrip failed at the format level: {e}"))
}

/// Assert that one full SB3 byte roundtrip preserves semantics exactly.
pub fn assert_roundtrip_preserves(project: &Project, case: &str) {
    let back = roundtrip(project);
    let diff = semantic_diff(project, &back);
    assert!(
        diff.is_empty(),
        "[{case}] SB3 roundtrip changed semantics:\n{}",
        diff.format(DiffFormat::Text)
    );
}

/// Assert that the framework *detects* a semantic change introduced by the
/// roundtrip (used for documented limitations, e.g. ABI-lowered code that the
/// native Scratch exporter expands into list idioms). This is not a loosened
/// test — it asserts the checker does not hide the loss.
pub fn assert_roundtrip_detects_loss(project: &Project, case: &str) {
    let back = roundtrip(project);
    let diff = semantic_diff(project, &back);
    assert!(
        !diff.is_empty(),
        "[{case}] roundtrip was expected to be lossy but no semantic difference was reported"
    );
}
