# LLVM IR → SAIR Translation

> Document version: 0.2
> Status: prototype

## 1. Why ScratchArch uses LLVM IR as input

ScratchArch is a compiler target architecture. Real-world compilers (Clang/LLVM,
rustc) already produce LLVM IR as their intermediate representation. By
translating LLVM IR to SAIR, ScratchArch can accept input from any language
that has an LLVM frontend (C, C++, Rust, Swift, Zig, etc.) without writing
language-specific parsers.

The pipeline is:

```
C/Rust/etc. → LLVM IR (.ll) → ScratchArch LLVM Translator → SAIR → ISA → VM
```

## 2. Why this is not a full LLVM backend

A full LLVM backend (registered with `llc`) would require:

- An LLVM `TargetMachine` implementation in C++
- LLVM instruction selection (SelectionDAG or GlobalISel)
- LLVM register allocation integration
- Machine code emission hooks

This prototype does **none** of those. Instead, it:

1. Parses **LLVM textual IR** (`.ll` files) using a Rust-native parser
2. Translates to SAIR, which already has a reference interpreter
3. Delegates ISA lowering to the existing `IsaLowerer`

This avoids any dependency on the LLVM C++ libraries. The trade-off is that we
only support a limited subset of LLVM IR — enough for simple programs but not
arbitrary C code.

### What we gain

- Zero LLVM dependency (no `llvm-sys`, no `libLLVM.so`)
- Runs in pure Rust
- Easy to test and extend
- Works in any environment (no LLVM installation required)

### What we lose

- No support for complex LLVM IR features (loads/stores with align, volatile,
  metadata, debug info, etc.)
- No optimization pipeline integration
- No target feature definitions

## 3. LLVM → SAIR mapping

> The authoritative, current compatibility matrix lives in
> [`docs/specification/LLVM_COMPATIBILITY.md`](../specification/LLVM_COMPATIBILITY.md).
> This section summarises the mapping rules; the spec documents exact status,
> verification, and known gaps.

### Types

| LLVM IR | SAIR | Notes |
|---------|------|-------|
| `i1` | `IrType::I1` | Boolean type |
| `i8` | `IrType::I8` | Byte type |
| `i16` | `IrType::I16` | Short type |
| `i32` | `IrType::I32` | Primary integer type |
| `i64` | `IrType::I64` | Fully supported on the SAIR interpreter (two cells on the VM) |
| `ptr` | `IrType::Pointer` | Generic pointer |
| `void` | `IrType::Void` | No return value |
| `[N x T]` | element type | For value ops; *allocations* are byte arrays |
| `%T` / `{…}` | element type | Struct layout only (via `scratcharch_target::layout`) |

Floating-point (`half`/`float`/`double`/`f128`), vectors, `i128` and other
unsupported types produce a clear **UnsupportedType** error at parse time.

### Values

| LLVM IR | SAIR |
|---------|------|
| `%name` | `ValueId` (via name lookup in value map) |
| `@name` | Function name (for call targets) |
| Integer literal | typed `Instruction::Const` (`I32`/`I8`/`I16`/`I64`…) |
| `true`/`false` | `Constant::I1(v)` |

### Instructions

| LLVM IR | SAIR | Notes |
|---------|------|-------|
| `add`/`sub`/`mul` | `Add`/`Sub`/`Mul` | Wrapping (SAIR semantics) |
| `udiv`/`urem` | `Div`/`Rem` | SAIR Div/Rem are unsigned floor — one-to-one, any width |
| `sdiv`/`srem` | expansion | Signed, trunc toward zero, dividend sign; built from magnitudes (`translate_signed_divrem`) |
| `icmp eq/ne/ult/ugt/ule/uge` | `Eq`/`Lt`/`Gt` (+negation) | Unsigned bit-pattern compare, one-to-one, any width |
| `icmp slt/sgt/sle/sge` | sign-bit select (`emit_signed_lt`) | Exact for negatives and mixed signs: `slt(a,b) = sign(a)!=sign(b) ? sign(a) : a <u b`; the other three derive from it |
| `alloca` | `Alloca` | Byte array sized from layout |
| `load`/`store` | `Load`/`Store` | Typed |
| `call` | `Call` | Direct; `llvm.*`/runtime intrinsics dispatched by the interpreter |
| `ret` | `Terminator::Return` | Value or void |
| `br` | `Terminator::Branch` | Unconditional |
| `br i1 …` | `Terminator::CondBranch` | Conditional |
| `select` | `Select` | Produces a value |
| `switch` | chain of `eq`+`CondBranch` | Per-case dispatch to the default |
| `unreachable` | `Unreachable` | No-return marker |
| `zext`/`sext`/`trunc`/`bitcast` | matching `CastOp` | Integer/pointer conversions |
| `ptrtoint`/`inttoptr` | `PtrToInt`/`IntToPtr` | Pointer ↔ address integer |
| `getelementptr` | single byte-offset `Gep` over `i8` | Indices folded via `scratcharch_target::layout` |

