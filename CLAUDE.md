# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build                          # build all crates
cargo test                           # run all tests (158 total)
cargo test -p scratcharch-llvm       # test a single crate
cargo test -p scratcharch-llvm -- test_name --nocapture  # run one test
cargo clippy                         # lint (zero warnings required)
./scripts/run_c_tests.sh             # C compatibility pipeline tests
```

## Architecture

ScratchArch is a custom architecture compiler/VM stack. The primary pipeline is:

```
C source → LLVM IR (.ll) → SAIR → interpreter
                                 → ISA lowering → VM  (partial; single-block only)
```

### Crate map

| Crate | Role |
|---|---|
| `scratcharch-core` | ISA-level types: `Instruction`, `Value`, `Program`, core type system |
| `scratcharch-ir` | SAIR: SSA IR with typed values, basic blocks, phi nodes, GEP, builder API, validator, ISA lowerer (`lower.rs`) |
| `scratcharch-vm` | Stack-based VM that executes core ISA programs |
| `scratcharch-sair-interpreter` | Directly interprets SAIR modules (no lowering); supports full multi-block/phi/GEP/recursion |
| `scratcharch-llvm` | Hand-written LLVM IR lexer + recursive-descent parser (`parser.rs`) and LLVM→SAIR translator (`translator.rs`); no LLVM dependency |
| `scratcharch-opt` | SAIR optimization passes: constant folding, DCE, CFG simplification; pass manager in `manager.rs` |
| `scratcharch-target` | `TargetProfile` struct; SA48 profile (48-bit cell, 32-bit pointer, LE) |

Dependency order (leaves first): `core` → `ir`, `vm` → `sair-interpreter` → `llvm` → `opt`

### Two execution paths

The **interpreter path** (`scratcharch-llvm` → `scratcharch-sair-interpreter`) is complete and used for all pipeline tests. It handles multi-block control flow, phi nodes, GEP, and recursive calls directly on SAIR.

The **VM path** (`scratcharch-ir::lower` → `scratcharch-vm`) is partial: `lower.rs` only handles single-block functions with no phi or GEP; multi-block lowering is in progress (see ROADMAP.md).

### SAIR key invariants

- All arithmetic is wrapping (no poison/undef).
- Every basic block ends with exactly one terminator (`Branch`, `CondBranch`, `Return`).
- First block in a function is the entry block.
- Phi nodes use edge-selected semantics; the interpreter tracks `prev_block` to resolve them.
- `IrModule::validate()` enforces reachability, phi predecessor consistency, and block label existence.

### Test layout

Integration/pipeline tests live in `crates/scratcharch-llvm/tests/pipeline_tests.rs` and exercise the full LLVM→SAIR→interpreter path using hand-written `.ll` files under `tests/c_programs/`.

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
`runtime`, `opt`, `target`, `backend`, `scratch`, `docs`, `ci`.

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
cargo test
cargo clippy
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
