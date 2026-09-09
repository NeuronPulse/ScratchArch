# LLVM Compatibility Benchmark

> Design/method for the **LLVM Compatibility Benchmark v0.1**.
> Companion to the [`LLVM_COMPATIBILITY_BASELINE.md`](./LLVM_COMPATIBILITY_BASELINE.md)
> (the v0.1 snapshot) and the normative matrix
> [`docs/specification/LLVM_COMPATIBILITY.md`](../specification/LLVM_COMPATIBILITY.md).
> Status: normative for the benchmark harness.

ScratchArch grows a long-term, repeatable way to answer one question at each
milestone: **how much of real, unmodified LLVM/clang programs makes it through
each layer of the toolchain — and what stops the rest?**

```text
.C → clang -O0 → .ll → LLVM parser → SAIR (validate) → optimizer
                                                          ↘ SAIR interpreter ── reference result
                                                          ↘ ISA lowering → ISA VM
                                                          ↘ ScratchGraph backend
```

The benchmark lives in `scratcharch-compat` — an **external measurement layer**
on top of the toolchain crates. No frontend, optimizer, interpreter, lowering, or
backend crate contains benchmark logic, and the benchmark never changes the
frozen ISA or a backend merely to raise a percentage.

## 1. What is measured

A **fixture** is one committed LLVM IR program plus the C source it was compiled
from, a set of **feature tags**, and a recorded **expectation** (the manifest,
Part 1). The corpus is the union of the existing committed real-clang fixtures
(`tests/c_programs/*.{c,ll}`) and feature-gap fixtures under
`tests/corpus/llvm/fixtures/` — real clang `-O0` output, committed, so the gate
does not depend on a compiler being present (recompilable with `--fresh-clang`).

The corpus is deliberately small and sharp rather than large and shallow. Each
fixture is a *capability probe* tagged with the LLVM features it exercises; the
feature taxonomy is fixed (`integer, i64, bitwise, shift, memory, global, phi,
switch, intrinsic, pointer, struct, array, float, vector, indirect-call,
atomic`).

## 2. The staged pipeline

`scratcharch test-compat` (and `scratcharch-compat::runner`) walks every fixture
through the layers and records a three-way outcome **per stage**:

| Stage | What runs | Outcome meaning |
|---|---|---|
| Parser | `scratcharch-llvm` lexer + parser | parsed? |
| SAIR | translator + `IrModule::validate()` | validated SAIR produced? |
| Optimizer *(internal, no row)* | constant-fold + DCE | module survived? |
| Interpreter | `scratcharch-sair-interpreter` | produced the reference result? |
| VM | `scratcharch-ir::lower` → `scratcharch-vm` | ISA lowering + execution exact? |
| Scratch | `scratcharch-scratchgraph::lower` | a ScratchGraph project constructed? |

Each stage records **PASS / FAIL / UNSUPPORTED**, never a silent skip:

- **Unsupported** — a *capability gap*: the toolchain met a real LLVM construct
  it does not implement and rejected it with a diagnostic that names the missing
  capability (`float`, a vector type, an `atomicrmw`, an `llvm.*` intrinsic…).
- **Fail** — a *genuine error*: validation rejected the translator's own output,
  an engine error that is not an unsupported construct, a wrong value, etc.
- **Pass** — the stage produced its correct result.

"Not implemented" and "implemented but wrong" are therefore never conflated. A
fixture's **result class** collapses the first blocking stage into the
nine-value classification: `parse-failure`, `sair-failure`,
`optimization-failure`, `interpreter-failure`, `lowering-failure`,
`vm-failure`, `scratch-backend-failure`, `semantic-mismatch` (a correctness
failure), and `success`. The `*failure` classes above the parser/SAIR front
always surface an `Unsupported` stage record when they are capability gaps.

## 3. Correctness channels

Wherever a fixture returns a value, the engines are cross-checked **full-width**
(the `main` return value) against each other and the manifest's
`expected_result`. The native reference is compiled from the committed `.c` and
compared **masked `& 0xFF`**, because its only interface is the process exit
status. A disagreement on any channel is a `semantic-mismatch` (or a native
mismatch), never a capability gap. A VM `i64` result is read back from its two
32-bit limb cells (high limb on top); the reconstruction is driven by the entry
function's declared return type.

## 4. Metrics

- **Stage row %** = fixtures passing the stage / total corpus. This is
  pass-through: a fixture blocked at the parser also reads as not-passed on
  every later row, which is exactly the "how far does the whole corpus get"
  question.
