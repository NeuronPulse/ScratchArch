# LLVM IR → SAIR Translation

> Document version: 0.3
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
| aggregate *values* | temporary slot | Backed by a compiler-managed `alloca`; the "value" is the slot's address and crosses a call as a pointer (`AGGREGATE_ABI.md`). No aggregate `IrType` variant |

Aggregate **geometry** comes from `scratcharch_target::layout` and nowhere else
(`AGGREGATE_DATA_MODEL.md`): `AggregateType` (`Scalar`/`Array`/`Struct`) plus
`TypeLayout` (`size`/`align`/`field_offsets`) answer scalar size and alignment,
array stride, struct alignment and field offsets, aggregate size and padding. The
translator's `type_size_align`, global seeding and GEP offsets are all thin
adapters over `DataLayout` — none of them re-implements a layout rule, so a
field offset cannot drift between the three.

Floating-point (`half`/`float`/`double`/`f128`), vectors, `i128`, non-power-of-two
integer widths (`i24`/`i40`/`i48`) and zero-sized types (`[0 x T]`, `{}`) produce a
clear **UnsupportedType** error at parse/translate time. An aggregate *value* is
*supported* — it is a temporary slot, not an `IrType` — but an aggregate in a
genuinely scalar position (an arithmetic operand, say) is rejected, since it has
no scalar representation to give.

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
| `and`/`or`/`xor` | `And`/`Or`/`Xor` | Width-preserving; `i8`/`i16` masked carriers, `i64` per-limb word ops on the VM (EXECUTION_MODEL.md §5.7) |
| `shl`/`lshr`/`ashr` | `Shl`/`Lshr`/`Ashr` | Effective amount = `amount mod width` (deterministic poison region); `ashr` sign-replicates bit `w-1`. VM realises every shift via software helpers `__sair_shl64`/`__sair_lshr64`/`__sair_ashr64` (§5.7) |
| `icmp eq/ne/ult/ugt/ule/uge` | `Eq`/`Lt`/`Gt` (+negation) | Unsigned bit-pattern compare, one-to-one, any width |
| `icmp slt/sgt/sle/sge` | sign-bit select (`emit_signed_lt`) | Exact for negatives and mixed signs: `slt(a,b) = sign(a)!=sign(b) ? sign(a) : a <u b`; the other three derive from it |
| `alloca` | `Alloca` | Byte array sized from layout |
| `load`/`store` | `Load`/`Store` | Typed |
| `call` | `Call` | Direct; `llvm.*`/runtime intrinsics resolved by the runtime registry — the interpreter at run time, the VM at load time |
| `ret` | `Terminator::Return` | Value or void |
| `br` | `Terminator::Branch` | Unconditional |
| `br i1 …` | `Terminator::CondBranch` | Conditional |
| `select` | `Select` | Produces a value |
| `switch` | chain of `eq`+`CondBranch` | Per-case dispatch to the default |
| `unreachable` | `Unreachable` | No-return marker |
| `zext`/`sext`/`trunc`/`bitcast` | matching `CastOp` | Integer/pointer conversions |
| `ptrtoint`/`inttoptr` | `PtrToInt`/`IntToPtr` | Pointer ↔ address integer |
| `getelementptr` | single byte-offset `Gep` over `i8` | Indices folded via `scratcharch_target::layout`: index 0 = the whole object's size, array index × element size, struct field index → the layout's field offset (constant only — a dynamic field index is rejected). The chain descends one level per index, so a nested access is a plain sum of layout constants |
| `phi` | SAIR `Phi` | Loop-carried and merge phi, one-to-one; predecessor labels remapped to SAIR block names; verified on the interpreter and the VM (edge copies) |
| `llvm.memcpy`/`llvm.memmove`/`llvm.memset` | inline expansion, or calls resolved by the runtime | Constant-length, non-volatile calls are expanded by the translator into width-exact `i8` load/store sequences (both engines). Runtime-length, volatile, and oversized calls survive as calls and are resolved by the runtime registry on *both* engines — the interpreter's memory-intrinsic family, the VM's load-time resolver. This is also how whole-struct assignment (`q = p`) reaches the backends at `-O0` |
| global variables | static data segment + address constants | Data globals lower to a module `StaticData` image; `@name` references become absolute-address `I32` constants (§ module symbols). Aggregate initializers (`%S { … }`, `[N x %S] […]`, `[[3 x i32] […]]`, `c"…"`) nest recursively and are serialized little-endian at `DataLayout` offsets, with inter-field and trailing padding exactly zero; `zeroinitializer` reserves the storage and leaves it zero; a shape mismatch is an explicit diagnostic |

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
- Bitwise and shift: `and`, `or`, `xor`, `shl`, `lshr`, `ashr` at every supported
  width; the full sign-fill and cross-limb `i64` behaviour is proven end-to-end by
  the `bitwise` corpus fixture and the interpreter/VM differential suite
  (`vm_differential_tests.rs`)
