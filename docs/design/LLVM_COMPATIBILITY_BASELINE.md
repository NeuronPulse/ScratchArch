# LLVM Compatibility — Benchmark Baseline

> Snapshot: **v0.6 (current)** — 2026-09-10. Companion to the benchmark
> [`LLVM_COMPATIBILITY_BENCHMARK.md`](./LLVM_COMPATIBILITY_BENCHMARK.md) (method),
> the normative matrix
> [`docs/specification/LLVM_COMPATIBILITY.md`](../specification/LLVM_COMPATIBILITY.md),
> and the generated status report
> [`LLVM_COMPATIBILITY_STATUS.md`](./LLVM_COMPATIBILITY_STATUS.md). Machine
> baselines live under `tests/corpus/llvm/results/` (`latest.json` is the current
> gate; `v0.5.json`, `v0.4.json`, `v0.3.json`, `v0.2.json` and `v0.1.json` are the
> historical ones).

This is the reproducible baseline of the **LLVM Compatibility Benchmark v0.6**:
one committed corpus, one unified runner, recorded numbers. Every milestone that
touches the parser, translator, optimizer, interpreter, ISA lowering, ISA VM, or
ScratchGraph backend must keep this gate green (no expectation drift) and is
expected to move these percentages upward — or explain why not. Regenerate the
numbers at any time with:

```bash
cargo run -p scratcharch-cli --bin scratcharch -- test-compat
```

The v0.6 scores supersede the v0.5 baseline recorded in the History section
below (kept as history). Version numbering: the spec change log
(LLVM_COMPATIBILITY.md §10) and the corpus-benchmark baseline axis move
independently and the spec log runs one ahead — the aggregate ABI is
**spec v0.7** / **baseline v0.6**, exactly as the aggregate data model was spec
v0.6 / baseline v0.5.

**Denominator change (v0.5 → v0.6): 40 → 49 fixtures.** Eight real-clang
aggregate-ABI fixtures (`abi-*`) were added, plus the `abi-odd-width` boundary
fixture that pins the `i24` rejection. Read every percentage below against the
new denominator: the semantic core grows 36 → 44 *and* the denominator grows, so
the movement in percentage is smaller than the movement in absolute count.

---

## v0.6 scores (current gate, 49 fixtures)

Bars are 20 cells (`█` filled, `░` empty); the numerator is explicit so a
percentage is never read as a bare score. `Overall` is the *semantic core* (see
metric definitions below).

```
Overall
  90% (44/49)  ██████████████████░░

Frontend
Parser      90% (44/49)  ██████████████████░░
SAIR        90% (44/49)  ██████████████████░░

Execution
Interpreter 90% (44/49)  ██████████████████░░
VM          90% (44/49)  ██████████████████░░

Targets
Scratch     84% (41/49)  █████████████████░░░
```

Gate: **green** (0 expectation violations — every fixture matched its recorded
manifest expectation). 0 semantic mismatches, 0 failures.

### Metric definitions

- **Stage rows** are pass-through: fixtures that pass the stage / 49 total. A
  fixture blocked at an early layer also reads as not-passed on every later
  layer.
- **Overall** is the *semantic core*: fixtures whose program computed a correct
  result on the reference interpreter (interpreter passed, no value mismatch
  against the expected result / native / VM) — whether or not a target backend
  could build them. Distinct from any single pass-through row by design.
- A fixture is only counted at a stage when the stage was actually measured;
  blocked fixtures never silently pass a downstream layer.

### Result classes

```
success                   41    full end-to-end through Scratch
scratch-backend-failure    3    correct on interpreter+VM; Scratch model cannot
                                express the construct (bitwise/shift, i64 helper
                                shifts, `and`)
parse-failure              5    float, vector, indirect-call, atomic (frontend
                                gaps) and abi-odd-width (the i24 boundary)
```

There is **no `vm-failure` class** (since v0.4): the ISA VM executes every
fixture the interpreter does.

Reading the v0.6 numbers: **every loss before the VM is frontend.** 44/49
fixtures parse, translate, and compute the correct result; the 5 that do not are
parser capability gaps (`float`, `vector`, `indirect-call`, `atomic`, and the
deliberately-refused `i24` width). The ISA VM then executes all 44 of them
exactly, bit-for-bit against the interpreter. Scratch constructs a project for
41/49 — every single-width program, the byte-exact memory model's
stores/loads, the runtime-intrinsic fixtures, every aggregate fixture, and now
**every aggregate-ABI fixture**: aggregates are a *layout* concern that reaches
the backends as `DataLayout` byte offsets plus scalar leaves, and the ABI adds
only a pointer cell plus byte copies over that same heap. Only the bitwise/shift
family and the i64 software-helper shifts that feed on it remain
unrepresentable.

## Feature breakdown

