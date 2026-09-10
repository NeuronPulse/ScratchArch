# LLVM Compatibility — Status Report

> Generated for the **v0.4 corpus-benchmark gate** (2026-09-10) — the VM runtime
> resolver closing the last `Interpreter only` gap, over the recorded v0.3
> baseline [`LLVM_COMPATIBILITY_BASELINE.md`](./LLVM_COMPATIBILITY_BASELINE.md).
> Companion to [`docs/specification/LLVM_COMPATIBILITY.md`](../specification/LLVM_COMPATIBILITY.md)
> (the normative matrix) and [`LLVM_TRANSLATION.md`](./LLVM_TRANSLATION.md) (the
> mapping design). This document records *where the toolchain currently stands*:
> what is exact, what is deliberately rejected, and the reason for each
> rejection. It is a snapshot, refreshed at milestone gates. Machine record:
> `tests/corpus/llvm/results/latest.json`.

## Execution surfaces (recap)

An LLVM construct crosses three surfaces before it runs:

| # | Surface | Role |
|---|---------|------|
| 1 | Frontend (`parser.rs` + `translator.rs`) | LLVM text → validated SAIR `IrModule` |
| 2 | SAIR interpreter (`scratcharch-sair-interpreter`) | Semantic reference; every translated construct is exact here |
| 3 | SA48 VM backend (`scratcharch-ir::lower` + `scratcharch-vm`) | Frozen 32-bit-word ISA; full-width `i64` arithmetic realised by program-level software helpers appended during lowering (EXECUTION_MODEL.md §5.7–§5.8); bodyless runtime/intrinsic calls resolved at load time through the shared `scratcharch-runtime` registry (spec §6.1, `RUNTIME.md`) |

A separate axis — the **Scratch backend** (`scratchgraph::lower`) — constructs
ScratchGraph projects with a **byte-exact, width-aware memory model**
(`docs/specification/SCRATCH_MEMORY.md`): one list item per byte,
little-endian, `i1`/`i8` = 1 byte … `i64` = 8, `ptr` = 4, with exact static-data
seeding. It remains a *construction-only* surface for LLVM input: the Scratch
execution model cannot express pointers or arbitrary call frames, so
LLVM→Scratch *execution* semantics are out of scope (§6.2 of the spec) until a
standalone reference executor exists (see §5 of
`SCRATCH_NUMERIC_MODEL.md`).

A matrix row is **`Supported`** only when surfaces 1, 2, *and* 3 are all exact.
Anything the VM cannot execute faithfully is rejected with a diagnostic that
names the missing ISA/runtime capability — never approximated.

## v0.4 scores (34 fixtures)

Bars are 20 cells; the denominator is explicit so no percentage reads as a bare
score. `Overall` is the *semantic core* (correct on the reference interpreter,
whether or not a target backend could build it); each stage row is fixtures that
pass *through* that stage.

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

Gate: **green** (0 expectation violations). 0 semantic mismatches, 0 failures.

Result classes:

```
success                   26    full end-to-end through Scratch
scratch-backend-failure    3    correct on interpreter+VM; Scratch model cannot
                                express the construct (bitwise/shift, i64 helper
                                shifts, `and`)
parse-failure              5    frontend gaps (float, vector, indirect-call,
                                atomic, global-agg)
```

There is **no `vm-failure` class**: the ISA VM executes every fixture the
interpreter does.

## Top gaps (ordered by how often real `-O0` C hits them)

### 1. Floating point (parser) — `float`/`double`

`unsupported type: Ident("double")`. The single most common real-C construct the
frontend cannot consume. Feasibility study: [`FLOATING_POINT.md`](./FLOATING_POINT.md)
— recommends option **C** (interpreter-exact `f64` plus `f32`-by-emulation)
near-term, with **A** (VM-exact) deferred until the interpreter model is proven.

### 2. Aggregate-constant global tables (parser) — `global-agg`

`parse error: unterminated global array initializer` on
`@t = [3 x %struct.Pair] [%struct.Pair { i32 1, i32 2 }, …]`. Static tables of
structs/arrays are idiomatic real C. Scalar and flat-array-of-scalar globals are
supported; struct/nested-aggregate constant *elements* are the gap (spec §8.5).

### 3. Function pointers / indirect calls (parser) — `indirect-call`

Callbacks, vtables, and function tables need an indirect-call ABI; the SAIR/ISA
`Call` names a static callee. Rejected explicitly — never a wrong dispatch.
Feasibility study: [`FUNCTION_POINTERS.md`](./FUNCTION_POINTERS.md) — a
function-id `i32` + per-site dispatch (`CallIndirect` expanded to eq/branch/
named-`Call`/`Trap` at lowering) is feasible; recommended for a later milestone.

### 4. `atomicrmw` (parser) — `atomic`

`unsupported instruction: atomicrmw`. Rare in scalar `-O0` code but real in
multithreaded libraries; out of scope for the word ISA.

