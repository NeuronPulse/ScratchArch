# LLVM Compatibility

> Specification version: **v0.3**
> Status: normative (defines what `scratcharch-llvm` accepts and how it behaves)
> Companion documents: [`LLVM_TRANSLATION.md`](../design/LLVM_TRANSLATION.md)
> (design/mapping), [`LLVM_COMPATIBILITY_STATUS.md`](../design/LLVM_COMPATIBILITY_STATUS.md)
> (generated status report), [`SAIR_FORMAT.md`](./SAIR_FORMAT.md),
> [`ISA.md`](./ISA.md), [`ABI.md`](./ABI.md), [`MEMORY.md`](./MEMORY.md).

---

## 1. Purpose and scope

ScratchArch accepts **LLVM textual IR** (`.ll`) as a frontend language. Real
compilers (clang, rustc) already lower source programs to LLVM IR, so a correct
LLVM→SAIR translation lets ScratchArch run C without language-specific parsers.

The goal of the current milestone is **not** the widest possible opcode
surface: it is to get
*real* clang/Clang output as far through the toolchain as possible while keeping
LLVM semantics **exact**, and — where a construct cannot be handled — to
**reject it with an explicit diagnostic** rather than translate it incorrectly
or drop it silently. Correctness is ranked above coverage.

This document is the authoritative compatibility matrix. It states, for each
LLVM construct, the strongest guarantee that holds. A status is never higher
than what every named execution surface actually provides: **`SUPPORTED` is
only used when the SAIR interpreter *and* the SA48 VM backend both execute the
construct exactly.** If only the parser/interpreter handles a construct, the row
is `INTERPRETER_ONLY`, `PARTIAL`, or `VM_UNSUPPORTED` — never `SUPPORTED`.

## 2. Execution surfaces and status meanings

An LLVM construct crosses three surfaces before it runs:

1. **Frontend** — `parser.rs` + `translator.rs` produce validated SAIR.
2. **SAIR interpreter** — `scratcharch-sair-interpreter` is the semantic
   reference; every translated construct is exact here.
3. **SA48 VM backend** — `scratcharch-ir::lower` + `scratcharch-vm`; the ISA is
   frozen, 32-bit-word based, and intentionally smaller than SAIR.

A separate axis — **Scratch backend** (`scratcharch-scratchgraph::lower` to a
ScratchGraph project) — is described in §6; the Scratch model cannot express
some things that SAIR and the VM handle fine.

| Status | Meaning |
|--------|---------|
| **Supported** | LLVM→SAIR is exact **and** the SAIR executes exactly on the interpreter **and** the VM backend. No caveat. |
| **Partial** | Exact on the interpreter; the VM backend lowers a *documented subset* and rejects the rest with an explicit diagnostic (or the construct is exact only for a documented input sub-range). |
| **Interpreter only** | Exact on the interpreter; the VM backend has no lowering for it yet (every use is an explicit diagnostic). A faithful VM path is compatible with the current ISA and tracked. |
| **VM unsupported** | Exact on the interpreter; the VM backend deliberately rejects it because faithful execution would need VM/ISA work beyond current scope. The explicit diagnostic is the intended terminal behavior — never a silent approximation. |
| **Scratch backend unsupported** | Exact through SAIR (and the VM where noted), but not expressible as a ScratchGraph project (§6). |
| **Unsupported** | Rejected at parse/translate time with an explicit, actionable diagnostic; the LLVM frontend does not accept the construct. |

The categories are disjoint by design. In particular, **`VM unsupported` is not
an alias for `Interpreter only`**: `Interpreter only` names work that is
tracked as a compatible future VM path, while `VM unsupported` names work the
VM will keep rejecting because faithful execution needs capability beyond the
frozen word ISA. (Software helpers are the bridge between the two: full-width
`i64` arithmetic and every shift are *Supported* — not merely tracked — because
`__sair_mul64`/`__sair_udivrem64`/`__sair_*64` realise them exactly from the
word ops, §5.1 and EXECUTION_MODEL.md §5.7–§5.8.) Both refuse to run, and both
refuse to fake it.

## 3. Pipeline and frontend

```
C source --clang -S -emit-llvm--> .ll text
    scratcharch_llvm::translate_llvm(text) -> IrModule (validated SAIR)
    scratcharch_sair_interpreter::Interpreter::run() -> RuntimeValue
    scratcharch_ir::lower (IsaLowerer) -> scratcharch_vm::Vm      # VM path
```

