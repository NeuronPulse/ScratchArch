# LLVM Compatibility — Benchmark Baseline

> Snapshot: **v0.4 (current)** — 2026-09-10. Companion to the benchmark
> [`LLVM_COMPATIBILITY_BENCHMARK.md`](./LLVM_COMPATIBILITY_BENCHMARK.md) (method),
> the normative matrix
> [`docs/specification/LLVM_COMPATIBILITY.md`](../specification/LLVM_COMPATIBILITY.md),
> and the generated status report
> [`LLVM_COMPATIBILITY_STATUS.md`](./LLVM_COMPATIBILITY_STATUS.md). Machine
> baselines live under `tests/corpus/llvm/results/` (`latest.json` is the current
> gate; `v0.3.json`, `v0.2.json` and `v0.1.json` are the historical ones).

This is the reproducible baseline of the **LLVM Compatibility Benchmark v0.4**:
one committed corpus, one unified runner, recorded numbers. Every milestone that
touches the parser, translator, optimizer, interpreter, ISA lowering, ISA VM, or
ScratchGraph backend must keep this gate green (no expectation drift) and is
expected to move these percentages upward — or explain why not. Regenerate the
numbers at any time with:

```bash
cargo run -p scratcharch-cli --bin scratcharch -- test-compat
```

The v0.4 scores supersede the v0.3 baseline recorded in the History section
below (kept as history). Version numbering: the spec change log
(LLVM_COMPATIBILITY.md §10) and the corpus-benchmark baseline axis move
independently — the spec is at **v0.5** (the VM runtime resolver) while the
benchmark gate is at **v0.4**.

---

## v0.4 scores (current gate, 34 fixtures)

Bars are 20 cells (`█` filled, `░` empty); the numerator is explicit so a
percentage is never read as a bare score. `Overall` is the *semantic core* (see
metric definitions below).

```
Overall
  85% (29/34)  █████████████████░░░

Frontend
Parser      85% (29/34)  █████████████████░░░
SAIR        85% (29/34)  █████████████████░░░

Execution
Interpreter 85% (29/34)  █████████████████░░░
VM          85% (29/34)  █████████████████░░░

Targets
Scratch     76% (26/34)  ███████████████░░░░░
```

Gate: **green** (0 expectation violations — every fixture matched its recorded
manifest expectation). 0 semantic mismatches, 0 failures.

### Metric definitions

- **Stage rows** are pass-through: fixtures that pass the stage / 34 total. A
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
success                   26    full end-to-end through Scratch
scratch-backend-failure    3    correct on interpreter+VM; Scratch model cannot
                                express the construct (bitwise/shift, i64 helper
                                shifts, `and`)
parse-failure              5    float, vector, indirect-call, atomic, global-agg
                                (frontend gaps)
