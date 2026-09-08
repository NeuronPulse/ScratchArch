# Roundtrip & Semantic Validation Framework

> Design version: **v1.0**
> Primary crate: `crates/scratcharch-validation`
> Spec contract: `docs/specification/SCRATCH_SEMANTICS.md` (§4 equivalence, §5 roundtrip)

ScratchArch converts Scratch programs between representations all the time —
`.sb3` ↔ ScratchGraph, ScratchGraph ↔ project.json, ScratchGraph ↔ SAIR, and
through optimization passes. Every such conversion is a chance to lose or
change meaning. This framework makes those conversions **verifiable**: for each
one it answers "did this conversion preserve semantics?", and when it cannot
prove preservation it says so instead of staying silent.

## Goal

Prove, as far as the IR models behavior, that the conversions in the Scratch
pipeline preserve semantics:

```
SB3 → ScratchGraph → (transform) → ScratchGraph → SB3
ScratchGraph → SAIR → ScratchGraph
```

Equality is **IR-level, never byte-level**. Two projects are compared through
the `SemanticNormalizer` (dropping block IDs, declaration ordering, broadcast
declaration sites, target order) and the categorized `semantic_diff`
(`docs/design/SEMANTIC_DIFF.md`). This is the deliberate answer to the trap of
treating `project.json` byte equality as semantic equality: bytes are
representation, meaning is normalized IR.

## Framework layout

| Component | Where | Role |
|---|---|---|
| Semantic model | `docs/specification/SCRATCH_SEMANTICS.md` | Defines observable behavior, fidelity buckets, equivalence contract |
| `SemanticNormalizer` | `scratcharch-scratchgraph::semantic` | Reduces a `Project` to its semantic content (`NormalizedProject`) |
| `semantic_diff` | `scratcharch-analyzer::diff` | Classifies `N(A) → N(B)` differences into semantic categories |
| `validate_graph` | `scratcharch-validation::graph` | Reference-integrity check: every referenced var/list is declared, every `Call` resolves |
| `roundtrip_sb3` | `scratcharch-validation::roundtrip` | `Project → Sb3Writer → .sb3 bytes → Sb3Reader → Project` |
| `verify_project` | `scratcharch-validation::verify` | Composes all checks into one report (used by `scratcharch verify`) |
| Preservation verdicts | `scratcharch-validation::preservation` | Per-pass soundness checks (transform preservation) |

## The checks

### 1. Graph validation

A parsed `Project` must be well-formed ScratchGraph: every variable/list a
script references is declared on the referencing target or the stage, and every
procedure `Call` resolves to a definition on the referencing target or the
stage. Broadcast messages are deliberately *not* checked here (a broadcast may
have zero receivers; that is legal Scratch).

`validate_graph(project) -> Result<(), Vec<String>>`.

### 2. Roundtrip

One full byte roundtrip through the SB3 archive format:
`Project → Sb3Writer → .sb3 → Sb3Reader → Project`. A format-level failure
(zip corruption, malformed JSON, unreadable opcode) is an error. A roundtrip
that succeeds may still be semantically lossy — that is what check 3 decides.

`roundtrip_sb3(project) -> Result<Project, String>`
`roundtrip_diff(project) -> Result<DiffResult, String>`

### 3. Semantic preservation

The roundtrip is lossless iff `N(original) == N(roundtripped)`, equivalently
`semantic_diff(original, roundtripped).is_empty()`. Because an `.sb3` file
cannot carry IR that lives outside the native subset (see the lossy-code note
below), a project loaded from a real archive is already in canonical form and
this check passes by construction; the check earns its keep on in-memory
programs that use ABI frame/heap code, where export expands into list idioms
that the parser does not reconstruct.

### 4. Transform preservation

Each default-pipeline transform pass (`scratcharch-transform`) is run on a copy
and judged against its own soundness contract. A pass may change the IR — that
is its job — so a plain `N(before) == N(after)` is neither necessary nor
sufficient. The verdicts instead forbid the disallowed changes:

- **DCE** may remove only provably-dead scripts/procedures; it is forbidden from
  deleting a reachable broadcast receiver, a live procedure, or changing which
  receivers a broadcast reaches (`dce_preserves_liveness`).
- **Constant folding** may replace a fully-constant numeric arithmetic
  expression with its IEEE-754 value and nothing else; boolean contexts
  (comparisons) are never folded, and control/broadcast/list usage is untouched
  (`constant_folding_reaches_canonical_fold`).
