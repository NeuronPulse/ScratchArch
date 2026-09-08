//! SB3 byte roundtrip: `ScratchGraph → Sb3Writer → .sb3 bytes → Sb3Reader
//! → ScratchGraph`.
//!
//! The full path goes through real archive bytes (`to_bytes` / `from_bytes`),
//! so it also exercises the zip serialization and the JSON round trip that the
//! `Sb3Reader` performs internally. Callers compare the returned project with
//! the original through [`scratcharch_analyzer::semantic_diff`].

use scratcharch_analyzer::{semantic_diff, DiffResult};
use scratcharch_sb3::{Sb3Archive, Sb3Reader, Sb3Writer};
use scratcharch_scratchgraph::Project;

/// Round-trip a project through the SB3 archive format and back to a
/// `Project`.
///
/// Errors are only *format-level* failures (zip serialization, malformed
/// JSON, an opcode the parser cannot read back). A successfully parsed result
/// may still differ semantically; compare it with [`roundtrip_diff`].
pub fn roundtrip_sb3(project: &Project) -> Result<Project, String> {
    let archive = Sb3Writer::new().write(project);
    let bytes = archive
        .to_bytes()
        .map_err(|e| format!("serializing project to sb3 bytes failed: {e}"))?;
    let archive2 = Sb3Archive::from_bytes(&bytes)
        .map_err(|e| format!("deserializing sb3 bytes failed: {e}"))?;
    Sb3Reader::new()
        .read(&archive2)
        .map_err(|e| format!("reading sb3 archive back into a project failed: {e}"))
}

/// Compute the semantic difference introduced by one SB3 byte roundtrip.
///
/// An empty result means the roundtrip preserved semantics exactly.
pub fn roundtrip_diff(project: &Project) -> Result<DiffResult, String> {
    let back = roundtrip_sb3(project)?;
    Ok(semantic_diff(project, &back))
}