| `phi` | SAIR `Phi` | Loop-carried and merge phi, one-to-one; predecessor labels remapped to SAIR block names; verified on the interpreter and the VM (edge copies) |
| `llvm.memcpy`/`llvm.memmove`/`llvm.memset` | calls resolved at runtime | Handled by the SAIR interpreter's memory-intrinsic family; the VM has no body to run and rejects these with an explicit diagnostic |
| bitwise ops (`and`/`or`/`xor`/`shl`/`lshr`/`ashr`) | — | **not** translated; explicit **UnsupportedInstruction** error |
| global variables | static data segment + address constants | Data globals lower to a module `StaticData` image; `@name` references become absolute-address `I32` constants (§ module symbols) |

Floating-point ops, `float`/`double` types, vectors, and indirect calls are
**not** translated; they produce a clear **UnsupportedInstruction** error rather
than a silent wrong result.

### Basic blocks

LLVM IR basic blocks (labels) map directly to SAIR `BasicBlock` objects. The
entry block must be first in both representations.

### Functions

LLVM IR `define` maps to `IrFunction`. Parameters become SSA values via
`IrBuilder::add_param()`. `declare`d functions are skipped by the translator —
they are resolved at call sites by the interpreter (runtime registry or the
LLVM bit-intrinsic handler). The first `define`d function named `main` (else
the last non-declaration) is used as the module entry point.

## 4. Supported subset

### Fully supported (SAIR interpreter)

- `define`/`declare` with `i1`, `i8`, `i16`, `i32`, `i64`, `ptr`, `void`
- Arithmetic: `add`, `sub`, `mul`, `udiv`, `urem`, `sdiv`, `srem`
  (signed ops are exact including negative operands)
- Comparison: `icmp eq/ne`, all unsigned predicates at all widths, and the
  signed predicates (`slt`/`sgt`/`sle`/`sge`) expanded exactly via the sign-bit
  identity — correct for negatives and mixed signs at `i8/i16/i32/i64`
- Conversions: `zext`, `sext`, `trunc`, `bitcast`, `ptrtoint`, `inttoptr`
- Memory: `alloca` (with count), `load`, `store`
- Control flow: `br`, `br i1 …`, `ret`, `select`, `switch`, `unreachable`
- `phi`: loop-carried and merge `phi` nodes, predecessor-remapped to SAIR block
  names
- `getelementptr` with constant or dynamic (typed) indices → byte offsets
- Global variables: data globals lower to a module static-data segment; `@name`
  operands become absolute-address constants; word-granular segments are
  VM-exact, byte-granular segments run exactly on the interpreter
- LLVM bit intrinsics: `llvm.bswap`, `llvm.ctpop`, `llvm.ctlz`, `llvm.cttz`
  (widths 8/16/32/64), resolved by the interpreter as pure expansions
- `llvm.memcpy`/`llvm.memmove`/`llvm.memset` and runtime intrinsics
  (`__scratcharch_memcpy`, `__scratcharch_memset`, …) via the
  `scratcharch-runtime` registry (declaration calls resolve at call time)

### Not supported (rejected with an explicit diagnostic)

- Floating-point values/ops (`float`/`double`/`fadd`/`fsub`/…), vectors,
  `i128` and other unsupported types
- Bitwise ops (`and`/`or`/`xor`/`shl`/`lshr`/`ashr`) — SAIR has no bitwise ops
- Indirect calls through function pointers (no function-pointer ABI)

### VM-backend limitations

The VM carries 64-bit integers as two 32-bit limbs for add/sub/compare/cast/
load/store/select/phi. `i64` mul/div/rem are rejected with an explicit
diagnostic (no 64-bit divide/widen ISA). Dynamic `getelementptr` into a byte
array (i64 index, scale 1) lowers and runs; dynamic scaling by multi-byte
element sizes needs an `i64 mul` and is rejected. Sub-word/byte global data and
`llvm.*`/runtime intrinsics are interpreter-exact; the VM rejects those modules
with a named diagnostic instead of misreading bytes. Nothing is approximated.

### Known gaps (see LLVM_COMPATIBILITY.md)

