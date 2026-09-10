# Compatibility Gap Analysis

> Benchmark-driven priority analysis for the LLVM Compatibility Benchmark,
> milestone **v0.4** ("ScratchArch Compatibility Expansion").
> Grounding data: `scratcharch test-compat --format json` over the **v0.2**
> corpus state (33 fixtures, corpus `tests/corpus/llvm/manifest.json`, recorded
> as `tests/corpus/llvm/results/v0.2.json`).
> Companion analyses: [`SCRATCH_NUMERIC_MODEL.md`](./SCRATCH_NUMERIC_MODEL.md)
> (Scratch representability), [`FUNCTION_POINTERS.md`](./FUNCTION_POINTERS.md).

## 1. Method

Every fixture is walked through the staged pipeline (Parser → SAIR →
Interpreter → VM → Scratch) and each stage records `PASS` / `FAIL` /
`UNSUPPORTED`, never a silent skip. A fixture's class is its first blocking
stage. A stage that rejects a construct *because the toolchain does not
implement it* is `unsupported` and must carry a diagnostic naming the missing
capability; a wrong value or an engine error that is not an unsupported
construct is a *failure*.

This document ranks the unsupported constructs **not by percentage impact** but
by the criteria below, so the implementation order follows engineering value,
not score cosmetics. Semantic mismatches are zero in the current corpus.

## 2. Current state (v0.2 corpus, 33 fixtures)

| Metric | Value |
|---|---|
| Corpus | 33 (20 `tests/c_programs/` + 13 `tests/corpus/llvm/fixtures/`) |
| Overall (semantic core) | 85% (28/33) |
| Parser / SAIR / Interpreter | 85% (28/33) |
| VM | 79% (26/33) |
| Scratch | 45% (15/33) |
| Result classes | success 14 · scratch-backend-failure 12 · parse-failure 5 · vm-failure 2 |
| Semantic mismatches | **0** |

Note on the Scratch row: 12 fixtures are Interpreter-exact **and** VM-exact but
stop at the Scratch backend. The dominant blocker is a single language feature
(width-changing casts); a smaller set is fundamentally unrepresentable (bitwise /
shift on the f64-only Scratch operator set). Section 5 decomposes this.

## 3. Gap inventory

| # | Construct | Blocking stage | Fixtures | Frequency | Cost | Class |
|---|---|---|---|---|---|---|
| G1 | Aggregate global initializers (`[N x {…}]`, `{…}`) | Parser | global-agg | 1 (grows with corpus) | Low–Med | **P0** |
| G2 | `__scratcharch_*` runtime intrinsic call (strlen) | VM | string | 1 | Low–Med | **P0** |
| G3 | Width casts `zext`/`sext`/`trunc`/`ptrtoint`/`inttoptr` | Scratch | signedcmp, i64arith, reinterp, byte-scan, char-mix, i64-struct, struct-array, ptrstruct, globals, bytes | 10 | Low | **P0** |
| G4 | `llvm.*` intrinsic execution path (bswap/ctpop family) | VM | intrinsics | 1 | Med | **P1** |
| G5 | Indirect calls (function-pointer dispatch) | Parser | indirect-call | 1 | Med–High | **P2** |
| G6 | Bitwise + shift ops (`and/or/xor/shl/lshr/ashr`) | Scratch | bitwise, i64muldiv | 2 | High (unrepresentable without new blocks) | **P3** |
| G7 | Float/double types | Parser | float | 1 | High | **P3** |
| G8 | Vector types | Parser | vector | 1 | High | **P3** |
| G9 | Atomic (`atomicrmw`) | Parser | atomic | 1 | High | **P3** |
| G10 | struct-field GEP offsets (VM lowerer) | — | — | 0 (dead path today) | Low | **P3** |

### Detail per gap

**G1 — Aggregate globals.** `global-agg.c` needs `@table = global [3 x {i32, i32}]
{{1,2},{3,4},{5,6}}` and a single-struct global. The parser rejects the
aggregate constant form, so the whole pipeline is `not-measured` downstream.
Exact byte layout (little-endian, struct padding via DataLayout) must be seeded
into the static-data segment; the VM's `vm.exact: 51` is already recorded.

**G2 — Runtime strlen on the VM.** `string.c` calls `__scratcharch_strlen`.
The SAIR interpreter resolves it through the runtime-intrinsic dispatch; the VM
reports "undefined function". Prefer linking the runtime operation to a
VM-executable helper (see Part 2); do not add libc-specific ISA instructions.

