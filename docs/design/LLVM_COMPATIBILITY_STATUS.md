# LLVM Compatibility — Status Report

> Generated for the v0.3 milestone gate (2026-09-09).
> Companion to [`docs/specification/LLVM_COMPATIBILITY.md`](../specification/LLVM_COMPATIBILITY.md)
> (the normative matrix) and [`LLVM_TRANSLATION.md`](./LLVM_TRANSLATION.md) (the
> mapping design). This document records *where the toolchain currently stands*:
> what is exact, what is interpreter-only, and what is deliberately rejected —
> with the reason for each rejection. It is a snapshot, refreshed at milestone
> gates.

## Execution surfaces (recap)

An LLVM construct crosses three surfaces before it runs:

| # | Surface | Role |
|---|---------|------|
| 1 | Frontend (`parser.rs` + `translator.rs`) | LLVM text → validated SAIR `IrModule` |
| 2 | SAIR interpreter (`scratcharch-sair-interpreter`) | Semantic reference; every translated construct is exact here |
| 3 | SA48 VM backend (`scratcharch-ir::lower` + `scratcharch-vm`) | Frozen 32-bit-word ISA; full-width `i64` arithmetic realised by program-level software helpers appended during lowering (EXECUTION_MODEL.md §5.7–§5.8) |

A separate axis — the **Scratch backend** (`scratchgraph::lower`) — constructs
ScratchGraph projects but is a *construction-only* surface for LLVM input: the
Scratch execution model cannot express flat byte-addressed memory, pointers, or
arbitrary call frames, so LLVM→Scratch semantics are out of scope (§6.2 of the
spec).

A matrix row is **`Supported`** only when surfaces 1, 2, *and* 3 are all exact.
Anything the VM cannot execute faithfully is rejected with a diagnostic that
names the missing ISA/runtime capability — never approximated.

## Current bottlenecks (v0.3 snapshot)

Each entry names the construct, its exact surface coverage, and the reason the
remaining surfaces cannot yet run it. Ordering is roughly by how often a real
`-O0` C program hits it.

### 1. Floating-point is reserved

`float`/`double`/`half`/`f128` types and `fadd`/`fsub`/`fmul`/`fdiv`/`fcmp`
conversions are an explicit parse-time `UnsupportedType`. SAIR carries an `f64`,
but the frontend does not expose float types yet.

### 2. Indirect calls have no function-pointer ABI

SAIR/ISA `Call` names a static callee. A call through a function-pointer *value*
(common in real C: callbacks, vtables, function tables) is rejected with an
`UnsupportedInstruction` naming the missing ABI — there is no wrong-dispatch
path.

### 3. `llvm.*` and runtime intrinsics have no VM body

`llvm.memcpy`/`memmove`/`memset`, `llvm.bswap`/`ctpop`/`ctlz`/`cttz`, and the
`__scratcharch_*` runtime intrinsics are resolved by the SAIR interpreter
(memory-intrinsic family / pure reference expansions / runtime registry). They
are **interpreter-only**: there is no `define`d body for the VM to call, and the
VM rejects such modules with an explicit diagnostic rather than running a
bodyless call.

### 4. `switch` is a linear chain

`switch` lowers to a chain of `eq` + `CondBranch` against a shared default.
Correct at every supported width on both the interpreter and the VM, but a huge
case table is not specialised into a jump table or binary search.

### 5. Hex literals are not lexed

Integer constants are decimal; `0x…` literals are not yet accepted by the lexer.

### 6. Poison is not modeled (deliberate)

Per SAIR's no-poison policy, overflow wraps and zero-operand `ctlz`/`cttz`
return the width even where LLVM would permit poison. This is a documented,
deliberate divergence — SAIR has no poison value.

## Not a bottleneck (v0.3 closed these)

The following were open gaps at earlier gates and are now exact end-to-end; they
are listed so a stale copy of this list is not mistaken for the current one:

- **Signed `icmp`** (`slt`/`sgt`/`sle`/`sge`): exact for negatives and mixed
  signs via the sign-bit identity, on the interpreter *and* the VM's two-limb
  path.
- **`i64` on the VM**: two-limb add/sub/compare/cast/load/store/select/phi;
  limb count from `TargetProfile::cells_for_type`.
- **Bitwise and shift ops** (`and`/`or`/`xor`/`shl`/`lshr`/`ashr`): accepted by
  the frontend and exact on both engines at every width — `i64` bitwise runs
  per-limb word ops, and every shift is realised by the software helpers
  `__sair_shl64`/`__sair_lshr64`/`__sair_ashr64` (EXECUTION_MODEL.md §5.7).