- Comparison: `icmp eq/ne`, all unsigned predicates at all widths, and the
  signed predicates (`slt`/`sgt`/`sle`/`sge`) expanded exactly via the sign-bit
  identity — correct for negatives and mixed signs at `i8/i16/i32/i64`
- Conversions: `zext`, `sext`, `trunc`, `bitcast`, `ptrtoint`, `inttoptr`
- Memory: `alloca` (with count), `load`, `store`
- Control flow: `br`, `br i1 …`, `ret`, `select`, `switch`, `unreachable`
- `phi`: loop-carried and merge `phi` nodes, predecessor-remapped to SAIR block
  names
- `getelementptr` with constant or dynamic (typed) indices → byte offsets,
  including nested aggregates: every offset is a `DataLayout` quantity, and the
  index chain descends one level per index
- Aggregates: `[N x T]` and `{ … }` as `alloca` sizes, GEP layouts and global
  storage; scalar leaves are reached through GEP and read/written as ordinary
  scalars, so an aggregate never needs a value representation
  (`AGGREGATE_DATA_MODEL.md`)
- Aggregate **values**: `load`/`store`/`insertvalue`/`extractvalue` of an
  aggregate, and aggregate parameters and returns, become byte-exact copies
  through a compiler-managed temporary slot; `byval` parameters get a private
  callee-prologue copy and a register-returned aggregate gets a hidden result
  pointer appended after the explicit parameters (`AGGREGATE_ABI.md`)
- Aggregate global initializers: constant integer leaves, nested arrays and
  structs, `c"…"` byte arrays, pointer relocations and `zeroinitializer`,
  serialized into the byte-exact `StaticData` image at `DataLayout` offsets with
  padding exactly zero
- Global variables: data globals lower to a module static-data segment; `@name`
  operands become absolute-address constants; seeding is byte-exact, so
  word-granular *and* byte-granular segments are VM-exact
- LLVM bit intrinsics: `llvm.bswap`, `llvm.ctpop`, `llvm.ctlz`, `llvm.cttz`
  (widths 8/16/32/64), evaluated by the shared `bit_intrinsic_value` leaf on
  *both* engines
- `llvm.memcpy`/`llvm.memmove`/`llvm.memset` and runtime intrinsics
  (`__scratcharch_memcpy`, `__scratcharch_memset`, …) via the
  `scratcharch-runtime` registry: the interpreter resolves a declaration call
  when it executes it, the VM resolves it once at load time
  (`scratcharch-vm/src/runtime.rs`) into a `Code::CallRuntime` entry

### Not supported (rejected with an explicit diagnostic)

- Floating-point values/ops (`float`/`double`/`fadd`/`fsub`/…), vectors,
  `i128` and other unsupported types
- Non-power-of-two integer widths (`i24`/`i40`/`i48`) — reached when clang
  coerces a record of that extent; refused by name rather than rounded, which
  would silently shift every later argument (`AGGREGATE_ABI.md` §10)
- An aggregate in a genuinely *scalar* position (e.g. an arithmetic operand) —
  there is no scalar representation to give it, so the diagnostic points at the
  layout-preserving access path instead of inventing one
- An aggregate-returning **declaration** — the hidden result pointer is a
  convention an external function cannot be assumed to honour
- Zero-sized types (`[0 x T]`, `{}`) — no layout exists; `DataLayout` reports
  `LayoutError::ZeroSized` rather than a made-up size that would shift every
  later offset
- Indirect calls through function pointers (no function-pointer ABI)

### VM-backend limitations