### 5. Vector types (parser) — `vector`

`unsupported type: Ident("<")`. Explicit SIMD vector types are niche for this
toolchain's targets.

### 6. Scratch number model (Scratch target) — 3 fixtures

Every one is **exact on the interpreter and the VM**; only the Scratch backend
blocks it. The remaining blockers are the **bitwise/shift family**, which has no
Scratch operator at all (`docs/design/SCRATCH_NUMERIC_MODEL.md` §3.6):

```
ashr / lshr    bitwise, i64muldiv
and            bytes
```

`i64muldiv` needs the i64 `mul`/`udiv`/`urem` software helpers, which are built
from `shl`/`lshr`/`ashr` — so the shift gap cascades to it. The v0.2 width-cast
blockers (`sext i8/i16`, `zext i8/i16`, `trunc i64→i32`, `ptrtoint`) are gone:
the byte-exact memory model lowered every one of them exactly, moving `globals`,
`signedcmp`, `i64arith`, `reinterp`, `struct-array`, `ptrstruct`, `byte-scan`,
`char-mix`, `i64-struct`, and `intrinsics` to full success.

The former **#6 gap — bodyless runtime intrinsics on the VM** — is closed. The
VM resolves `__scratcharch_*` builtins, the `llvm.*` bit intrinsics, and
runtime-length `llvm.mem*` calls at load time through the same registry the
interpreter consults; `string` and `intrinsics` are now full successes on all
four measured stages.

These are the Scratch-model limits recorded in spec §6.2, surfaced here as
measured blockers rather than opcode counts.

## Not a bottleneck (v0.1→v0.2 expanded these)

The following are exact end-to-end; listed so a stale copy of this list is not
mistaken for the current one:

- **Structs and aggregates (real clang `-O0`)**: struct fields at
  natural-alignment offsets with padding (`char-mix`), nested structs
  (`nested-struct` → 156), whole-struct assignment `y = x` lowered by clang to a
  constant-length `memcpy` (`struct-assign` → 56), struct arrays and
  pointer-to-struct member access through dynamic `i64` GEP indices
  (`struct-array` → 66, `ptrstruct` → 36 — exact on the VM), and an `i64` field
  inside a struct (`i64-struct` → 45). No `datalayout` parsing and no
  aggregate-by-value ABI were needed (spec §4).
- **Constant-length memory ops on the VM** (`llvm.memcpy`/`llvm.memmove`/
  `llvm.memset`, `__scratcharch_memcpy`): the translator expands them into
  width-exact `i8` load/store sequences (load-all-then-store, so overlapping
  `memmove` is well-defined), moving the `memory`/`memintrin`/`struct-assign`
  fixtures to full success.
- **Runtime-length memory ops, the SART string builtins, and the `llvm.*` bit
  intrinsics on the VM**: load-time resolved, never approximated (`runtime_mem`
  → 255, `string` → 5, `intrinsics` → 2018928754).
- Everything the v0.3 spec gate already closed: signed `icmp`; two-limb `i64`
  add/sub/compare/cast/load/store/select/phi and full-width
  `mul`/`udiv`/`urem` (software helpers `__sair_mul64`/`__sair_udivrem64`);
  bitwise/shift ops at every width; `unreachable`/`Trap`; byte-granular
  `Load8`/`Store8` memory and byte-exact static globals; cell-preserving
  reinterpretation; `phi`; `switch`.

## New since baseline (v0.1 → v0.2)

- **Corpus 25 → 33** committed real-clang fixtures (8 new aggregate/real-world
  programs under `tests/corpus/llvm/fixtures/`).
- **Full end-to-end success +4**: `memory` (6), `memintrin` (1),
  `nested-struct` (156), `struct-assign` (56). The first two flipped from
  `vm-failure` (interpreter-only `memcpy`) to success via the translator's
  constant-length memory-op expansion; the last two are new aggregate fixtures.
- **VM exact 17/25 → 26/33** (+9 percentage points): the memory-op fixtures plus
  the new struct/array/pointer/i64-in-struct fixtures all run exactly on the
  ISA VM; only the two bodyless-intrinsic fixtures remain interpreter-only.
- **Overall 84% → 85%** (semantic core 21/25 → 28/33); Parser/SAIR/Interpreter
  84% → 85%. Scratch 13/25 → 15/33 (its % falls only because the denominator
  grew from 25 to 33).

## New since baseline (v0.2 → v0.3)

- **Byte-exact Scratch memory model** (`SCRATCH_MEMORY.md`): the ScratchGraph
  heap became byte-addressable — one list item per byte, width-exact
  little-endian load/store, exact static-data seeding, mathematical-signed value
  convention reconciled with SAIR raw-bit semantics. Verified by formula-level
  and structural unit tests (`scratchgraph_tests.rs`), never by claiming
  execution.
