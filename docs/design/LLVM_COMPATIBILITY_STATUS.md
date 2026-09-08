# LLVM Compatibility — Status Report

> Generated for the v0.2 milestone gate (2026-09-09).
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
| 3 | SA48 VM backend (`scratcharch-ir::lower` + `scratcharch-vm`) | Frozen 32-bit-word ISA; intentionally smaller than SAIR |

A separate axis — the **Scratch backend** (`scratchgraph::lower`) — constructs
ScratchGraph projects but is a *construction-only* surface for LLVM input: the
Scratch execution model cannot express flat byte-addressed memory, pointers, or
arbitrary call frames, so LLVM→Scratch semantics are out of the v0.2 scope
(§6.2 of the spec).

A matrix row is **`Supported`** only when surfaces 1, 2, *and* 3 are all exact.
Anything the VM cannot execute faithfully is rejected with a diagnostic that
names the missing ISA/runtime capability — never approximated.

## Current bottlenecks (v0.2 snapshot)

Each entry names the construct, its exact surface coverage, and the reason the
remaining surfaces cannot yet run it. Ordering is roughly by how often a real
`-O0` C program hits it.

### 1. Bitwise ops and shifts are rejected at parse time

`and`, `or`, `xor`, `shl`, `lshr`, `ashr` produce an explicit
`UnsupportedInstruction`. SAIR and the VM both have the ops; the **LLVM
frontend** has not wired them yet. Real C code that survives `-O0` as bitwise
logic (masking, flags, packing, `<<`/`>>`) is currently refused rather than
mis-translated.

### 2. Floating-point is reserved

`float`/`double`/`half`/`f128` types and `fadd`/`fsub`/`fmul`/`fdiv`/`fcmp`
conversions are an explicit parse-time `UnsupportedType`. SAIR carries an `f64`,
but the frontend does not expose float types yet.

### 3. Indirect calls have no function-pointer ABI

SAIR/ISA `Call` names a static callee. A call through a function-pointer *value*
(common in real C: callbacks, vtables, function tables) is rejected with an
`UnsupportedInstruction` naming the missing ABI — there is no wrong-dispatch
path.

### 4. VM `i64` mul/div/rem

`i64` add/sub/compare/cast/load/store/select/phi run as **two 32-bit limbs** on
the VM. Multiply and divide (plus remainder) would need a 64-bit widening
operation the frozen ISA does not have, so `i64 mul`/`udiv`/`urem`/`sdiv`/`srem`
are interpreter-exact and **VM-rejected** with a diagnostic naming that need.
The interpreter is exact at every width.

### 5. Byte-granular (sub-word) globals are interpreter-only

Word-granular global data (aligned `i32`/`i64`/`ptr` leaves) is **VM-exact** via
the module static-data segment. Sub-word/byte leaves (`i1`/`i8`/`i16`, byte
strings, arrays with byte elements) run exactly on the interpreter; the VM's
word ops cannot read them and the module is rejected with a `sub-word or byte`
diagnostic.

### 6. `llvm.*` and runtime intrinsics have no VM body

`llvm.memcpy`/`memmove`/`memset`, `llvm.bswap`/`ctpop`/`ctlz`/`cttz`, and the
`__scratcharch_*` runtime intrinsics are resolved by the SAIR interpreter
(memory-intrinsic family / pure reference expansions / runtime registry). They
are **interpreter-only**: there is no `define`d body for the VM to call, and the
VM rejects such modules with an explicit diagnostic rather than running a
bodyless call.

### 7. `switch` is a linear chain

`switch` lowers to a chain of `eq` + `CondBranch` against a shared default.
Correct at every supported width on both the interpreter and the VM, but a huge
case table is not specialised into a jump table or binary search.

### 8. Reinterpret casts and `unreachable` are interpreter-only

`bitcast` (int↔int), `ptrtoint`, and `inttoptr` are exact on the interpreter; the
VM has no reinterpret/address-no-op path yet (compatible and tracked). Same for
`unreachable`, which traps on the interpreter but has no trap ISA on the VM.

### 9. Hex literals are not lexed

Integer constants are decimal; `0x…` literals are not yet accepted by the lexer.

### 10. Poison is not modeled (deliberate)

Per SAIR's no-poison policy, overflow wraps and zero-operand `ctlz`/`cttz`
return the width even where LLVM would permit poison. This is a documented,
deliberate divergence — SAIR has no poison value.

## Not a bottleneck (v0.2 closed these)

The following were open gaps at v0.1 and are now exact end-to-end; they are
listed so a stale copy of this list is not mistaken for the current one:

- **Signed `icmp`** (`slt`/`sgt`/`sle`/`sge`): exact for negatives and mixed
  signs via the sign-bit identity, on the interpreter *and* the VM's two-limb
  path.
- **`i64` on the VM**: two-limb add/sub/compare/cast/load/store/select/phi;
  limb count from `TargetProfile::cells_for_type`.
- **`phi`**: first-class SAIR `Phi`, edge copies on the VM.
- **Global data**: module static-data segment; word-granular leaves VM-exact.
- **`llvm.memcpy`/`memmove`/`memset`**: handled by the interpreter's memory
  intrinsic family.
- **Indirect calls**: *rejected explicitly* (named diagnostic) — never a wrong
  dispatch. This is the intended terminal behavior for v0.2.

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
  interpreter/VM agreement, including the multi-cell `i64` corpus and
  profile-driven limb counts.
- **Frontend-focused suites**: `translator_tests.rs`, `phi_icmp_tests.rs`,
  `memintrin_tests.rs`, `reject_tests.rs`, plus hand-written loop/`phi` fixtures
  for the VM corpus (`clang -O0` never emits `phi`; loop-carried `phi` is covered
  by hand-written fixtures, recorded honestly).

Gate commands: `cargo test --workspace`, `cargo clippy --workspace --all-targets`
(zero warnings), `./scripts/run_c_tests.sh`.