- Parser: hand-written lexer + recursive-descent parser (`parser.rs`). No LLVM
  C++ dependency.
- Translator: walks the AST and builds SAIR with `IrBuilder` (`translator.rs`).
  Every produced module passes `IrModule::validate()` before execution.
- `declare`d functions are parsed; the interpreter resolves them at call time
  via the runtime-intrinsic registry or the bit-intrinsic handler. The VM
  backend only runs functions that have a `define`d body — see §6.
- Entry point: the `define`d function named `main` (fallback: last
  non-declaration).
- Endianness: little-endian; an address is an integer of pointer width
  (SA48: 32-bit pointer). Integer literals are decimal (`i64` and negative
  constants included); hexadecimal (`0x…`) literals are not yet lexed.
- Argument attributes (`noundef`, `signext`, `align N`, …) are skipped.
  `nsw`/`nuw`/`exact` flags are parsed and ignored: SAIR is wrapping and
  poison-free, so the flags never bind.

## 4. Type matrix

| LLVM IR | SAIR `IrType` | Status | Notes |
|---------|---------------|--------|-------|
| `i1` | `I1` | Supported | Boolean; distinct from numeric `0`/`1` |
| `i8` | `I8` | Supported | Byte |
| `i16` | `I16` | Supported | |
| `i32` | `I32` | Supported | Primary integer carrier |
| `i64` | `I64` | Supported | Interpreter exact at any width; VM carries it as two 32-bit limbs for the whole arithmetic family — `add`/`sub`/compare/cast/load/store/select/phi per limb, and full-width `mul`/`udiv`/`urem` (hence the translator's signed `sdiv`/`srem`) via software helpers `__sair_mul64`/`__sair_udivrem64` built from the word ops (§5.1) |
| `ptr` | `Pointer` | Supported | Generic byte-addressed pointer; 32-bit on SA48 |
| `void` | `Void` | Supported | Return type / declaration only |
| `[N x T]` | element type | Supported | As allocation/layout for `alloca`/`getelementptr` only |
| `{ … }` / anonymous struct | element type | Supported | As layout for `alloca`/`getelementptr` only |
| aggregate *values* | — | Unsupported | Loads/phis/params returning a struct/array *by value* are not modeled |
| `half`/`float`/`double`/`f128` | — | Unsupported | Explicit diagnostic at parse (SAIR has `F64` but the frontend does not expose floats) |
| `i128`, `x86_mmx`, vectors | — | Unsupported | Explicit diagnostic at parse |

## 5. Instruction matrix

Legend for the per-row status: **S** = Supported, **P** = Partial,
**IO** = Interpreter only, **VU** = VM unsupported, **U** = Unsupported.
Every row is exact on the SAIR interpreter unless the notes say otherwise.

### 5.1 Arithmetic and bitwise

| LLVM | SAIR | Status | Notes |
|------|------|--------|-------|
| `add` / `sub` | `Add` / `Sub` | **S** | Wrapping; `nsw`/`nuw` ignored. `i64` = two limbs on the VM (carry/borrow across the limb boundary) |
| `mul` | `Mul` | **S** | Wrapping; `nsw`/`nuw` ignored. `i32`/narrower is a word `I32Mul`; `i64` = two limbs via the software helper `__sair_mul64` — a 16-bit schoolbook expansion of `(a0+a1·2³²)(b0+b1·2³²) mod 2⁶⁴` in which every partial product is an exact 16×16→32 word multiply, so no widening ISA op is needed (§5.8 of EXECUTION_MODEL.md) |
| `udiv` / `urem` | `Div` / `Rem` | **S** | Unsigned floor, one-to-one at any width; `i64` = two limbs via the software helper `__sair_udivrem64` (64-step restoring division, §5.8). Divide-by-zero runs the same error path as the word `I32Div`: the helper manufactures a `0/0` word division, so both engines report `DivisionByZero` |
| `sdiv` / `srem` | expansion | **S** | Exact trunc-toward-zero magnitude expansion over unsigned ops (`translate_signed_divrem`); `i64` inherits the unsigned helper's quotient/remainder; `INT_MIN/-1` wraps (x86-consistent) |
| `and` / `or` / `xor` | `And` / `Or` / `Xor` | **S** | Width-preserving, wrapping; `i8/i16` masked carriers, `i32` word, `i64` per-limb word ops on the VM (EXECUTION_MODEL.md §5.7) |
| `shl` / `lshr` / `ashr` | `Shl` / `Lshr` / `Ashr` | **S** | Effective amount = `amount mod w` (deterministic poison region, §5.7); `ashr` sign-replicates bit `w-1`. VM realises every shift via software helpers `__sair_shl64`/`__sair_lshr64`/`__sair_ashr64` (single-limb and two-limb) |

### 5.2 Comparison (`icmp`)

All comparisons operate on the raw operand **bit pattern** at any supported
width (`i1`–`i64`), on the interpreter and on the VM's two-limb path alike.

| LLVM | SAIR | Status | Notes |
|------|------|--------|-------|
| `icmp eq` | `Eq` | **S** | Two-limb `eq` compares each limb |
| `icmp ne` | `not Eq` | **S** | |
| `icmp ult` | `Lt` | **S** | Unsigned; limb-wise on the VM |
| `icmp ugt` | `Gt` | **S** | |
| `icmp ule` | `not Gt` | **S** | |
| `icmp uge` | `not Lt` | **S** | |
| `icmp slt` | `emit_signed_lt` | **S** | Exact for negatives and mixed sign bits at `i8/i16/i32/i64` — sign-bit select over the unsigned compare, no approximation |
| `icmp sgt` | `emit_signed_lt(b,a)` | **S** | |
| `icmp sle` | `not sgt` | **S** | |
| `icmp sge` | `not slt` | **S** | |

### 5.3 Conversions

| LLVM | SAIR `CastOp` | Status | Notes |
|------|---------------|--------|-------|
| `zext` | `Zext` | **S** | Any width → wider, incl. `i32→i64` and `i64`→nothing |
| `sext` | `Sext` | **S** | Sign-extend; VM fills the high limb / sign-extends the low cell exactly |
| `trunc` | `Trunc` | **S** | Wider → narrower, low bits kept; VM reads the low limb for `i64→…` |
| `bitcast ptr→ptr` | — | **S** | No-op passthrough |
| `bitcast` int↔int | `Bitcast` | **IO** | Same-width reinterpretation; interpreter exact, VM rejects (`"no ISA instruction reinterprets bits"`) — a no-op path is compatible and tracked |
| `ptrtoint` | `PtrToInt` | **IO** | Address → integer of pointer width (interpreter); VM rejects pending the same no-op path |
| `inttoptr` | `IntToPtr` | **IO** | Integer → address (interpreter); VM rejects pending the same no-op path |
| float conversions (`fptosi`, …) | — | **U** | Floats unsupported |

### 5.4 Memory

| LLVM | SAIR | Status | Notes |
|------|------|--------|-------|
| `alloca T` / `alloca T, i32 N` | `Alloca` (byte array) | **S** | Sized from the target layout; constant count |
| `load` / `store` | `Load` / `Store` | **S** | Typed; `i64` = two limbs at `addr`/`addr+4` on the VM |
| `getelementptr` | single byte-offset `Gep` over `i8` | **S** | Constant indices fold to a byte offset (works everywhere); a *dynamic* index on a byte array carries its low limb into the 32-bit address space; a dynamic index on a multi-byte-element array multiplies the index by the element size with the same software `mul` the VM uses for full-width `i64` multiply (§5.1). Interpreter and VM are exact for all three |
| `volatile` / `atomic` | — | **U** | Rejected (alignment and other attributes are tolerated) |
| global data (`@g = global T init`) | static data segment; uses of `@g` are its absolute address (I32) | **P** | Flat segment laid out below the stack floor (`STATIC_DATA_BASE`); the region below `stack_limit` is never stack-allocated, so it is safe static data. Interpreter seeding is **byte-exact**. Supported initializers: scalar integers (`i1/i8/i16/i32/i64`), `zeroinitializer`, `null`, pointer relocations (`ptr @other`), `c"…"` byte strings, and flat arrays of scalars. Rejected with explicit diagnostics: struct/void globals, `undef`/`poison`, nested-aggregate (array-of-array) initializers, and relocations to undeclared globals. **Address-of-function is rejected** (no function-pointer ABI — see the indirect-`call` row in §5.5). VM split: word-granular data is VM-exact; sub-word/byte data is interpreter-only (§6.1) |

### 5.5 Calls, intrinsics, and control flow

| LLVM | SAIR | Status | Notes |
|------|------|--------|-------|
| `call @f` (direct, recursion) | `Call` | **S** | Independent frames. VM path pinned for the ≤32-bit ABI; `i64`-wide arguments/returns travel the same cell-slot ABI |
| `call @__scratcharch_*` | `Call` | **IO** | Runtime-intrinsic registry (`memcpy`/`memmove`/`memset`, …) lives in the interpreter |
| `call @llvm.memcpy/memmove/memset.*` | `Call` | **IO** | clang's canonical forms; resolved to the runtime semantics in the interpreter; unknown variants get an explicit diagnostic |
| `call @llvm.bswap/ctpop/ctlz/cttz` | `Call` | **IO** | Interpreter reference expansion over the declared width (`i8…i64`); see `LLVM_TRANSLATION.md` |
| other `llvm.*` | — | **U** | Explicit `unsupported llvm intrinsic` diagnostic |
| indirect `call` / function pointers | — | **U** | SAIR/ISA `Call`s name a static callee; there is no function-pointer ABI. Rejected with an explicit diagnostic |
| `ret` | `Return` | **S** | Value or `void` |
| `br label` | `Branch` | **S** | |
| `br i1 %c, l1, l2` | `CondBranch` | **S** | |
| `phi` | `Phi` | **S** | Frontend keeps `phi` as SAIR `Phi` with edge-selected semantics; the VM lowers it to edge copies (no frontend flattening) |
| `select i1 %c, a, b` | `Select` | **S** | Cell-count generic, incl. `i64` values on the VM |
| `switch` | chain of `eq` + `CondBranch` | **S** | Linear chain against a shared default |
| `unreachable` | `Unreachable` | **S** | Both engines halt in a well-defined terminal trap state: interpreter `InterpError::Trap`, VM `VmError::Trap` at the trap location (EXECUTION_MODEL.md §5.6) |

Control-flow translation is expressed purely in SAIR terms — it never depends on
Scratch-specific behavior.

## 6. Backend axes

### 6.1 VM backend

Translation to SAIR is the compatibility boundary. A separate concern is whether
the frozen ISA VM backend can lower the resulting SAIR:

| Construct | Interpreter | VM backend | Category |
|-----------|-------------|------------|----------|
| `i32`-representable programs | Supported | Supported | Supported |
| `i64` add/sub/compare/cast/load/store/select/phi | Supported | Supported (two 32-bit limbs) | Supported |
| `i64` mul/div/rem | Supported | Supported — software helpers `__sair_mul64`/`__sair_udivrem64` over the word ops (§5.8) | Supported |
| dynamic `getelementptr` into a byte array | Supported | Supported (low limb) | Supported |
| dynamic `getelementptr` into `i32`/wider elements | Supported | Supported — scaled byte offset via the software `mul` helper | Supported |
| `select` / `switch` | Supported | Supported | Supported |
| `phi` | Supported | Supported (edge copies) | Supported |
| `and`/`or`/`xor` (all integer widths) | Supported | Supported (word op; `i64` per-limb) | Supported |
| `shl`/`lshr`/`ashr` (all integer widths) | Supported | Supported (software helpers `__sair_shl64`/`__sair_lshr64`/`__sair_ashr64`) | Supported |
| `unreachable` | trap | Supported — `Trap` terminal primitive (§5.6) | Supported |
| `bitcast`/`ptrtoint`/`inttoptr` | Supported | Rejected — no reinterpret ISA | Interpreter only |
| `llvm.*` bit intrinsics, `llvm.memcpy`/`memset`, runtime intrinsics | Supported | No `define`d body to run | Interpreter only |
| global data, word-granular (aligned `i32`/`i64`/`ptr` leaves) | Supported | Supported (VM-exact static segment) | Supported |
| global data, sub-word/byte leaves (`i1`/`i8`/`i16`, byte strings, arrays with byte elements) | Supported | Rejected — `sub-word or byte` diagnostic | Interpreter only |

No `VM unsupported` row remains: every construct the VM can express — exactly,
or via a program-level software helper (§5.8) — is marked Supported, and what
it cannot (reinterpret no-ops, byte-granular globals, bodyless calls) is on the
`Interpreter only` side with a compatible, tracked path.

The VM gaps above are **backend** limitations, not translator ones. Nothing is
approximated: constructs the VM cannot execute faithfully are rejected with a
diagnostic that names the ISA need. Static data lives below `stack_limit` on
*both* backends — a segment that does not fit below the stack floor is rejected
(`StaticDataTooLarge` / "does not fit below the stack floor"), never allowed to
collide with the downward-growing stack.

**v0.3 ISA extension.** The frozen word ISA gains three **additive** stack-machine
primitives — `Load8`, `Store8`, `Trap` — defined normatively in
[`ISA.md`](./ISA.md) Appendix A and [`EXECUTION_MODEL.md`](./EXECUTION_MODEL.md)
§5.6. They are target-independent (no Scratch/LLVM/libc semantics). Of the rows
these were expected to move from `Rejected` toward the exact-VM column, the
**trap path has now landed**: `unreachable` and the interpreter/VM `Trap` rows
above are re-stated and exercised by `diff_unreachable_traps_both_engines`. The
**`sub-word/byte globals`** and the **reinterpret no-op path**
(`bitcast`/`ptrtoint`/`inttoptr`) rows remain on the `Interpreter only` side and
are tracked as gaps (§8). Word `Load`/`Store` are unchanged — the matrix never
claims a VM capability before the VM exercises it.

**Full-width arithmetic on the VM.** Closing `i64 mul`/`div`/`rem` required no
further ISA change. The two-limb forms lower to calls on program-level software
helpers appended to the ISA program — `__sair_mul64` (16-bit schoolbook
multiply) and `__sair_udivrem64` (64-step restoring division computing quotient
and remainder together) — built entirely from the existing word ops
(EXECUTION_MODEL.md §5.8). Because the *translator* already expands LLVM's
signed `sdiv`/`srem` into magnitudes over the unsigned `Div`/`Rem`, all five
i64 arithmetic forms reach the VM exactly, and dynamic scaled
`getelementptr` (an `i64`/`i32` `mul` in the byte-offset computation) follows
for free. Divide-by-zero is manufactured as the word `0/0` error inside the
helper, so the VM and interpreter raise the same `DivisionByZero` on the same
module.

### 6.2 Scratch backend

`scratcharch-scratchgraph::lower` can map SAIR into a ScratchGraph project, but
the Scratch execution model is fundamentally different: per-target variables and
lists, sprites/scripts, broadcast events, and no flat byte-addressed memory, no
`malloc`/pointers, and no arbitrary call stack with independent frames. The
LLVM-to-Scratch path is therefore out of the v0.2 LLVM scope. As a cross-cutting
axis, every construct that depends on the flat linear memory model, on
dereferenced pointers, or on recursive/stack frames is
**Scratch backend unsupported** — even where the interpreter and the VM execute
it exactly. This is a *model* limitation of Scratch, documented here so the
matrix is not misread as implying Scratch exportability.

## 7. Explicitly rejected — summary

Anything not listed above is rejected with an explicit, actionable diagnostic
(`UnsupportedInstruction` / `UnsupportedType`), including: float ops and float
conversions, aggregate values, indirect calls, the unsupported global shapes
named in §5.4 (`undef`/`poison`, struct/void globals, nested aggregates,
relocations to undeclared globals), unknown `llvm.*` intrinsics,
`i128`/vectors, and `volatile`/`atomic`. ScratchArch never silently drops an
instruction, never lowers a test standard to force a PASS, and never changes
frozen ISA semantics merely to satisfy a frontend case.

## 8. Known gaps

1. **No reinterpret casts on the VM** (`bitcast`, `ptrtoint`, `inttoptr`):
   the interpreter is exact; a VM no-op path is compatible and tracked.
2. **Call-runtime constructs are interpreter-only** (`llvm.memcpy` family,
   bit intrinsics): no `define`d body exists for the VM to call.
3. **Globals with sub-word/byte data are interpreter-only**; the VM's word ops
   cannot read them. Word-granular globals run on both backends (§6.1).
4. **Hex literals are not lexed** (`0x…`); integer constants are decimal.
5. **Poison is not modeled.** Per SAIR's no-poison policy, overflow wraps and
   zero-operand `ctlz`/`cttz` return the width even when LLVM would permit
   poison. This is a deliberate, documented divergence.
6. **`switch` is a linear chain**, not a jump table or binary search.

## 9. Verification

Three layers prove the matrix (counts updated at the v0.3 gate, §LLVM_TRANSLATION
and the generated status report):

1. **Committed real-clang fixtures** (`tests/c_programs/*.{c,ll}`): clang `-O0`
   output, committed, run through `translate_llvm` → interpreter. The corpus
   covers signed comparisons on negatives/mixed signs (`signedcmp`), `i64`
   arithmetic across the limb boundary (`i64arith`), the full bitwise/shift
   family with sign-fill and cross-limb shift amounts (`bitwise`, native exit
   `293345`), full-width `i64` multiply/divide/remainder over real clang
   `mul`/`udiv`/`urem`/`sdiv`/`srem` (`i64muldiv`, native checksum `3579139508`
   — high-limb weighted so a dropped limb or carry perturbs it), structs,
   arrays, globals, `llvm.*`/runtime intrinsics, memory intrinsics, and
   function calls. `phi` loops do not appear in clang `-O0` output (clang keeps
   induction variables in memory at `-O0`); loop-carried `phi` is exercised by
   the hand-written VM corpus `tests/c_programs_vm/phi_sum.ll`/`neg_countdown.ll`
   and the focused translator tests, which is recorded honestly rather than
   forcing an `-O1` output into an `-O0` corpus.
2. **Fresh-clang recompile harness** (`tests/corpus_clang_tests.rs`): recompiles
   each `.c` with clang on every run so the committed `.ll` cannot drift.
3. **Five-surface corpus record** (`scratcharch-pipeline/tests/llvm_corpus_surfaces.rs`):
   for every committed fixture pins the parser, SAIR-validation, interpreter,
   VM, and Scratch-lower verdicts, and re-runs each `.c` against a real C
   compiler as the native reference (three-way differential). VM verdicts are
   explicit: exact value where supported, named diagnostic where not.
4. **Focused unit tests** per part: `translator_tests.rs` (constants, i64
   arithmetic, conversions, signed comparisons incl. negatives and
   `i64::MIN/-1`), `phi_icmp_tests.rs` (phi + signed `icmp`), `memintrin_tests.rs`
   (memcpy/memmove/memset), `reject_tests.rs` (indirect calls), the driver's
   `vm_backend_tests.rs` (interpreter/VM agreement incl. the multi-cell `i64`
   corpus and profile-driven limb counts), and the interpreter's
   `vm_differential_tests.rs` (24 tests requiring bit-for-bit
   interpreter-vs-VM agreement: `and`/`or`/`xor`/`shl`/`lshr`/`ashr` across
   `i1`–`i64`, poison-region shift amounts, negative sign-fill, cross-limb `i64`
   shifts, full-width `i64` `mul`/`udiv`/`urem` — carry-across-limbs, exact
   long-division matches, boundary divisors, and an `lcg` random sweep over both
   — plus `unreachable` trapping in both engines).

`scripts/run_c_tests.sh` drives the C corpus as a gate.

## 10. Change log

- **v0.3 (2026-09-09)**: additive `Load8`/`Store8`/`Trap` word-ISA primitives
  (EXECUTION_MODEL.md §5.6) realise `unreachable` as a well-defined terminal
  trap on the VM; bitwise ops (`and`/`or`/`xor`) become Supported at every
  width (per-limb word ops for `i64`) and every shift is realised exactly on
  the VM by the program-level software helpers `__sair_shl64`/`__sair_lshr64`/
  `__sair_ashr64` (§5.7); full-width `i64` multiply/divide/remainder close with
  two further software helpers `__sair_mul64`/`__sair_udivrem64` (§5.8), which
  also carry dynamic scaled `getelementptr` to Supported. The `VM unsupported`
  column is now empty (that classification remains defined for future
  capability boundaries); the remaining gaps are the tracked `Interpreter only`
  paths (§8).
- **v0.2 (2026-09-09)**: six-level classification (`Supported` only where the
  interpreter **and** the VM backend are exact); exact signed `icmp` (Part 2);
  two-limb `i64` lowering on the VM (Part 3); `llvm.memcpy`/`memmove`/`memset`
  (Part 4); `phi` as first-class SAIR `Phi` (Part 5); explicit rejection of
  indirect calls (Part 7); real-clang corpus expansion with signed-comparison
  and `i64` fixtures plus a per-fixture five-surface record and native-reference
  differential (Parts 8–9).
- **v0.1 (2026-09-08)**: initial matrix for the v0.1 milestone.
