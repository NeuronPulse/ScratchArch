//! Roundtrip test matrix: SB3 → ScratchGraph → (transform) → ScratchGraph → SB3
//! (the SB3 leg exercised here is `Sb3Writer → .sb3 bytes → Sb3Reader`).
//!
//! Each case builds a native-Scratch `Project` within the roundtrippable
//! subset documented in `docs/specification/SCRATCH_SEMANTICS.md` and asserts
//! that one full byte roundtrip preserves semantics exactly (the semantic diff
//! between the original and the round-tripped project is empty).
//!
//! The matrix is deliberately grouped by program shape:
//! - `basic`: hats and plain data flow
//! - `procedures`: custom-block definitions, parameters, calls
//! - `recursion`: self- and mutual-recursive procedure structure
//! - `events`: broadcasts across targets
//! - `lists`: list mutation and reads
//! - `memory`: native list-as-memory programs (preserved) and ABI-lowered
//!   code (correctly *detected* as non-equivalent, not silently blessed)
//!
//! The case modules live in `tests/roundtrip/` (the `#[path]` attributes keep
//! them out of Cargo's auto-discovery of standalone integration tests).

#[path = "roundtrip/basic.rs"]
mod basic;
#[path = "roundtrip/events.rs"]
mod events;
#[path = "roundtrip/lists.rs"]
mod lists;
#[path = "roundtrip/memory.rs"]
mod memory;
#[path = "roundtrip/procedures.rs"]
mod procedures;
#[path = "roundtrip/recursion.rs"]
mod recursion;
#[path = "roundtrip/support.rs"]
mod support;