Percent = fixtures **fully successful through Scratch** with that tag / fixtures
tagged with it. A low value here is *expected* for a capability-gap feature
(`bitwise`, `shift`) whose tagged fixtures compute correctly on
the interpreter and VM but cannot yet build a Scratch project.

```
integer         92%  ██████████████████░░
i64             80%  ████████████████░░░░
bitwise          0%  ░░░░░░░░░░░░░░░░░░░░
shift            0%  ░░░░░░░░░░░░░░░░░░░░
memory          97%  ███████████████████░
global          88%  ██████████████████░░
phi            100%  ████████████████████
switch         100%  ████████████████████
intrinsic      100%  ████████████████████
pointer         91%  ██████████████████░░
struct          96%  ███████████████████░
array           89%  ██████████████████░░
aggregate-abi   89%  ██████████████████░░
struct-param    86%  █████████████████░░░
struct-return  100%  ████████████████████
nested-aggregate 100% ████████████████████
float            0%  ░░░░░░░░░░░░░░░░░░░░
vector           0%  ░░░░░░░░░░░░░░░░░░░░
indirect-call    0%  ░░░░░░░░░░░░░░░░░░░░
atomic           0%  ░░░░░░░░░░░░░░░░░░░░
```

`struct-return` and `nested-aggregate` reach **100%**: every fixture tagged with
them builds end-to-end through Scratch. `struct-param` is 86% and `aggregate-abi`
89% because the one intentionally-rejected fixture (`abi-odd-width`, the `i24`
boundary) carries both tags — a *documented* refusal, not a silent failure.
(`bitwise`/`shift` stay at 0% because their single-tagged fixtures are the
Scratch-backend gaps `bitwise` and `i64muldiv`.)

## Unsupported features (known capability gaps, v0.6)

Counted across fixtures tagged with each feature that are *known-unsupported* at
their recorded boundary (never an implemented-but-wrong construct). These grow
when the corpus grows, so read them together with the feature-success rows:

```
integer            3
aggregate-abi      1
array              1
atomic             1
bitwise            1
float              1
global             1
i64                1
indirect-call      1
memory             1
pointer            1
shift              1
struct             1
struct-param       1
vector             1
```

`intrinsic`, `phi`, `switch`, `struct-return` and `nested-aggregate` are absent
from this table — every fixture tagged with them is fully successful
end-to-end.

## Failure diagnostics preserved (v0.6, non-regressions)

Each *known* rejection keeps the diagnostic that names the missing capability —
the two flavours of "did not work" are never conflated:

| Fixture | First non-fully-expected stage | Pinned diagnostic |
|---|---|---|
| `float` | parser | `unsupported type: Ident("double")` |
| `vector` | parser | `unsupported type: Ident("<")` |
| `indirect-call` | parser | indirect call unsupported: the SAIR/ISA call ABI requires a statically-named callee (no function-pointer ABI) |
| `atomic` | parser | `unsupported instruction: atomicrmw` |
| `abi-odd-width` | parser | `integer width i24 is not supported: SAIR represents i1, i8, i16, i32 and i64 only` |
| `bitwise` | scratch | `unsupported instruction: ashr cannot be lowered to Scratch numbers` |
| `i64muldiv` | scratch | `unsupported instruction: lshr cannot be lowered to Scratch numbers` |
| `bytes` | scratch | `unsupported instruction: and cannot be lowered to Scratch numbers` |

