# LLVM Compatibility — Benchmark Baseline

> Snapshot: **v0.1** (2026-09-09). Companion to the benchmark
> [`LLVM_COMPATIBILITY_BENCHMARK.md`](./LLVM_COMPATIBILITY_BENCHMARK.md) (method),
> the normative matrix
> [`docs/specification/LLVM_COMPATIBILITY.md`](../specification/LLVM_COMPATIBILITY.md),
> and the generated status report [`LLVM_COMPATIBILITY_STATUS.md`](./LLVM_COMPATIBILITY_STATUS.md).
> Machine baselines live under `tests/corpus/llvm/results/`.

This is the reproducible baseline of the **LLVM Compatibility Benchmark v0.1**:
one committed corpus, one unified runner, recorded numbers. Every milestone that
touches the parser, translator, optimizer, interpreter, ISA lowering, ISA VM, or
ScratchGraph backend must keep this gate green (no expectation drift) and is
expected to move these percentages upward — or explain why not. Regenerate the
numbers at any time with:

```bash
cargo run -p scratcharch-cli --bin scratcharch -- test-compat
```

---

## Corpus

- **25 fixtures** — 20 committed real-clang `tests/c_programs/*.{c,ll}` plus 5
  feature-gap fixtures under `tests/corpus/llvm/fixtures/` (real clang `-O0`
  output committed for each).
- Feature taxonomy: `integer, i64, bitwise, shift, memory, global, phi, switch,
  intrinsic, pointer, struct, array, float, vector, indirect-call, atomic`.
- Native differential runs for every fixture with a committed `.c` (clang/gcc on
  PATH). Result channel: full-width `main` return value on the interpreter and
  VM; masked `& 0xFF` process exit for the native reference.

## v0.1 scores

Bars are 20 cells (`█` filled, `░` empty); 0% is all empty, 100% all filled.

```
Overall
  84%  █████████████████░░░

Frontend
Parser      84%  █████████████████░░░
SAIR        84%  █████████████████░░░

Execution
Interpreter 84%  █████████████████░░░
VM          68%  ██████████████░░░░░░

Targets
Scratch     52%  ██████████░░░░░░░░░░
```

### Metric definitions (v0.1)

- **Stage rows** are pass-through: fixtures that pass the stage / 25 total. A
  fixture blocked at an early layer also reads as not-passed on every later
  layer.
- **Overall** is the *semantic core*: fixtures whose program computed a correct
  result on the reference interpreter (interpreter passed, no value mismatch
  against the expected result / native / VM) — whether or not a target backend
  could build them. Distinct from any single pass-through row by design.
- A fixture is only counted at a stage when the stage was actually measured;
  blocked fixtures never silently pass a downstream layer.

Reading the v0.1 numbers: **every loss before the VM is frontend.** 21/25
fixtures parse, translate, and compute the correct result — the 4 that do not
(`float`, `vector`, `indirect-call`, `atomic`) are parser capability gaps. The
ISA VM then executes 17/25 exactly; the 4 it cannot run are interpreter-only
runtime intrinsics (`llvm.memcpy`, `llvm.bswap`, `__scratcharch_*`) with no
`define`d body to call. Scratch constructs a project for 13/25 — every
non-pointer, non-intrinsic program.

### Result classes

```
success                10    full end-to-end through Scratch
parse-failure           4    float, vector, indirect-call, atomic (frontend gaps)
vm-failure              4    string, memory, memintrin, intrinsics (runtime intrinsics)
scratch-backend-failure 7    bitwise, i64muldiv, globals, signedcmp, i64arith,
                             bytes, reinterp (correct on interpreter+VM; Scratch
                             model cannot express the construct)
```

Gate: **green** (0 expectation violations — every fixture matched its recorded
manifest expectation).

## Feature breakdown

Percent = fixtures **fully successful through Scratch** with that tag / fixtures
tagged with it. A low value here is *expected* for a capability-gap feature
(`i64`, `bitwise`, `shift`) whose tagged fixtures compute correctly on the
interpreter and VM but cannot yet build a Scratch project.