- **Overall %** = the **semantic core**: fixtures that computed a correct result
  on the reference interpreter (no mismatch against expected / native / VM),
  whether or not a target backend could build them. This is the headline
  "the toolchain is semantically faithful for X% of the corpus"; the backend
  rows then quantify ISA-VM and Scratch reach.
- **Feature %** = fixtures fully successful through Scratch with the tag /
  fixtures tagged with it.

A percentage is a statement about *whole programs reaching a stage*, never an
opcode-coverage count. **The benchmark percentage is not the opcode
percentage**: two matrices can both cover 100% of the instructions ScratchArch
accepts while the benchmark differs, because the benchmark counts real programs
that *complete* each layer, and programs die on the first construct their stage
cannot handle.

## 5. The manifest as a regression oracle

The manifest (`tests/corpus/llvm/manifest.json`) records, per fixture, the
stage it is first *not* expected to fully pass (`expected_stage`), whether that
boundary is a known capability gap or full success (`expected_status`), and
optionally the exact VM value (`vm.exact`) or a rejection diagnostic that must
appear (`vm.rejected` / `scratch.rejected`). The runner's gate is
**actual outcome == recorded expectation**:

- stages strictly before `expected_stage` must pass;
- a `success` fixture must pass Scratch too;
- an `unsupported` fixture must be Unsupported at exactly its boundary, with the
  pinned diagnostic present;
- an expected VM value must match exactly.

Any drift is a **violation** with a stable kind (`semantic-mismatch`,
`native-mismatch`, `unexpected-block`, `unexpected-success`,
`diagnostic-mismatch`, `missing-unsupported`, `not-measured`) and the fixture's
diagnostic. A red gate exits non-zero. **There is no silent fallback and no
deleting failing fixtures**: a fixture that starts failing is a regression to be
fixed, a fixture whose expectation no longer reflects reality is a conscious,
documented manifest change that lands with the milestone that justifies it.

## 6. Regression policy

The corpus is a **formal gate** for the ScratchArch repo. Every milestone that
touches a layer the benchmark measures must land with the full corpus green
(no expectation violations). Unsupported fixtures are not expected to turn green
— they are expected to stay *exactly as unsupported as recorded*, at the same
stage, with the same diagnostic. A milestone that extends a layer updates the
affected fixtures' expectations and moves them up the stage ladder.

## 7. Outputs

- **Text dashboard** (default): fixed 20-cell character progress bars (0% all
  empty, 100% all full; `█/░` on a terminal, `#/.` otherwise), grouped as
  Frontend (Parser, SAIR) / Execution (Interpreter, VM) / Targets (Scratch),
  with Overall, the feature breakdown, the unsupported-feature summary, any
  regressions, and the gate verdict.
- **JSON** (`--format json`): machine record with `total`, per-stage and
  per-feature numbers, `failures`, `classes`, and per-fixture stage
  diagnostics. No progress bars in JSON.
- **Baseline** (see `LLVM_COMPATIBILITY_BASELINE.md` and
  `tests/corpus/llvm/results/`): `vX.Y.json` snapshots + `latest.json`,
  regenerated at milestone gates.

## 8. Command-line

```text
scratcharch test-compat [--feature TAG]… [--stage STAGE]…
                        [--fresh-clang] [--no-native]
                        [--format text|json] [--quiet|--verbose]
                        [--ascii] [--corpus-root DIR]
```

- `--feature TAG` — only fixtures carrying the canonical feature tag.
- `--stage STAGE` — only fixtures whose recorded first non-fully-expected stage
  is `STAGE` (`parser | sair | interpreter | vm | scratch`). A drill-down, not a
  depth bound: selected fixtures still run the full pipeline.
- `--fresh-clang` — recompile each `.c` with clang before running (committed
  `.ll` is the fallback when clang is absent).
- `--no-native` — skip the native differential.
- `--quiet` — compact text: headline, class counts, regressions.
- `--verbose` — text with per-fixture stage detail.
- The corpus root is auto-discovered (explicit `--corpus-root`, the
  `SCRATCHARCH_COMPAT_ROOT` env var, an upward search from the working
  directory, then the build-time workspace root).

Exit status is zero when the gate is green and non-zero when a fixture drifted
from its manifest expectation. `scratcharch-cli` is the only thin dispatch layer;
all measurement logic lives in `scratcharch-compat`.

## 9. How to run the gate

```bash
cargo run -p scratcharch-cli --bin scratcharch -- test-compat           # text
cargo run -p scratcharch-cli --bin scratcharch -- test-compat --format json
cargo test -p scratcharch-compat                                        # unit tests
```

See `tests/corpus/llvm/manifest.json` for the oracle and
`docs/design/LLVM_COMPATIBILITY_BASELINE.md` for the recorded v0.1 numbers.