`global-agg`'s v0.2-era parser diagnostic (`parse error: unterminated global
array initializer`) is gone: aggregate global initializers now lay out into the
static-data image. Every remaining row is a **Scratch-backend model limit**, a
**frontend gap**, or a **deliberate ABI boundary**. See `§6` of
LLVM_COMPATIBILITY.md for the normative per-construct matrix,
`AGGREGATE_DATA_MODEL.md` for the aggregate layout contract and
`AGGREGATE_ABI.md` for the aggregate parameter/return ABI.

## What moved between v0.5 and v0.6

- **Corpus 40 → 49.** Eight new real-clang aggregate-ABI fixtures —
  `abi-struct-param` (57), `abi-struct-return` (66), `abi-nested-param` (125),
  `abi-nested-return` (126), `abi-i64-field` (7), `abi-ptr-field` (36),
  `abi-multi-agg` (119), `abi-mixed-args` (87) — each with its `.c` and
  clang-`-O0` `.ll` committed and recompiled on every test run, plus the
  `abi-odd-width` boundary fixture.
- **Overall holds 90% (36/40 → 44/49); Scratch 83% → 84% (33/40 → 41/49).**
  Parser/SAIR/Interpreter/VM all remain 90%.
- **New feature rows:** `aggregate-abi` 89%, `struct-param` 86%,
  `struct-return` 100%, `nested-aggregate` 100%.
- **No regression:** every pre-v0.6 fixture keeps its recorded expectation and
  its value; `scratch-backend-failure` stays 3, `parse-failure` 4 → 5 (the new
  odd-width boundary).
- **0 semantic mismatches** across the native/interpreter/VM differential (all
  49 fixtures) and the LLVM → ScratchGraph construction record.

### The aggregate-ABI mechanisms this milestone added

Normative detail in `docs/design/AGGREGATE_ABI.md`, building on the v0.5
aggregate data model:

- **An aggregate value is a temporary slot.** An aggregate-typed SSA value is
  backed by a compiler-managed `alloca` whose extent is `DataLayout::size`;
  every move (`load`, `store`, `insertvalue`, nested `extractvalue`, the `byval`
  prologue) is a byte-exact copy of that extent. `IrType` gains no aggregate
  variant.
- **An aggregate crosses a call as one pointer cell.** clang's memory-class
  `sret(%T)`/`byval(%T)` pointer is an ordinary explicit parameter passed
  through; a `byval` parameter is copied into a private callee slot in the
  prologue and the parameter name is re-bound to the copy. A record clang
  returns *in registers* gets a synthesized hidden result pointer appended after
  the explicit parameters, the function returning `void`.
- **Fresh slots, never aliased.** `insertvalue` is a functional update (fresh
  slot, whole-operand copy, one leaf patched) and each call site allocates its
  own result slot, so two calls never share storage.
- **Explicit rejection, never flattening.** Non-power-of-two widths
  (`i24`/`i40`/`i48`) and aggregate-returning declarations are refused by name;
  an aggregate in a genuinely scalar position is still rejected.
- **Nothing new in the VM or Scratch.** The ABI lowers to the byte-exact copy
  and scalar-leaf primitives v0.5 established — no aggregate instruction, no
  aggregate `IrType`, and no second aggregate ABI on the Scratch side.

## What moved between v0.4 and v0.5

- **Corpus 34 → 40.** Six new real-clang aggregate fixtures —
  `aggstruct` (whole-struct `memcpy` assignment over a padded layout),
  `aggarray` (array of structs, struct containing an array),
  `aggglobal` (global struct initializers with interior padding and a pointer
  field), `aggmatrix` (global nested arrays and arrays of strings),
  `aggnested` (struct-of-array-of-structs global), `aggbytes` (byte view of
  aggregate memory) — each with its `.c` and clang-`-O0` `.ll` committed and
  recompiled on every test run.
- **`global-agg` parser gap closed.** `expected_stage` moves `parser` →
  `scratch`, `expected_status` `unsupported` → `success`; the pinned diagnostic
  row is removed.
- **Overall 85% → 90% (29/34 → 36/40); Scratch 76% → 83% (26/34 → 33/40).**
  Parser/SAIR/Interpreter/VM all 85% → 90%.
- **`struct` 88% → 100%**, `memory` 88% → 96%, `global` 50% → 88%,
  `array` 60% → 89%, `integer` 84% → 90%, `pointer` 88% → 90%.
- **No regression:** every pre-v0.5 fixture keeps its recorded expectation and
  its value; `parse-failure` 5 → 4 and `scratch-backend-failure` stays 3.
- **0 semantic mismatches** across the native/interpreter/VM differential (all
  40 fixtures) and the LLVM → ScratchGraph construction record.

### The aggregate mechanisms this milestone added

Normative detail in `docs/design/AGGREGATE_DATA_MODEL.md`, with the type model
in `scratcharch-target::layout`:

- **`DataLayout` is the single authority for aggregate geometry.**
  `AggregateType` (`Scalar` / `Array` / `Struct`) plus `TypeLayout`
  (`size`/`align`/`field_offsets`) answer scalar size, scalar alignment, array
  stride, struct alignment, struct field offsets, aggregate size and padding.
  The translator holds no layout arithmetic of its own: `type_size_align`,
  global seeding and GEP offsets all call into `DataLayout`.
- **No aggregate `IrType` variant.** Aggregates are never *values* — an
  aggregate load/store is a layout question over scalar leaves, so `IrType`
  stays scalar and `Copy` (Part 10 defers aggregate-by-value ABI).
- **Aggregate global initializers become a byte image.** The parser accepts
  nested `{ … }`/`[ … ]`/`c"…"` constants into `LlvmGlobalInit::Struct`/
  `Array`/`Bytes`; the translator writes them into `StaticData.image`
  little-endian at `DataLayout` offsets, leaving padding zero.
- **Explicit rejection, never flattening.** An aggregate *value* type at an
  operation has no SAIR representation and is rejected with a named diagnostic;
  a zero-sized aggregate (`[0 x T]`, `{}`) is rejected by `DataLayout` with
  `LayoutError::ZeroSized`.
- **Nothing new in the VM.** Aggregate memory lowers to the existing byte/word
  load/store path and the already-verified sub-word (`i1`/`i8`/`i16`) exact
  path; aggregation is a layout concern, not a new VM value representation.

## History

| Milestone | Date | Fixtures | Overall | Parser | SAIR | Interpreter | VM | Scratch | Gate |
|---|---|---|---|---|---|---|---|---|---|
| **v0.6** | 2026-09-10 | 49 | 90% | 90% | 90% | 90% | 90% | 84% | green |
| **v0.5** | 2026-09-10 | 40 | 90% | 90% | 90% | 90% | 90% | 83% | green |
| **v0.4** | 2026-09-10 | 34 | 85% | 85% | 85% | 85% | 85% | 76% | green |
| **v0.3** | 2026-09-09 | 33 | 85% | 85% | 85% | 85% | 79% | 76% | green |
| **v0.2** | 2026-09-09 | 33 | 85% | 85% | 85% | 85% | 79% | 45% | green |
| **v0.1** | 2026-09-09 | 25 | 84% | 84% | 84% | 84% | 68% | 52% | green |

### v0.4 historical snapshot

The v0.4 gate (34 fixtures) read: Overall 85% (29/34), Parser/SAIR/Interpreter/VM
85%, Scratch 76%; classes success 26 / scratch-backend-failure 3 /
parse-failure 5, with no `vm-failure` class. Its feature breakdown,
unsupported-feature counts, and pinned diagnostics are preserved in machine form
at `tests/corpus/llvm/results/v0.4.json` and in git history at this file's v0.4
revision. The milestone that produced it added the VM's load-time runtime
resolver: a bodyless named `Call` is resolved once, per name, per program,
against the shared `scratcharch-runtime` registry and rewritten to an internal
`Code::CallRuntime` entry — never a per-execution string dispatch, and never a
silent approximation. An unresolvable name stays a load-time
`undefined function: <name>`. Its runtime mechanisms (normative detail in
`docs/specification/RUNTIME.md` and EXECUTION_MODEL.md §5.7–§5.8):

- **SART builtins** (`__scratcharch_*`) carry a machine-readable
  `IntrinsicSignature` (operand-stack arity in words, result arity) so the VM
  knows how many cells a call consumes and leaves *before* it runs.
- **`llvm.mem*`** flat ops are realised with the interpreter's contiguous-range,
  null-destination, through-a-temporary semantics.
- **Bit intrinsics** (`llvm.bswap/ctpop/ctlz/cttz.iN`) evaluate through the
  shared `scratcharch_runtime::bit_intrinsic_value` leaf, so interpreter and VM
  cannot diverge (no poison: `ctlz`/`cttz` of 0 yield the width).
- **Failure categories stay distinct** on the VM: `Abort`, `Panic`, `Trap`
  (runtime), `DivisionByZero`, and the ISA `unreachable` trap. A runtime abort
  is never reported as an `unreachable`.

### v0.3 historical snapshot

The v0.3 gate (33 fixtures) read: Overall 85%, Parser/SAIR/Interpreter 85%,
VM 79%, Scratch 76%; classes success 23 / scratch-backend-failure 3 /
vm-failure 2 / parse-failure 5. Its feature breakdown, unsupported-feature
counts, and pinned diagnostics are preserved in machine form at
`tests/corpus/llvm/results/v0.3.json` and in git history at this file's v0.3
revision.

### v0.2 historical snapshot

The v0.2 gate (33 fixtures) read: Overall 85%, Parser/SAIR/Interpreter 85%,
VM 79%, Scratch 45%; classes success 14 / scratch-backend-failure 12 /
vm-failure 2 / parse-failure 5. Its feature breakdown, unsupported-feature
counts, and pinned diagnostics are preserved in machine form at
`tests/corpus/llvm/results/v0.2.json` and in git history at this file's v0.2
revision.

### v0.1 historical snapshot

The v0.1 gate (25 fixtures) read: Overall 84% (`█████████████████░░░`),
Parser/SAIR/Interpreter 84%, VM 68%, Scratch 52%; classes
success 10 / parse-failure 4 / vm-failure 4 / scratch-backend-failure 7. Its
feature breakdown, unsupported-feature counts, and pinned diagnostics are
preserved in machine form at `tests/corpus/llvm/results/v0.1.json` and in git
history at this file's v0.1 revision.

### Adding a milestone baseline

1. Land the milestone changes (keep every prior fixture green).
2. Re-run `scratcharch test-compat`; copy the machine report:
   `cp tests/corpus/llvm/results/latest.json tests/corpus/llvm/results/vX.Y.json`.
3. Bump `version` in `tests/corpus/llvm/manifest.json` to the new baseline.
4. Copy the dashboard above into this document under a new `vX.Y scores`
   heading, add a `vX.Y` row to History, demote the previous current snapshot to
   a historical paragraph, and note in the entry what moved and why.