The VM carries 64-bit integers as two 32-bit limbs, and the whole `i64`
arithmetic family now runs on it. Add/sub/compare/cast/load/store/select/phi
operate per limb; bitwise/logical ops (`and`/`or`/`xor`) run per-limb at any
width; every shift runs through a software helper
(`__sair_shl64`/`__sair_lshr64`/`__sair_ashr64`) that handles single- and
two-limb operands with exact sign-fill (EXECUTION_MODEL.md §5.7). Full-width
`mul`/`udiv`/`urem` lower the same way to `__sair_mul64` (16-bit schoolbook
multiply `mod 2⁶⁴`) and `__sair_udivrem64` (64-step restoring division that
computes quotient and remainder together, §5.8); the translator's signed
`div`/`rem` expansion already works over magnitudes, so LLVM `sdiv`/`srem`
reach the VM through the unsigned helper unchanged. A helper is appended to the
ISA program only when the module uses the op. Dynamic `getelementptr` — a byte
array indexed by a dynamic value (low limb into the 32-bit address space) or a
multi-byte element array scaled by the same software `mul` — lowers and runs.
Divide-by-zero is manufactured as the word `0/0` error inside the divrem
helper, so the VM and interpreter raise the same `DivisionByZero` on the same
module. Sub-word memory is byte-exact on the VM too: a single-limb
load/store writes exactly `IrType::size_in_bytes()` bytes via `Load8`/`Store8`
(`i16` = two little-endian byte accesses, `i1` load = `byte > 0`), and
reinterpretation (`bitcast`/`ptrtoint`/`inttoptr`) lowers to a zero-cost
cell-preserving copy — `ptrtoint i64` zero-extends into the limb pair, `inttoptr
i64` traps on a nonzero high limb rather than truncating, and `bitcast` is
accepted only as a same-size same-kind no-op. `llvm.*`/runtime intrinsics no
longer limit the VM: a bodyless call resolves at load time through the same
`scratcharch-runtime` registry the interpreter uses (`__scratcharch_*` builtins,
canonical `llvm.mem*` variants, and the `llvm.bswap/ctpop/ctlz/cttz.iN` family,
via the shared `bit_intrinsic_value` leaf), producing a `Code::CallRuntime`
entry and the identical result on both engines. A name the registry does not
know stays a load-time `undefined function`; the VM rejects such a module with a
named diagnostic instead of misreading bytes. Nothing is approximated.

### Known gaps (see LLVM_COMPATIBILITY.md)

- `switch` lowers to a linear chain of `eq`+`CondBranch` compared at the
  switch's own type width; huge case tables are not specialised into a jump
  table or binary search.
- No sign of "poison": zero-operand `ctlz`/`cttz` yield the width, and overflow
  wraps, per SAIR's no-poison policy.
- The **Scratch backend** (SAIR → ScratchGraph) is a construction surface only;
  it carries a byte-exact, width-aware memory model
  (`docs/specification/SCRATCH_MEMORY.md`), but the Scratch execution model
  cannot express pointers or arbitrary call frames and there is no standalone
  executor (§6.2 of LLVM_COMPATIBILITY.md, `SCRATCH_NUMERIC_MODEL.md` §5), so
  LLVM→Scratch *execution* semantics are out
  of the LLVM compatibility scope.

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
| `i64muldiv` | full-width `i64` `mul`/`udiv`/`urem` and signed `sdiv`/`srem` over real clang IR; signed forms ride the translator's magnitude expansion (native checksum 3579139508) |
| `bitwise` | `and`/`or`/`xor` and every shift width/sign-fill combination incl. cross-limb `i64` amounts (native exit 293345) |
| `memory`, `memintrin` | memory intrinsics (`__scratcharch_memcpy`, `llvm.memcpy`/`memmove`/`memset`) |
| `intrinsics` | `llvm.bswap/ctpop/ctlz/cttz`, 16/32/64-bit, `i1 true` immarg |
| `signed` | negative `sdiv`/`srem` (trunc-toward-zero, dividend sign) |
| `globals` | module-level global data: scalar/array/string/pointer relocations |
| `global-agg` | aggregate global initializers: array/struct constants laid out through `DataLayout` |
| `aggstruct`, `aggarray` | whole-struct `memcpy` assignment over a padded layout; array-of-struct and struct-of-array |
| `aggglobal`, `aggmatrix`, `aggnested` | nested aggregate *global* initializers with interior padding, a pointer field, and arrays of strings |
| `aggbytes` | byte view of aggregate memory through `unsigned char *` (the same bytes seen three ways) |
| `abi-struct-param`, `abi-struct-return`, `abi-nested-param`, `abi-nested-return`, `abi-i64-field`, `abi-ptr-field`, `abi-multi-agg`, `abi-mixed-args` | the aggregate ABI: both SysV classes, nested records, an `i64` member, a pointer member, several aggregates in one call, scalar/aggregate interleaving |

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
require interpreter/VM agreement; the interpreter's `vm_differential_tests.rs`
adds bit-for-bit interpreter-vs-VM agreement for the whole bitwise/shift family,
full-width `i64` `mul`/`udiv`/`urem` (long-division matches, boundary divisors,
an `lcg` random sweep), plus the `unreachable` trap on both engines;
`scratcharch-llvm/tests/` adds hand-written
loop/`phi` fixtures and the memory-intrinsic matrix.

