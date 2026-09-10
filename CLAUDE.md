# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build                          # build all crates
cargo test --workspace               # run all workspace tests (624 total)
cargo test -p scratcharch-transform  # test a single crate
cargo test -p scratcharch-llvm -- test_name --nocapture  # run one test
cargo clippy --workspace --all-targets  # lint (zero warnings required)
./scripts/run_c_tests.sh             # C compatibility pipeline tests
```

## Architecture

ScratchArch started as a custom-architecture compiler/VM stack and has grown
into a Scratch 3.0 toolchain. The repo contains two related IR pipelines that
share the `scratcharch-core`/`scratcharch-ir` foundations:

```
Scratch source (.sb3 / project.json)
        │  Sb3Reader / parse_project_json
        ▼
  ScratchGraph Project ── analyzer ──▶ reports / DOT / diff
        │  scratcharch-transform passes
        ▼
  ScratchGraph Project ── JsonExporter / Sb3Writer ──▶ .sb3 / .json

C source → LLVM IR (.ll) → SAIR (IrModule) ──▶ interpreter
                                       └────▶ ISA lowering → VM (exact subset)
IrModule ── scratchgraph::lower ──▶ ScratchGraph Project
```

The **Scratch pipeline** (`.sb3`/`.json` ↔ ScratchGraph) is the primary,
complete toolchain: parse into `scratcharch_scratchgraph::ir::Project`,
analyze or optimize it with `scratcharch-transform` passes, and export back to
`.sb3` (via `scratcharch-sb3`) or project JSON (via `JsonExporter`).

The **SAIR pipeline** (C/LLVM → SAIR → interpreter/VM) is the original custom
stack. It also feeds the Scratch side: SAIR modules can be lowered to
ScratchGraph with `scratchgraph::lower`.

### Crate map

| Crate | Role |
|---|---|
| `scratcharch-core` | ISA-level types: `Instruction`, `Value`, `Program`, core type system |
| `scratcharch-target` | `TargetProfile` struct; SA48 profile (48-bit cell, 32-bit pointer, LE) |
| `scratcharch-ir` | SAIR: SSA IR with typed values, basic blocks, phi nodes, GEP, builder API, validator, ISA lowerer (`lower.rs`) |
| `scratcharch-vm` | Stack-based VM that executes core ISA programs |
| `scratcharch-runtime` | Portable runtime library (SART), shared by the SAIR interpreter |
| `scratcharch-sair-interpreter` | Directly interprets SAIR modules (no lowering); supports full multi-block/phi/GEP/recursion |
| `scratcharch-llvm` | Hand-written LLVM IR lexer + recursive-descent parser (`parser.rs`) and LLVM→SAIR translator (`translator.rs`); no LLVM dependency |
| `scratcharch-opt` | SAIR optimization passes: constant folding, DCE, CFG simplification; pass manager in `manager.rs` |
| `scratcharch-driver` | Compilation driver for the classic SAIR/ISA stack |
| `scratcharch-scratchgraph` | ScratchGraph IR (`ir`), project JSON parser, `JsonExporter`, SAIR→ScratchGraph lowerer (`lower.rs`), event/runtime model (`runtime.rs`) |
| `scratcharch-sb3` | `.sb3` archive reader/writer (`Sb3Reader`/`Sb3Writer`), asset manager; writer reuses the ScratchGraph `JsonExporter` |
| `scratcharch-transform` | ScratchGraph optimization passes (`TransformPass`/`PassManager`) and reports |
| `scratcharch-validation` | Roundtrip & semantic validation: graph validation, SB3 byte roundtrip, per-pass preservation verdicts, `verify_project` (used by `scratcharch verify`) |
| `scratcharch-analyzer` | Static analysis of ScratchGraph projects: reports, DOT, semantic diff |
| `scratcharch-explorer` | Unified data exploration interface over SAIR and ScratchGraph |
| `scratcharch-pipeline` | Unified compilation pipeline orchestrator (dumps, modes) used by the CLI |
| `scratcharch-cli` | `scratcharch` binary: build / analyze / decompile / graph / inspect / optimize / diff / verify / debug / pipeline subcommands |

`scratcharch-opt` and `scratcharch-transform` are deliberately separate
frameworks: `scratcharch-opt` optimizes SAIR `IrModule`s (SSA/basic blocks),
while `scratcharch-transform` optimizes ScratchGraph `Project`s (sprites,
scripts, procedures, variables). See `docs/design/OPTIMIZATION.md` section 6.

### Execution paths

The **Scratch path** (`scratcharch-scratchgraph` + `scratcharch-sb3` →
`scratcharch-transform`/`scratcharch-analyzer`) is complete and covered by the
sb3/scratchgraph/transform/cli test suites. `scratcharch-validation` proves the
pipeline preserves semantics — graph validation, `.sb3` roundtrip, semantic
preservation, and per-pass transform preservation via `scratcharch verify` —
over roundtrip, preservation, and differential-corpus suites (see
`docs/design/ROUNDTRIP_VALIDATION.md`).

The **SAIR interpreter path** (`scratcharch-llvm` → `scratcharch-sair-interpreter`)
is complete and used for all pipeline tests. It handles multi-block control
flow, phi nodes, GEP, and recursive calls directly on SAIR.

The **VM path** (`scratcharch-ir::lower` → `scratcharch-vm`) lowers validated
SAIR to the frozen 32-bit-word ISA. It handles multi-block functions, `phi`
(edge copies inserted by `lower.rs` via critical-edge splitting), GEP (byte
offsets), and `select`/`switch`/`unreachable` (`Trap`). `i64` values are two
32-bit limbs: add/sub/compare/cast/load/store/select/phi run per limb, bitwise
runs per-limb word ops, and every shift and full-width
`mul`/`udiv`/`urem`/`sdiv`/`srem` (the signed forms are already expanded by the
translator to magnitudes) is realised by a demand-appended program-level
software helper — `__sair_shl64`/`__sair_lshr64`/`__sair_ashr64`,
`__sair_mul64`, `__sair_udivrem64` — built from the word ops, so no widening
ISA was needed. Sub-word memory is byte-exact: `i1`/`i8`/`i16` loads/stores
lower to width-exact `Load8`/`Store8` sequences and the static-data segment is
seeded byte-exact on both backends; reinterpret casts
(`bitcast`/`ptrtoint`/`inttoptr`) lower to zero-cost cell-preserving copies
(`ptrtoint i64` zero-extends, `inttoptr i64` traps on a nonzero high limb). The
remaining constructs are `Interpreter only`: `llvm.*`/runtime intrinsics (no
`define`d body to call). The VM never approximates —
what it cannot execute faithfully it rejects with a named diagnostic.

The authoritative per-construct status lives in
`docs/specification/LLVM_COMPATIBILITY.md` (interpreter/VM matrix, known gaps,
verification); `docs/specification/EXECUTION_MODEL.md` §5.7–§5.8 define the
software-helper semantics and `docs/design/LLVM_TRANSLATION.md` the mapping.

### SAIR key invariants

- All arithmetic is wrapping (no poison/undef).
- Every basic block ends with exactly one terminator (`Branch`, `CondBranch`, `Return`).
- First block in a function is the entry block.
- Phi nodes use edge-selected semantics; the interpreter tracks `prev_block` to resolve them.
- `IrModule::validate()` enforces reachability, phi predecessor consistency, and block label existence.

### ScratchGraph key facts

- Broadcast messages are project-global: a `broadcast` in any sprite/stage
  (including inside a procedure) can trigger `BroadcastReceived` hats on any
  target. Procedure (custom block) definitions are per-target.
- Variables are declared per target with an `id` and a `name`; scripts and
  procedures reference variables by name, so renames/removals must respect all
  targets.
- Scratch boolean values are distinct from the numeric literals `0`/`1`; passes
  must not collapse between them.
- Arithmetic follows Scratch/JS IEEE-754 `f64` semantics (wrapping/inf/NaN as
  Scratch defines them).

### Test layout

- `crates/scratcharch-llvm/tests/pipeline_tests.rs` — full LLVM→SAIR→interpreter
  path over committed real-clang `-O0` `.ll` output in `tests/c_programs/`.
- `crates/scratcharch-llvm/tests/corpus_clang_tests.rs` — recompiles each
  `tests/c_programs/*.c` with clang on every run so the committed `.ll` corpus
  cannot drift (skips silently when clang is absent).
- `crates/scratcharch-driver/tests/vm_backend_tests.rs` — VM/interpreter
  agreement over real-clang fixtures, including the multi-cell `i64` corpus
  (two-limb arithmetic and the software-helper mul/div/rem) and
  profile-driven limb counts.
- `crates/scratcharch-sair-interpreter/tests/vm_differential_tests.rs` —
  bit-for-bit interpreter-vs-VM agreement for the bitwise/shift family, full-width
  `i64` mul/udiv/urem, and the `unreachable` trap on both engines.
- `crates/scratcharch-sb3/tests/sb3_tests.rs` — SB3 write/read roundtrips and
  asset handling.
- `crates/scratcharch-scratchgraph/tests/parser_tests.rs` — JSON→Project parsing
  and JSON export.
- `crates/scratcharch-cli/tests/cli_tests.rs` — end-to-end CLI subcommands.
- `crates/scratcharch-transform/src/*.rs` — unit tests per pass (DCE, constant
  folding, empty-block removal, variable analysis).
- `crates/scratcharch-validation/tests/roundtrip/` — semantic SB3 roundtrip
  matrix (basic/procedures/recursion/events/lists/memory).
- `crates/scratcharch-validation/tests/preservation.rs` — per-pass transform
  preservation verdicts (soundness contracts).
- `crates/scratcharch-validation/tests/corpus/` — differential corpus harness
  over committed `project.json` fixtures + `manifest.json`.

## Commit Guidelines (hard requirement)

Every commit must satisfy the following rules. These are strict and must not be
relaxed.

### General principles

- One logical purpose per commit.
- Code, tests, and docs stay in sync.
- The commit must be understandable on its own.
- Never commit an unverified state.
- Forbidden messages: `fix stuff`, `update code`, `changes`, `wip`, `test`, etc.

### Message format (Conventional Commits)

```text
<type>(<scope>): <summary>
```

Examples:

```text
feat(ir): add phi validation
fix(vm): prevent double return write
docs(spec): document runtime model
test(llvm): add memcpy pipeline tests
```

### Types

| Type | Use for |
| ---- | ------- |
| `feat` | New crate, module, ISA instruction, or capability |
| `fix` | Bug fix (explain location and cause) |
| `docs` | Documentation-only changes |
| `test` | New or changed tests |
| `refactor` | Behavior-preserving restructuring |
| `perf` | Performance optimization |
| `chore` | CI, build scripts, dependencies |

### Scopes

Preferred scopes: `core`, `isa`, `abi`, `memory`, `ir`, `sair`, `vm`, `llvm`,
`runtime`, `opt`, `target`, `backend`, `scratch`, `scratchgraph`, `sb3`,
`transform`, `analyzer`, `cli`, `pipeline`, `docs`, `ci`.

### Large features

If a change touches more than ~5 files, ~300 lines, or one milestone, split it.
Good:

```text
feat(runtime): create runtime crate
feat(runtime): add memory intrinsics
feat(runtime): add string intrinsics
test(runtime): add runtime pipeline tests
docs(runtime): document runtime design
```

Bad:

```text
feat(runtime): add runtime
```

### Verification before commit

Run before committing:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
```

For LLVM-related changes also run:

```bash
./scripts/run_c_tests.sh
```

When committing, include verification in the message body if helpful:

```text
feat(runtime): add string intrinsics

Tests:
- cargo test ✓
- cargo clippy ✓
- run_c_tests.sh ✓
```

### Agent rule

After completing a task the agent must output a commit proposal in this form
and wait for user confirmation before committing:

```text
## Commit Proposal

Type:
Scope:

Title:

Summary:
-
-

Tests:
-

Files changed:
-
```

### Prohibited

- Changing a specification without updating its `docs/specification/*.md` file.
- Changing semantics without a migration note in docs.
- Mixing unrelated changes in one commit.