```
integer         53%  ███████████░░░░░░░░░
i64              0%  ░░░░░░░░░░░░░░░░░░░░
bitwise          0%  ░░░░░░░░░░░░░░░░░░░░
shift            0%  ░░░░░░░░░░░░░░░░░░░░
memory          38%  ████████░░░░░░░░░░░░
global           0%  ░░░░░░░░░░░░░░░░░░░░
phi             75%  ███████████████░░░░░
switch         100%  ████████████████████
intrinsic        0%  ░░░░░░░░░░░░░░░░░░░░
pointer         40%  ████████░░░░░░░░░░░░
struct         100%  ████████████████████
array           50%  ██████████░░░░░░░░░░
float            0%  ░░░░░░░░░░░░░░░░░░░░
vector           0%  ░░░░░░░░░░░░░░░░░░░░
indirect-call    0%  ░░░░░░░░░░░░░░░░░░░░
atomic           0%  ░░░░░░░░░░░░░░░░░░░░
```

## Unsupported features (known capability gaps)

Counted across fixtures tagged with each feature that are *known-unsupported* at
their recorded boundary (never an implemented-but-wrong construct):

```
integer           8
memory            5
intrinsic         4
i64               3
pointer           3
global            2
array             1
atomic            1
bitwise           1
float             1
indirect-call     1
phi               1
shift             1
vector            1
```

## Failure diagnostics preserved (v0.1, non-regressions)

Each *known* rejection keeps the diagnostic that names the missing capability —
the two flavours of "did not work" are never conflated:

| Fixture | First non-fully-expected stage | Pinned diagnostic |
|---|---|---|
| `float` | parser | `unsupported type: Ident("double")` |
| `vector` | parser | `unsupported type: Ident("<")` |
| `indirect-call` | parser | indirect call unsupported: the SAIR/ISA call ABI requires a statically-named callee (no function-pointer ABI) |
| `atomic` | parser | `unsupported instruction: atomicrmw` |
| `string` | vm | `undefined function: __scratcharch_strlen` |
| `memory` | vm | `undefined function: __scratcharch_memcpy` |
| `memintrin` | vm | `undefined function: llvm.memcpy.p0.p0.i64` |
| `intrinsics` | vm | `undefined function: llvm.bswap.i16` |
| `bitwise` | scratch | `unsupported instruction: ashr cannot be lowered to Scratch numbers` |
| `i64muldiv` | scratch | `unsupported instruction: lshr cannot be lowered to Scratch numbers` |
| `globals` | scratch | `unsupported instruction: sext i8 to i32 cannot be lowered to Scratch numbers` |
| `signedcmp` | scratch | `unsupported instruction: zext i1 to i32 cannot be lowered to Scratch numbers` |
| `i64arith` | scratch | `unsupported instruction: trunc i64 to i32 cannot be lowered to Scratch numbers` |
| `bytes` | scratch | `unsupported instruction: zext i8 to i32 cannot be lowered to Scratch numbers` |
| `reinterp` | scratch | `unsupported instruction: ptrtoint ptr to i64 cannot be lowered to Scratch numbers` |

These are the **Scratch-backend model limits** and the **interpreter-only runtime
intrinsic** paths recorded in the specification (`§6` of LLVM_COMPATIBILITY.md),
surfaced here as measured numbers rather than opcode counts.

## History

| Milestone | Date | Fixtures | Overall | Parser | SAIR | Interpreter | VM | Scratch | Gate |
|---|---|---|---|---|---|---|---|---|---|
| **v0.1** | 2026-09-09 | 25 | 84% | 84% | 84% | 84% | 68% | 52% | green |

### Adding a milestone baseline

1. Land the milestone changes (keep every prior fixture green).
2. Re-run `scratcharch test-compat`; copy the machine report:
   `cp tests/corpus/llvm/results/latest.json tests/corpus/llvm/results/vX.Y.json`.
3. Copy the dashboard above into this document under a new `vX.Y scores`
   heading, add a `vX.Y` row to History, and note in the entry what moved and
   why.