- Bitwise ops have no SAIR form, so C code that compiles to `and`/`or`/`xor`/
  shifts at `-O0` is rejected rather than approximated.
- `switch` lowers to a linear chain of `eq`+`CondBranch` compared at the
  switch's own type width; huge case tables are not specialised into a jump
  table or binary search.
- No sign of "poison": zero-operand `ctlz`/`cttz` yield the width, and overflow
  wraps, per SAIR's no-poison policy.
- The **Scratch backend** (SAIR → ScratchGraph) is a construction surface only;
  the Scratch execution model cannot express flat memory, pointers, or arbitrary
  call frames (§6.2 of LLVM_COMPATIBILITY.md), so LLVM→Scratch semantics are out
  of the v0.2 scope.

## 5. Crate structure

```
crates/scratcharch-llvm/
├── Cargo.toml
├── src/
│   ├── lib.rs          — Public API (translate_llvm)
│   ├── parser.rs       — Lexer + parser for LLVM IR text
│   ├── translator.rs   — LLVM AST → SAIR IrModule
│   └── errors.rs       — LlvmError type
└── tests/
    └── translator_tests.rs — Integration tests
```

### Public API

```rust
/// Parse LLVM IR text and translate to a validated SAIR module.
pub fn translate_llvm(input: &str) -> Result<IrModule, LlvmError>
```

The returned `IrModule` passes SAIR validation and can be executed by the
SAIR interpreter.

### Dependencies

- `scratcharch-ir` (SAIR types, builder, validation)
- `scratcharch-sair-interpreter` (test-only: execution verification)

No LLVM libraries are required.

## 6. Pipeline

```
LLVM IR text (.ll)
    │
    ▼
parser.rs (lexer + recursive descent parser)
    │
    ▼
LlvmProgram (AST: functions, blocks, instructions)
    │
    ▼
translator.rs (walk AST, call IrBuilder)
    │
    ▼
IrModule (SAIR) → validate() → Interpreter.run()
```

Every translated module passes through:

1. LLVM IR parsing
2. SAIR construction via builder
3. SAIR validation (`IrModule::validate()`)
4. (Optional) interpreter execution for result verification

## 7. Test cases

The committed fixtures under `tests/c_programs/` (real clang output, committed)
plus a fresh-clang re-compile of each `.c` on every run:

| Corpus | Pipeline covered |
|--------|------------------|
| `hello`, `add`, `array`, `struct`, `pointer`, `string` | value flow, memory, GEP, calls |
| `factorial`, `fib`, `recursion` | control flow, recursion, multi-block |
| `signedcmp` | signed `icmp` on negatives/mixed signs (exact expansion) |
| `i64arith` | `i64` add/sub across the limb boundary, signed `i64` compare, trunc |
| `memory`, `memintrin` | memory intrinsics (`__scratcharch_memcpy`, `llvm.memcpy`/`memmove`/`memset`) |
| `intrinsics` | `llvm.bswap/ctpop/ctlz/cttz`, 16/32/64-bit, `i1 true` immarg |
| `signed` | negative `sdiv`/`srem` (trunc-toward-zero, dividend sign) |
| `globals` | module-level global data: scalar/array/string/pointer relocations |

The corpus is executed on three surfaces (`scratcharch-pipeline/tests/
llvm_corpus_surfaces.rs`): the SAIR interpreter, the ISA VM (exact where
supported, explicit diagnostic where not), and native execution through a real C
compiler — all three must agree per fixture.

`translator_tests.rs` adds focused coverage (50+ tests): constants, i64
arithmetic, conversions, `select`/`switch`/`unreachable`, unsigned predicates,
signed & unsigned division (including negatives and `i64::MIN / -1` wrapping),
signed `icmp`, `phi`, and explicit rejection of unknown intrinsics and
unsupported constructs. `scratcharch-driver`'s VM-backend tests run the
i32-representable fixtures and the multi-cell `i64` corpus through the VM and
require interpreter/VM agreement; `scratcharch-llvm/tests/` adds hand-written
loop/`phi` fixtures and the memory-intrinsic matrix.

## 8. Future work

- **Bitwise operations**: `and`, `or`, `xor`, `shl`, `lshr`, `ashr` (needs SAIR
  ops; currently an explicit UnsupportedInstruction).
- **Floating point**: `fadd`, `fsub`, `fmul`, `fdiv` (SAIR has `f64`, but the
  LLVM frontend rejects float types for now).
- **Indirect calls**: a function-pointer ABI (currently rejected at parse time).
- **Volatile / alignment / metadata**: currently skipped at parse time.