- **Variable analysis** may drop only declarations whose name no remaining code
  reads or writes (`variable_analysis_keeps_referenced`).
- **Empty-block removal** may drop only empty control wrappers, never a block
  with a non-empty body (`empty_block_removal_matches`).

`verify_project` reports a `PassCheck` per pass: `name`, number of IR nodes the
pass changed, and whether the pass stayed within its contract.

## `scratcharch verify`

The CLI command wires the engine to a real file:

```text
scratcharch verify <project.sb3 | project.json>
```

Output lines:

```text
Parse: PASS
Graph validation: PASS
Roundtrip: PASS
Semantic preservation: PASS
Transform preservation: PASS
```

`--json` emits the same report as JSON (`parse_ok`, `graph_valid`,
`roundtrip_ok`, `semantic_preserved`, `passes[]`, `transform_preserved`). A
failed check prints its detail lines and exits non-zero.

### When is a project "verified"?

All five checks green. A pass removing provably-dead code or folding a constant
is *allowed* — the verdict is sound for that pass, so a project with
legitimately dead code still verifies. Verification fails when a pass commits a
disallowed change (deletes a live receiver, folds a boolean, drops a referenced
variable, removes a non-empty block) or when graph validation/roundtrip/
semantic preservation fails.

## Test layers

| Layer | Location | Asserts |
|---|---|---|
| Roundtrip matrix | `scratcharch-validation/tests/roundtrip/` | Each native program round-trips byte-exact semantically; ABI code is *detected* as lossy (31 tests) |
| Transform preservation | `scratcharch-validation/tests/preservation.rs` | Each pass commits no disallowed change (17 tests) |
| Verify engine | `scratcharch-validation/src/verify.rs` (unit) | Empty/live projects pass; undeclared var fails graph; ABI code fails semantic preservation (4 tests) |
| CLI | `scratcharch-cli/tests/cli_tests.rs` | `verify` PASS/FAIL/json on real files (5 tests) |
| Differential corpus | `scratcharch-validation/tests/corpus/` | Representative programs load from committed `project.json`, pass every check, and match their canonical builder (3 tests) |

A failure must never be hidden by loosening a comparison. The roundtrip matrix
contains *detection* tests: for documented limitations (e.g. ABI-lowered frame
code, which the native exporter expands into list idioms) the assertion is that
the framework *reports* the difference — not that it passes it.

## Differential corpus

`scratcharch-validation/tests/corpus/scratch/` commits representative native
programs as loadable `project.json` data plus a `manifest.json` catalog. The
harness loads each fixture through the real `.json` parser, graph-validates it,
runs the full `verify_project` gate, and normalizes it against the canonical
member builder. The corpus is the seed for future automatic regression: any
change to the parser, exporter, normalizer, or a pass that alters a committed
member's meaning fails until fixed or deliberately regenerated. See
`tests/corpus/README.md`.

## Known boundaries (deliberately reported, never hidden)

These follow `SCRATCH_SEMANTICS.md` §6. The framework verifies what the IR
models; it does not claim more:

- **Lossy code is an in-memory property.** No `.sb3` file can carry IR outside
  the native roundtrippable subset, so `Semantic preservation: FAIL` is not
  reachable from an archive on disk — it is reachable from in-memory ABI code,
  and that is exactly where the framework refuses to bless it.
- **Empty `Forever` / empty loops.** Removing an empty control wrapper is sound
  only under the model's assumption that an idle spin is unobservable. The
  preservation engine mirrors the pass so verdicts agree, but a project with
  statements *after* a removed infinite loop would run them where the original
  never did — outside the model.
- **SAIR leg.** `ScratchGraph → SAIR → ScratchGraph` is limited to the
  procedure subset and is value-lossy for non-integer numerics (SAIR is typed,
  Scratch is untyped `f64`).

## Relation to other documents

- `docs/specification/SCRATCH_SEMANTICS.md` — the semantic model this framework verifies
- `docs/design/SEMANTIC_DIFF.md` — the diff engine and its categories
- `docs/design/SCRATCH_ROUNDTRIP.md` — earlier roundtrip design (v0.3) for the parser
- `docs/design/OPTIMIZATION.md` — the transform passes and their policies (§6)
- `docs/specification/SCRATCHGRAPH.md` — the ScratchGraph IR
- `docs/design/SCRATCHARCH_CLI.md` — CLI subcommands including `verify`