Aggregate-specific coverage lives at three layers.
`crates/scratcharch-llvm/tests/aggregate_tests.rs` pins the exact static-data
*byte image* an aggregate initializer produces — struct fields at
`DataLayout` offsets with zero inter-field and trailing padding, array elements
striding by the element's full size, nested arrays laying out recursively, a
pointer field reserving its 8-byte source stride while storing a 4-byte SAIR
address, `zeroinitializer` reserving storage, and determinism across
translations. The layout *rules* themselves (padding, nested struct,
array-of-struct, struct-of-array, stride, alignment, zero-sizing) are unit-tested
in `crates/scratcharch-target/src/layout.rs`.
`crates/scratcharch-llvm/tests/aggregate_abi_tests.rs` pins the ABI shape:
byte-exact copy extents for `load`/`store`/`insertvalue`, the
fresh-slot-per-call-site rule for a register-returned aggregate, the `byval`
prologue copy (with a two-function semantic check that the caller's object
survives a callee write), the appended hidden result pointer, `extractvalue`
offsets agreeing with the GEP path, and translation determinism.
`crates/scratcharch-llvm/tests/reject_tests.rs` pins the boundary
diagnostics: an aggregate in a scalar position (`array value type [2 x i32] has
no SAIR value representation`), a non-power-of-two width (`i24`), an
aggregate-returning declaration, and a zero-sized aggregate (`[0 x i32]` →
`LayoutError::ZeroSized`), alongside the `undef`/`poison` global and
undeclared-relocation rejections.

## 8. Future work

Aggregate *memory* (v0.5) and the aggregate *ABI* (v0.6) are complete; the
boundaries below are deliberate, each a separate milestone rather than a
half-done aggregate feature. Normative detail in
[`AGGREGATE_ABI.md`](./AGGREGATE_ABI.md) §10 and
[`AGGREGATE_DATA_MODEL.md`](./AGGREGATE_DATA_MODEL.md) §7.

- **Non-power-of-two integer widths** (`i24`/`i40`/`i48`): reached when clang
  coerces a record of that extent. SAIR has `i1/i8/i16/i32/i64`, so the width is
  refused by name rather than rounded. Adding a generic arbitrary-width integer
  is a separate milestone, not a rounding shortcut.
- **Aggregate varargs / packed aggregate ABI / vector ABI / atomic ABI / EH**:
  each needs a different ABI rule than the positional-pointer form, and each is
  out of scope for the word ISA today.
- **Zero-sized / flexible-array types**: `struct S { int n; int a[]; }` and
  explicit `[0 x T]` are rejected (`LayoutError::ZeroSized`) rather than given a
  made-up size that would shift every later offset. Supporting them means
  deciding what a trailing unbounded array *is* at the memory-model level.
- **Floating point**: `fadd`, `fsub`, `fmul`, `fdiv` (SAIR has `f64`, but the
  LLVM frontend rejects float types for now).
- **Indirect calls**: a function-pointer ABI (currently rejected at parse time);
  see [`FUNCTION_POINTERS.md`](./FUNCTION_POINTERS.md).
- **Vector types** (`<4 x i32>`) and **atomic operations** (`atomicrmw`):
  parser-level gaps, unrelated to memory layout.
- **Exception handling**: no landing pads or unwind tables.
- **Packed / explicitly-aligned structs** (`<{ … }>`, `align N`): the `align N`
  attributes clang emits are never semantically observable in this subset and
  are ignored; a genuinely packed struct needs its own layout rule.
- **Volatile / metadata**: currently skipped at parse time.