- **`unreachable`** / **`Trap`**: the additive word-ISA primitives `Load8`/
  `Store8`/`Trap` give `unreachable` a well-defined terminal trap state on the
  VM that matches the interpreter (EXECUTION_MODEL.md §5.6).
- **Full-width `i64` multiply/divide/remainder**: `mul`/`udiv`/`urem` — and the
  translator's signed `sdiv`/`srem`, which reach the VM through the same
  unsigned helper via their magnitude expansion — run on both engines. The VM
  realises them with the software helpers `__sair_mul64` (16-bit schoolbook
  multiply `mod 2⁶⁴`) and `__sair_udivrem64` (64-step restoring division
  computing quotient and remainder together, §5.8); divide-by-zero is raised as
  the manufactured word `0/0` error, so both engines report `DivisionByZero`.
  Dynamic scaled `getelementptr` (an `i64`/`i32` `mul` in the byte-offset
  computation) runs for free.
- **`phi`**: first-class SAIR `Phi`, edge copies on the VM.
- **Byte-granular memory and sub-word/byte globals**: `Load8`/`Store8` give the
  VM width-exact `i1`/`i8`/`i16` loads and stores (little-endian,
  neighbour-preserving), and the static-data segment is seeded byte-exact, so
  sub-word/byte leaves (`i1`/`i8`/`i16`, byte strings, arrays with byte
  elements) run on the interpreter *and* the VM.
- **Reinterpret casts** (`bitcast`/`ptrtoint`/`inttoptr`): lowered as zero-cost
  cell-preserving copies on the VM — `ptrtoint i64` zero-extends into the limb
  pair, `inttoptr i64` traps on a nonzero high limb (same value the interpreter
  rejects), `bitcast` only as a same-size same-kind no-op. Both engines exact.
- **Global data**: module static-data segment; word- and byte-granular leaves
  VM-exact (byte-exact seeding).
- **`llvm.memcpy`/`memmove`/`memset`**: handled by the interpreter's memory
  intrinsic family.
- **Indirect calls**: *rejected explicitly* (named diagnostic) — never a wrong
  dispatch. This is the intended terminal behavior.

## How this is verified

- **Committed real-clang corpus** `tests/c_programs/*.{c,ll}` (clang `-O0`,
  committed so no toolchain is needed) plus a fresh-clang recompile harness.
- **Five-surface record** `scratcharch-pipeline/tests/llvm_corpus_surfaces.rs`
  pins, per fixture, the parser / SAIR-validation / interpreter / VM /
  Scratch-lower verdict. VM verdicts are *exact value* or *named diagnostic* —
  nothing in between.
- **Native-reference differential**: each `.c` is recompiled and run through a
  real C compiler; native exit, interpreter result, and VM result must all agree
  where the VM is supported, and the VM must reject with the pinned diagnostic
  where it is not (exit codes compared `& 0xFF`).
- **VM-backend suite** (`scratcharch-driver/tests/vm_backend_tests.rs`) forces
  interpreter/VM agreement, including the multi-cell `i64` corpus (two-limb
  arithmetic and the software-helper multiply/divide/remainder),
  profile-driven limb counts, byte-granular globals, `i1`/`i8`/`i16`
  loads/stores (LE, neighbour-preserving, unaligned), and reinterpret
  round-trips with `inttoptr` overflow trapping on both engines.
- **Differential suite** (`scratcharch-sair-interpreter/tests/
  vm_differential_tests.rs`, 31 tests) requires bit-for-bit interpreter-vs-VM
  agreement for the bitwise/shift family, full-width `i64`
  `mul`/`udiv`/`urem` (long-division matches, boundary divisors, an `lcg` random
  sweep), the `unreachable` trap in both engines, `i1`/`i8`/`i16` memory
  round-trips, and cell-preserving `ptrtoint`/`inttoptr`/`bitcast`
  reinterpretation including the `inttoptr i64` overflow trap.
- **Frontend-focused suites**: `translator_tests.rs`, `phi_icmp_tests.rs`,
  `memintrin_tests.rs`, `reject_tests.rs`, plus hand-written loop/`phi` fixtures
  for the VM corpus (`clang -O0` never emits `phi`; loop-carried `phi` is covered
  by hand-written fixtures, recorded honestly).

Gate commands: `cargo test --workspace`, `cargo clippy --workspace --all-targets`
(zero warnings), `./scripts/run_c_tests.sh`.