- **Scratch 45% → 76%** (15 → 25 constructed projects): `globals`, `signedcmp`,
  `i64arith`, `reinterp`, `struct-array`, `ptrstruct`, `byte-scan`, `char-mix`,
  `i64-struct` are new full successes; `intrinsics` now constructs too (still a
  vm-failure at the runtime intrinsics). scratch-backend-failure 12 → 3; full
  end-to-end success 14 → 23.
- **Parser/SAIR/Interpreter/VM unchanged** (85/85/85/79): the byte-exact work
  touched only the Scratch backend, so the semantic core stays 28/33.
- **Honest manifest pins**: `string` and `global-agg` carried aspirational
  expectations (vm success / scratch rejection) that never matched the measured
  behavior. Their manifest entries now record their true boundaries
  (`string` → vm-failure at `__scratcharch_strlen`; `global-agg` → parse-failure
  at the aggregate global initializer) — no fixture was made "Supported" by
  weakening a check.

## New since baseline (v0.3 → v0.4)

- **VM runtime resolver**: the ISA VM no longer has an `Interpreter only` class.
  At `load_program` every bodyless named `Call` is resolved once, per name, per
  program — first against the shared `scratcharch-runtime` `IntrinsicRegistry`
  (whose `IntrinsicSignature` supplies the operand-stack arity), then against
  the canonical `llvm.mem*` variants and the `llvm.bswap/ctpop/ctlz/cttz.iN`
  bit intrinsics — and rewritten to an internal `Code::CallRuntime` entry. The
  execute loop dispatches on the resolved kind, never a name string; the ISA is
  unchanged and gains no libc-specific instruction.
- **VM 79% → 85%** (26/33 → 29/34); **no `vm-failure` class remains**. The two
  former interpreter-only fixtures (`string`, `intrinsics`) are full successes.
- **Failure categories stay distinct on the VM**: `Abort`, `Panic`, `Trap`
  (runtime), and `DivisionByZero` are separate from the ISA `unreachable` trap —
  a runtime abort is never reported as an `unreachable`.
- **Corpus 33 → 34**: `runtime_mem`, a program whose memory-op lengths are
  computed at run time (so the translator leaves them as calls) and which
  exercises every SART string builtin — interpreter-exact, VM-exact, and
  native-exact (255).
- **Interpreter and VM now share the bit-intrinsic leaf**
  (`scratcharch_runtime::bit_intrinsic_value`), so the two engines cannot
  diverge on `bswap`/`ctpop`/`ctlz`/`cttz` including the no-poison
  zero-operand case.
- **Differential suite 31 → 40 tests** (`vm_differential_tests.rs`): bit
  intrinsics across families and widths, runtime-length `llvm.mem*`, the SART
  string/memory builtins, the distinct failure categories, and an unknown
  bodyless callee rejected by both engines (VM at load time, interpreter when
  the call is reached).
- **Parser/SAIR/Interpreter unchanged** at 85%; Scratch stays 76% but its
  numerator grows 25 → 26 (the new fixture constructs).

## How this is verified

- **Committed real-clang corpus** `tests/corpus/llvm/fixtures/` (34 fixtures)
  plus the classic `tests/c_programs/*.{c,ll}`, each with a fresh-clang
  recompile harness so the committed `.ll` cannot drift.
- **Five-surface record** `scratcharch-pipeline/tests/llvm_corpus_surfaces.rs`
  pins, per fixture, the parser / SAIR-validation / interpreter / VM /
  Scratch-lower verdict. VM verdicts are *exact value* or *named diagnostic* —
  nothing in between.
- **Native-reference differential**: each `.c` is recompiled and run through a
  real C compiler; native exit, interpreter result, and VM result must all agree
  where the VM is supported, and the VM must reject with the pinned diagnostic
  where it is not (exit codes compared `& 0xFF`). Runtime-helper calls are
  provided natively by a libc-backed shim (`scratcharch-compat/src/native.rs`).
- **Compatibility benchmark** (`scratcharch test-compat`) enforces the recorded
  manifest expectations as a regression gate and renders the dashboard above
  with explicit `(pass/total)` denominators. Machine results live in
  `tests/corpus/llvm/results/`.
- **Memory-intrinsic differential suite** (`memintrin_tests.rs`, 5 tests):
  constant-length `memcpy`/`memmove`(overlap)/`memset` must agree bit-for-bit on
  the interpreter *and* the VM; `llvm.memcpy.inline` stays a rejected variant;
  runtime-length `memcpy` is resolved on both engines now that the VM has its
  load-time runtime resolver.
- **Frontend-focused suites**: `translator_tests.rs`, `phi_icmp_tests.rs`,
  `reject_tests.rs`, plus the interpreter/VM differential suite
  (`vm_differential_tests.rs`, 40 tests) and the driver's `vm_backend_tests.rs`
  for two-limb `i64` and byte memory.

Gate commands: `cargo test --workspace`, `cargo clippy --workspace
--all-targets` (zero warnings), `./scripts/run_c_tests.sh`.