```

There is **no `vm-failure` class in v0.4**: the ISA VM now executes every
fixture the interpreter does. The two former VM gaps (`string`, `intrinsics`)
were interpreter-only runtime intrinsics with no `define`d body to call; the
VM's load-time runtime resolver (§ below) resolves them through the same
`scratcharch-runtime` registry the interpreter uses.

Reading the v0.4 numbers: **every loss before the VM is frontend.** 29/34
fixtures parse, translate, and compute the correct result — the 5 that do not
are parser capability gaps (`float`, `vector`, `indirect-call`, `atomic`,
`global-agg`). The ISA VM then executes all 29 of them exactly, bit-for-bit
against the interpreter. Scratch constructs a project for 26/34 — every
single-width program plus the byte-exact memory model's stores/loads, and now
the runtime-intrinsic fixtures (`string`, `intrinsics`) whose `llvm.bswap`/`zext`
shapes the byte-exact model handles; only the bitwise/shift family and the i64
software-helper shifts that feed on it remain unrepresentable.

## Feature breakdown

Percent = fixtures **fully successful through Scratch** with that tag / fixtures
tagged with it. A low value here is *expected* for a capability-gap feature
(`bitwise`, `shift`) whose tagged fixtures compute correctly on
the interpreter and VM but cannot yet build a Scratch project.

```
integer         84%  █████████████████░░░
i64             75%  ███████████████░░░░░
bitwise          0%  ░░░░░░░░░░░░░░░░░░░░
shift            0%  ░░░░░░░░░░░░░░░░░░░░
memory          88%  ██████████████████░░
global          50%  ██████████░░░░░░░░░░
phi            100%  ████████████████████
switch         100%  ████████████████████
intrinsic      100%  ████████████████████
pointer         88%  ██████████████████░░
struct          88%  ██████████████████░░
array           60%  ████████████░░░░░░░░
float            0%  ░░░░░░░░░░░░░░░░░░░░
vector           0%  ░░░░░░░░░░░░░░░░░░░░
indirect-call    0%  ░░░░░░░░░░░░░░░░░░░░
atomic           0%  ░░░░░░░░░░░░░░░░░░░░
```

## Unsupported features (known capability gaps, v0.4)

Counted across fixtures tagged with each feature that are *known-unsupported* at
their recorded boundary (never an implemented-but-wrong construct). These grow
when the corpus grows, so read them together with the feature-success rows:

```
integer           4
array             2
global            2
memory            2
atomic            1
bitwise           1
float             1
i64               1
indirect-call     1
pointer           1
shift             1
struct            1
vector            1
```

`intrinsic` is absent from this table for the first time: every intrinsic-tagged
fixture is now fully successful end-to-end.

## Failure diagnostics preserved (v0.4, non-regressions)

Each *known* rejection keeps the diagnostic that names the missing capability —
the two flavours of "did not work" are never conflated:

| Fixture | First non-fully-expected stage | Pinned diagnostic |
|---|---|---|
| `float` | parser | `unsupported type: Ident("double")` |
| `vector` | parser | `unsupported type: Ident("<")` |
| `indirect-call` | parser | indirect call unsupported: the SAIR/ISA call ABI requires a statically-named callee (no function-pointer ABI) |
| `atomic` | parser | `unsupported instruction: atomicrmw` |
| `global-agg` | parser | `parse error: unterminated global array initializer` |
| `bitwise` | scratch | `unsupported instruction: ashr cannot be lowered to Scratch numbers` |
| `i64muldiv` | scratch | `unsupported instruction: lshr cannot be lowered to Scratch numbers` |
| `bytes` | scratch | `unsupported instruction: and cannot be lowered to Scratch numbers` |

Every row is a **Scratch-backend model limit** or a **frontend gap**; the two
interpreter-only runtime-intrinsic rows that v0.3 carried (`string` →
`undefined function: __scratcharch_strlen`, `intrinsics` → `undefined function:
llvm.bswap.i16`) are gone — those calls now resolve on the ISA VM. See
`§6` of LLVM_COMPATIBILITY.md for the normative per-construct matrix.

## What moved between v0.3 and v0.4

- **VM 79% → 85%** (26/33 → 29/34). The ISA VM gained a **load-time runtime
  resolver**: a bodyless named `Call` is resolved once, per name, per program
  against the shared `scratcharch-runtime` registry and rewritten to an internal
  `Code::CallRuntime` entry — never a per-execution string dispatch, and never a
  silent approximation. An unresolvable name stays a load-time
  `undefined function: <name>`.
- **No `vm-failure` class remains.** `string` and `intrinsics` — the two
  interpreter-only runtime intrinsics — are now full successes on all four
  measured stages.
- **Corpus +1 (`runtime_mem`).** A new fixture exercising *runtime-length*
  memory operations (the translator inlines only constant-length ones, so these
  survive translation as calls) and every SART string builtin
  (`memcmp`/`memset`/`memmove`/`strcmp`/`strcpy`/`strncpy`/`strlen`, plus
  `memcpy`) in one program. It is interpreter-exact, VM-exact, and
  native-exact (255).
- **`intrinsic` 50% → 100%**; `integer` 79% → 84%, `memory` 81% → 88%,
  `pointer` 71% → 88% (the new fixture carries those tags and succeeds).
- **Parser/SAIR/Interpreter unchanged** at 85% — the runtime work touched only
  the VM's call resolution and the shared runtime crate. The semantic core
  moved 28/33 → 29/34 (the new fixture is a correct program).
- **Scratch's percentage shifted 76% → 76%** but its numerator grew 25 → 26:
  the new fixture constructs, and the denominator grew by one.

### The runtime mechanisms this milestone added

Both engines now share one runtime model (normative detail in
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

## History

| Milestone | Date | Fixtures | Overall | Parser | SAIR | Interpreter | VM | Scratch | Gate |
|---|---|---|---|---|---|---|---|---|---|
| **v0.4** | 2026-09-10 | 34 | 85% | 85% | 85% | 85% | 85% | 76% | green |
| **v0.3** | 2026-09-09 | 33 | 85% | 85% | 85% | 85% | 79% | 76% | green |
| **v0.2** | 2026-09-09 | 33 | 85% | 85% | 85% | 85% | 79% | 45% | green |
| **v0.1** | 2026-09-09 | 25 | 84% | 84% | 84% | 84% | 68% | 52% | green |

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