**G3 — Width casts on the Scratch backend.** Every one of the ten failing
fixtures stops here. Concretely, the SAIR cast ops the lowerer currently rejects:
`zext i1→i32` (signedcmp, i64arith, reinterp), `zext i8→i32` (reinterp, bytes),
`sext i8→i32` (byte-scan, globals), `sext i16→i32` (char-mix),
`sext i32→i64` (struct-array, ptrstruct), `trunc i64→i32` (i64arith, i64-struct),
`inttoptr i64` (reinterp). All are exact in the Scratch f64 model under the
SAFE_SUBSET contract (values within the f64-exact integer range) — see
`SCRATCH_NUMERIC_MODEL.md`. Implementation cost is a handful of expression
templates in `scratcharch-scratchgraph/src/lower.rs`.

**G4 — `llvm.*` intrinsic VM execution.** ✅ **Closed.** `intrinsics.c` needs
`llvm.ctpop.i32`/`llvm.bswap.*` to run on the VM. The VM now resolves these
bodyless names at load time and evaluates them through the shared
`scratcharch_runtime::bit_intrinsic_value` leaf, so the VM produces the
interpreter's exact value (RUNTIME.md §4.1/§6). The rubric's guess was that each
family would need "a VM-executable helper or an interpreter-only
classification" — neither was needed: one shared leaf plus load-time resolution
covers all four families at every width.

**G5 — Indirect calls.** `indirect-call.c` needs function-id + dispatch-table.
Re-evaluated against the current ScratchGraph runtime ABI in
`FUNCTION_POINTERS.md`; remains deferred while blockers stand.

**G6 — Bitwise/shift on Scratch.** Scratch 3.0's operator set has no
bitwise/shift blocks, so `ashr`/`lshr` have no exact f64 lowering. Arithmetic
emulation of i32 bitwise is possible in principle (i32 fits f64 exactly) but
high-cost and fragile; deferred. The two fixtures keep their explicit
`UnsupportedInstruction` diagnostic.

**G7–G9 — Float / vector / atomic.** Need type-system and engine work far
beyond this milestone; deferred with explicit diagnostics already present.

**G10 — StructField GEP path.** `scratcharch-ir/src/lower.rs` rejects
`GepIndex::StructField` ("require type layout"), but the LLVM translator always
folds GEP to a single `Dynamic` byte offset before SAIR is built, so the path is
dead today. Tracked so the explicit diagnostic stays if the folding is ever
bypassed.

## 4. Ranking rationale

- **P0 — low/moderate implementation cost, real programs unblocked.**
  G1 + G2 + G3 are the three "many fixtures / one feature" gaps: G3 alone
  covers ten fixtures, G1 and G2 each unblock a full pipeline. Each is
  contained, exact, and regression-testable.
- **P1 — moderate cost, smaller surface.** G4 (llvm.* intrinsics on the VM).
- **P2 — high cost or explicit deferral after study.** G5 (indirect calls —
  evaluated, blockers recorded in `FUNCTION_POINTERS.md`).
- **P3 — intentionally deferred.** G6 (unrepresentable on Scratch without new
  blocks), G7/G8/G9 (large type/engine work), G10 (dead path, keep diagnostic).

## 5. Frequency analysis (per feature)

Counted per fixture *first* blocked (a fixture counts once, at the feature that
stopped it):

| Feature | Blocked at parser | Blocked at VM | Blocked at Scratch | Count |
|---|---|---|---|---|
| global (aggregate init) | 1 | — | — | 1 |
| intrinsic (strlen) | — | 1 | — | 1 |
| cast (width/ptr) | — | — | 10 | 10 |
| intrinsic (llvm.*) | — | 1 | — | 1 |
| indirect-call | 1 | — | — | 1 |
| bitwise/shift | — | — | 2 | 2 |
| float / vector / atomic | 3 | — | — | 3 |

The Scratch row is dominated by casts (10/12), not by semantics that the Scratch
target lacks. The implementation order (G3 → G1 → G2 → G4) therefore closes the
largest real gap first.

## 6. Guardrails

- No percentage is chased: a fixture that cannot be represented exactly keeps
  its explicit diagnostic and its unsupported classification.
- No silent approximation: every newly supported construct is proven by the
  native/interpreter/VM differential (and the Scratch execution model where it
  applies) before its manifest expectation is raised.
- Denominator changes are reported explicitly (v0.2: 25 → 33; see
  `LLVM_COMPATIBILITY_BASELINE.md` History table).
