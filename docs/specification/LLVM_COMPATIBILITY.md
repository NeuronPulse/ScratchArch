# LLVM Compatibility

> Specification version: **v0.6**
> Status: normative (defines what `scratcharch-llvm` accepts and how it behaves)
> Companion documents: [`LLVM_TRANSLATION.md`](../design/LLVM_TRANSLATION.md)
> (design/mapping), [`AGGREGATE_DATA_MODEL.md`](../design/AGGREGATE_DATA_MODEL.md)
> (aggregate layout contract), [`LLVM_COMPATIBILITY_STATUS.md`](../design/LLVM_COMPATIBILITY_STATUS.md)
> (generated status report), [`LLVM_COMPATIBILITY_BENCHMARK.md`](../design/LLVM_COMPATIBILITY_BENCHMARK.md)
> (the long-term compatibility benchmark), [`LLVM_COMPATIBILITY_BASELINE.md`](../design/LLVM_COMPATIBILITY_BASELINE.md)
> (the recorded benchmark snapshot), [`SAIR_FORMAT.md`](./SAIR_FORMAT.md),
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

**Benchmark percentage vs opcode coverage.** The percentages reported by the
compatibility benchmark
([`LLVM_COMPATIBILITY_BENCHMARK.md`](../design/LLVM_COMPATIBILITY_BENCHMARK.md))
are *not* opcode-coverage counts and must not be read as one. A percentage is a
statement about whole programs: how many real clang fixtures **complete** each
toolchain layer. A fixture dies on the first construct its current stage cannot
handle, so a program that only needs supported ops can still read as a failure
at a deeper layer if a single construct blocks it there. Two toolchains can
cover the identical opcode surface yet report very different percentages,
because programs fail on the first gap they meet. This is deliberate: the
benchmark measures *whole-program reach through the layers*, which is what a
user actually experiences, while the matrix above states per-construct
guarantees. The benchmark never changes the ISA or a backend merely to raise a
percentage, and it never reclassifies a real rejection as support.

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
| **Interpreter only** | Exact on the interpreter; the VM backend has no lowering for it yet (every use is an explicit diagnostic). A faithful VM path is compatible with the current ISA and tracked. **No matrix row currently carries this status** — the last one (bodyless runtime intrinsics) closed when the VM gained its load-time runtime resolver (§6.1). The status is retained so a future construct with the same shape is classified honestly rather than mislabelled. |
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
  backend resolves a bodyless named `Call` *once, at load time* against the
  same runtime names (§6.1) and otherwise runs only functions with a `define`d
  body.
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
| `[N x T]` | element type | Supported | As allocation/layout for `alloca`/`getelementptr` and as a global's storage; the byte geometry comes from `DataLayout` (`AGGREGATE_DATA_MODEL.md`) |
| `{ … }` / anonymous struct | element type | Supported | As layout for `alloca`/`getelementptr` and as a global's storage; field offsets come from `DataLayout` |
| `[N x T]` / `{ … }` **global initializers** | static-data image | Supported | Constant integer leaves, nested arrays/structs, `c"…"` byte arrays, pointer relocations, `zeroinitializer`; serialized little-endian at `DataLayout` offsets with padding exactly zero. A shape mismatch is an explicit diagnostic (`AGGREGATE_DATA_MODEL.md` §3) |
| `[0 x T]` / `{}` (zero-sized) | — | Unsupported | `LayoutError::ZeroSized` — a zero-sized type has no layout, and inventing one would shift every later offset. Flexible array members are future work |
| aggregate *values* | — | Unsupported | Loads/phis/params returning a struct/array *by value* are not modeled — no aggregate-by-value ABI; the diagnostic points at GEP over the layout |
| `half`/`float`/`double`/`f128` | — | Unsupported | Explicit diagnostic at parse (SAIR has `F64` but the frontend does not expose floats) |
| `i128`, `x86_mmx`, vectors | — | Unsupported | Explicit diagnostic at parse |

**Aggregate data model (v0.6).** Aggregates are a *layout* concern, not a value
representation: a struct or array never becomes a SAIR value, a VM cell, or a
Scratch list element — it becomes bytes at DataLayout-computed offsets, and every
access to it is an ordinary scalar load/store at one of those offsets.
`scratcharch_target::layout` is the single authority for that geometry
(`AggregateType` / `TypeLayout`: scalar size and alignment, array stride, struct
alignment and field offsets, aggregate size, padding), and the translator holds
no layout arithmetic of its own — `type_size_align`, global seeding and GEP
offsets all call into it, so a field offset cannot drift between the GEP path,
the allocator and the static-data serializer. The source layout is
`DataLayout::x86_64()` because the clang corpus is compiled for
`x86_64-pc-linux-gnu` and clang bakes those offsets into its `getelementptr`
constants; SA48 (4-byte pointer) is the execution layout.

Struct offsets, natural-alignment padding, nested structs, struct arrays and
whole-struct `memcpy` assignment are exercised against real clang `-O0` output
(field offsets, `char-mix`: `{i8,i32,i16}` at offsets 0/4/8; `aggstruct`:
`{i8,i64,i16}` at offsets 0/8/16, size 24), and the v0.6 fixtures add nested
aggregate *global initializers* (`aggglobal`, `aggmatrix`, `aggnested`),
array-of-struct and struct-of-array images (`aggarray`, `aggnested`), and byte
views of aggregate memory (`aggbytes`). Every fixture runs byte-exact on both
engines, which validates the layout model against the clang/x86-64 ABI the
fixtures were compiled with. Aggregate **by value** (a struct returned from /
passed to a function as a first-class value) remains unsupported —
`alloca` + GEP + load/store is the supported model, and only `i64`-wide scalar
values cross the call ABI as two cells.

## 5. Instruction matrix

Legend for the per-row status: **S** = Supported, **P** = Partial,
**IO** = Interpreter only, **VU** = VM unsupported, **U** = Unsupported.
Every row is exact on the SAIR interpreter unless the notes say otherwise.
As of v0.5 every row is **S**, **P**, or **U**: no row carries **IO** or **VU**
(the bodyless-runtime-intrinsic row, the last **IO**, closed with the VM
load-time runtime resolver — §6.1). Both statuses stay defined so a future
construct of the same shape is classified honestly.

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
| `bitcast` int↔int | `Bitcast` | **S** | Same-width reinterpretation; both engines copy the cell(s) through unchanged |
| `ptrtoint` | `PtrToInt` | **S** | Address → integer of pointer width; `i32` copies the address cell, `i64` zero-extends into the `(low, high)` limb pair |
| `inttoptr` | `IntToPtr` | **S** | Integer → address: any single-cell integer copies through; an `i64` source **traps on a nonzero high limb** (never a silent truncation — same value the interpreter rejects), else its low limb is the address (§6.1) |
| float conversions (`fptosi`, …) | — | **U** | Floats unsupported |

### 5.4 Memory

| LLVM | SAIR | Status | Notes |
|------|------|--------|-------|
| `alloca T` / `alloca T, i32 N` | `Alloca` (byte array) | **S** | Sized from the target layout; constant count |
| `load` / `store` | `Load` / `Store` | **S** | Typed; `i64` = two limbs at `addr`/`addr+4` on the VM. Sub-word types (`i1`/`i8`/`i16`) write exactly `ty.size_in_bytes()` bytes via `Load8`/`Store8`, little-endian, neighbours preserved (§6.1) |
| `getelementptr` | single byte-offset `Gep` over `i8` | **S** | Constant indices fold to a byte offset (works everywhere); a *dynamic* index on a byte array carries its low limb into the 32-bit address space; a dynamic index on a multi-byte-element array multiplies the index by the element size with the same software `mul` the VM uses for full-width `i64` multiply (§5.1). Into an aggregate, every offset is a `DataLayout` quantity — array stride = element size, struct field offset = the layout's offset, index 0 = the whole pointed-to object's size — and the index chain descends one level per index, so `t.rows[1].b.y` is a plain sum of layout constants. Interpreter and VM are exact for all of these |
| `volatile` / `atomic` | — | **U** | Rejected (alignment and other attributes are tolerated) |
| global data (`@g = global T init`) | static data segment; uses of `@g` are its absolute address (I32) | **P** | Flat segment laid out below the stack floor (`STATIC_DATA_BASE`); the region below `stack_limit` is never stack-allocated, so it is safe static data. Seeding is **byte-exact on both backends**, so word- *and* sub-word/byte leaves run on the interpreter and the VM alike (§6.1); a segment that does not fit below the stack floor is rejected on either engine. Supported initializers: scalar integers (`i1/i8/i16/i32/i64`), `zeroinitializer`, `null`, pointer relocations (`ptr @other`), `c"…"` byte strings, and flat arrays of scalars. Rejected with explicit diagnostics: struct/void globals, `undef`/`poison`, nested-aggregate (array-of-array) initializers, aggregate-constant (struct / array-of-struct) element initializers (a parser gap — §8.5), and relocations to undeclared globals. **Address-of-function is rejected** (no function-pointer ABI — see the indirect-`call` row in §5.5) |

### 5.5 Calls, intrinsics, and control flow

| LLVM | SAIR | Status | Notes |
|------|------|--------|-------|
| `call @f` (direct, recursion) | `Call` | **S** | Independent frames. VM path pinned for the ≤32-bit ABI; `i64`-wide arguments/returns travel the same cell-slot ABI |
| `call @__scratcharch_*` | `Call` | **S** | Three-operand `__scratcharch_memcpy(dst, src, const len)` is expanded by the translator to byte copies (§6.1). Every other runtime intrinsic (`__scratcharch_memcpy/memmove/memset/memcmp/strcmp/strcpy/strncpy/strlen/abort/panic/trap`) is a bodyless named call that resolves through the shared `scratcharch-runtime` **`IntrinsicRegistry`** on *both* engines: the interpreter dispatches its registry entry, the VM resolves the name at load time into a `Code::CallRuntime` entry and runs the same registry body over a flat memory view (§6.1, RUNTIME.md). The registry entry carries the operand-stack **arity** (`arg_words`/`result_words`) so each engine knows the call's cell shape before it runs |
| `call @llvm.memcpy/memmove/memset.*` | `Call` → expansion / resolution | **S** | Canonical constant-length, non-volatile forms (`llvm.memcpy.p0.p0.iN`, `llvm.memmove.p0.p0.iN`, `llvm.memset.p0.iN`) are expanded by the translator into width-exact `i8` load/store sequences, so they run on the interpreter *and* the VM. Runtime-length, volatile, and oversized (>4096-byte) calls that survive translation as calls are now also VM-supported: the VM resolves them to a flat memory op with the interpreter's contiguous-range, null-destination, and as-if-through-a-temporary rules (§6.1). Non-canonical variants (`llvm.memcpy.inline.*`, `llvm.memcpy.element.unordered.*`) get an explicit diagnostic on both engines — never a silent copy |
| `call @llvm.bswap/ctpop/ctlz/cttz` | `Call` | **S** | Both engines evaluate the same shared leaf, `scratcharch_runtime::bit_intrinsic_value`, over the declared width (`i8…i64`) — masking to width, `ctlz`/`cttz` of `0` yielding the width, `bswap` reversing the low `width/8` bytes (§6.1, LLVM_TRANSLATION.md) |
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
| `i1`/`i8`/`i16` loads and stores (byte memory) | Supported | Supported — width-exact `Load8`/`Store8` sequences (§6.1 below) | Supported |
| `bitcast`/`ptrtoint`/`inttoptr` | Supported | Supported — cell-preserving reinterpretation; `inttoptr i64` traps on a nonzero high limb | Supported |
| constant-length, non-volatile `llvm.memcpy`/`memmove`/`memset` and `__scratcharch_memcpy` | Supported | Supported — expanded by the translator into width-exact `i8` load/store sequences (load-all-then-store, so overlapping `memmove` is well-defined); runs on both engines | Supported |
| runtime-length / volatile / oversized (`>4096` bytes) `llvm.memcpy`/`memmove`/`memset` | Supported | Supported — load-time resolved to a flat memory op with the interpreter's contiguous-range / null-destination / through-a-temporary rules | Supported |
| `llvm.bswap`/`ctpop`/`ctlz`/`cttz.iN` | Supported | Supported — resolved at load time, evaluated by the shared `bit_intrinsic_value` leaf | Supported |
| `__scratcharch_*` builtins (`memcpy`/`memmove`/`memset`/`memcmp`/`strcmp`/`strcpy`/`strncpy`/`strlen`/`abort`/`panic`/`trap`) | Supported | Supported — resolved at load time through the same `IntrinsicRegistry` the interpreter consults; the shared registry body runs over a flat `ByteMemory` view of VM memory | Supported |
| global data, word-granular (aligned `i32`/`i64`/`ptr` leaves) | Supported | Supported (VM-exact static segment) | Supported |
| global data, sub-word/byte leaves (`i1`/`i8`/`i16`, byte strings, arrays with byte elements) | Supported | Supported (byte-exact static segment; neighbouring bytes preserved) | Supported |

No `VM unsupported` row remains: every construct the VM can express — exactly,
via a program-level software helper (§5.8), or through the load-time runtime
resolver below — is marked Supported. **No `Interpreter only` row remains
either** as of this revision: the last one (bodyless runtime intrinsics) closed
when the VM gained the load-time runtime resolver.

**Runtime and intrinsic calls on the VM (load-time resolution).** The ISA is
frozen and gains no libc-specific instruction; instead the *VM's* `Code` space
grows one variant, `CallRuntime(FnId)`. At `load_program` the VM resolves every
bodyless named `Call` **once, per name, per program** against the same runtime
names the interpreter resolves at run time, and rewrites it to the resolved
table entry. Resolution order mirrors the interpreter's dispatch tiers:

1. **SART builtins** — the name is looked up in the shared
   `scratcharch-runtime` `IntrinsicRegistry` (the *same* registry instance
   semantics the interpreter uses); its `IntrinsicSignature` supplies the
   operand-stack arity (`arg_words`/`result_words`) the VM needs to pop
   arguments and push results.
2. **`llvm.mem*`** — canonical variants resolve to a flat `Copy`/`Move`/`Set`
   op sized from the length type in the name; the VM applies the interpreter's
   documented rules (a zero length succeeds *before* any null check; a null
   destination is rejected; a non-contiguous range is rejected; the copy is
   as-if-through-a-temporary).
3. **`llvm.bswap`/`ctpop`/`ctlz`/`cttz.iN`** — resolved per family and width and
   evaluated through the shared `bit_intrinsic_value` leaf, so interpreter and
   VM cannot diverge.

The execute loop dispatches on the resolved **kind** — an enum match — never on
a name string; there is no per-execution string comparison. Anything else stays
a load-time `undefined function: <name>` (the interpreter rejects the same
program with an unknown-intrinsic error when the call is reached). No engine
ever approximates: what the VM cannot execute faithfully it refuses to load.

**Compiler-synthesized helpers are separate from runtime functions.** The
`__sair_*` helpers (EXECUTION_MODEL.md §5.7–§5.8) are *lowering artifacts* the
ISA lowerer appends to the program and are never resolved through the runtime
registry; the registry path exists only for calls the *source* program makes to
a bodyless named function. The two families do not overlap by name or by
mechanism.

**Failure categories stay distinguishable on the VM.** A runtime failure is not
collapsed into the ISA trap: the VM raises distinct `Abort`, `Panic`, `Trap`
(runtime), and `DivisionByZero` categories, and `unreachable` remains the ISA
`Trap` path (§5.6). A runtime `abort` is never reported as an `unreachable`, and
a division by zero is never reported as a trap.

The VM gaps above are **backend** limitations, not translator ones. Nothing is
approximated: constructs the VM cannot execute faithfully are rejected with a
diagnostic that names the ISA need. Static data lives below `stack_limit` on
*both* backends — a segment that does not fit below the stack floor is rejected
(`StaticDataTooLarge` / "does not fit below the stack floor"), never allowed to
collide with the downward-growing stack.

**v0.3 ISA extension.** The frozen word ISA gains three **additive** stack-machine
primitives — `Load8`, `Store8`, `Trap` — defined normatively in
[`ISA.md`](./ISA.md) Appendix A and [`EXECUTION_MODEL.md`](./EXECUTION_MODEL.md)
§5.6. They are target-independent (no Scratch/LLVM/libc semantics). Word
`Load`/`Store` are unchanged — the matrix never claims a VM capability before the
VM exercises it.

**Byte-exact memory and reinterpretation on the VM.** `Load8`/`Store8` give the VM
width-exact sub-word memory and close the last byte-granular gaps. A single-limb
load/store writes exactly `ty.size_in_bytes()` bytes, little-endian, so an `i8`
store never touches its neighbours and an `i16` is two byte accesses
(`hi << 8 | lo`); an `i1` load is `byte > 0` after `Load8`, a genuine flag.
Static data is seeded **byte-exact** on both backends, so sub-word/byte leaves
(`i1`/`i8`/`i16`, byte strings, arrays with byte elements) run on the VM too, not
just the interpreter. Reinterpretation is lowered as a **zero-cost
cell-preserving** copy — never a conversion where LLVM requires a value
transformation: `ptrtoint` copies the 32-bit address cell (`i64` zero-extends into
the limb pair), `inttoptr` copies a single-cell integer back (an `i64` source
**traps on a nonzero high limb** — the interpreter rejects the same value — rather
than truncating), and `bitcast` is accepted only as a same-size, same-kind no-op
(pointer→pointer or equal-width integer→integer). Both engines drive memory width
from the IR type (`IrType::size_in_bytes`) and cell decomposition from the
`TargetProfile`, so interpreter and VM agree byte-for-byte; the differential suite
and the `bytes`/`reinterp` corpus fixtures (native exit 412 / 331, §9) pin it.
Layout decisions flow through the profile and the source layout
(`scratcharch_target::layout`); no width is hard-coded in the VM backend.

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

**Constant-length memory ops expand to the VM.** The translator expands
constant-length, non-volatile calls to the canonical `llvm.memcpy`/`llvm.memmove`/
`llvm.memset` families (and the three-operand `__scratcharch_memcpy`) into
straight-line width-exact `i8` `Load8`/`Store8` sequences at compile time. A
copy reads **every source byte into an SSA value before storing any destination
byte** — the as-if-through-a-temporary semantics `memmove` guarantees for
overlapping regions at no extra cost — and `memset` is a per-byte store of the
resolved value. The expansion is bounded (`MAX_INLINE_MEMOP` = 4096 bytes) and
gated on a literal `isvolatile = false`; runtime-length, volatile, and oversized
calls are left as ordinary calls and are handled by both engines through the
runtime resolver below. Non-canonical names
that merely share the `llvm.mem*` prefix (`llvm.memcpy.inline.*`,
`llvm.memcpy.element.unordered.*`) are *not* expanded and are rejected with a
diagnostic that names the intrinsic — the translator matches exactly the same
canonical families the interpreter resolves, so no engine ever treats a distinct
intrinsic as a plain byte copy. The `memory`/`memintrin`/`struct-assign` corpus
fixtures (native 6 / 1 / 56) and the `memintrin_tests.rs` suite pin this
end-to-end on both engines.

### 6.2 Scratch backend

`scratcharch-scratchgraph::lower` can map SAIR into a ScratchGraph project. Its
memory model is **byte-addressable and width-exact**
(`docs/specification/SCRATCH_MEMORY.md`): a stage-owned list backed the heap, one
list item per byte, little-endian `i1`/`i8`/`i16`/`i32`/`i64`/`ptr` loads and
stores with a mathematical-signed value convention reconciled to SAIR raw-bit
semantics, exact static-data seeding, and `HeapAlloc`/`Load`/`Store`/`HeapIndex`
as first-class project statements. The Scratch execution model still differs
from the SAIR/VM memory model in the ways that matter for *execution*: per-target
variables and lists, sprites/scripts, broadcast events, no `malloc`, and no
arbitrary call stack with independent frames. The backend is a **construction**
surface (it emits a `Project` and verifies the lowering by formula and
structure), not an executor — whole-program *execution* semantics on real Scratch
are not claimed until a standalone reference executor exists
(`docs/design/SCRATCH_NUMERIC_MODEL.md` §5). Every construct that depends on
constructs the model cannot build exactly is **Scratch backend unsupported** —
even where the interpreter and the VM execute it — and is rejected with a named
diagnostic, never approximated.

**Aggregates need no Scratch construct** (v0.6). Because an aggregate reaches the
backend as `DataLayout` byte offsets plus scalar leaves, the existing byte-exact
heap already realizes one: a static-data image seeded one list item per byte, and
width-exact little-endian scalar loads/stores at layout offsets. The same bytes
are therefore observable through a typed field access, a nested GEP chain, and an
`unsigned char *` byte view (`aggbytes`) — which is exactly what the old
one-cell-per-address model could not do (`AGGREGATE_DATA_MODEL.md` §6).

## 7. Explicitly rejected — summary

Anything not listed above is rejected with an explicit, actionable diagnostic
(`UnsupportedInstruction` / `UnsupportedType`), including: float ops and float
conversions, aggregate *values*, zero-sized types (`[0 x T]`, `{}`), indirect
calls, the unsupported global shapes named in §5.4 (`undef`/`poison`, `void`
globals, relocations to undeclared globals, initializer/type shape mismatches),
unknown `llvm.*` intrinsics, `i128`/vectors, and `volatile`/`atomic`.
ScratchArch never silently drops an instruction, never lowers a test standard to
force a PASS, and never changes frozen ISA semantics merely to satisfy a
frontend case.

## 8. Known gaps

1. **Runtime intrinsics are supported on both engines, with no VM gap left.**
   `__scratcharch_*` builtins, the `llvm.*` bit intrinsics, and runtime-length /
   volatile / oversized memory ops all resolve on the ISA VM through the
   load-time runtime resolver (§6.1). The resolver is deliberately *closed*: a
   name it does not know is a load-time `undefined function` on the VM and an
   unknown-intrinsic error on the interpreter — never a silent approximation.
   Adding a runtime function therefore means registering it (with its arity) in
   `scratcharch-runtime`, which both engines then pick up.
2. **Hex literals are not lexed** (`0x…`); integer constants are decimal.
3. **Poison is not modeled.** Per SAIR's no-poison policy, overflow wraps and
   zero-operand `ctlz`/`cttz` return the width even when LLVM would permit
   poison. This is a deliberate, documented divergence.
4. **`switch` is a linear chain**, not a jump table or binary search.
5. **Flexible array members and other zero-sized types** (`struct S { int n; int
   a[]; }`, explicit `[0 x T]`, `{}`) are rejected by `DataLayout` with
   `LayoutError::ZeroSized`. Giving them a made-up size would shift every later
   global or field offset, so they stay an explicit diagnostic rather than a
   silent approximation. Supporting them means deciding what a trailing
   unbounded array *is* at the memory-model level — future work
   (`AGGREGATE_DATA_MODEL.md` §7).
6. **Packed / explicitly-aligned aggregates** (`<{ … }>`, `align N` attributes)
   are not modeled. The `align N` attributes clang emits are never semantically
   observable in this subset and are ignored; a genuinely packed struct needs
   its own layout rule.
7. **Aggregate-by-value ABI** — passing or returning a struct by value — is
   future work. It is the reason `IrType` has no aggregate variant: the value
   representation is a prerequisite, and inventing one here would fix the ABI by
   accident (`AGGREGATE_DATA_MODEL.md` §7).

## 9. Verification

Three layers prove the matrix (counts updated at the v0.5 gate, §LLVM_TRANSLATION
and the generated status report):

1. **Committed real-clang fixtures** (`tests/c_programs/*.{c,ll}` plus the
   real-world aggregate corpus under `tests/corpus/llvm/fixtures/`): clang `-O0`
   output, committed, run through `translate_llvm` → interpreter. The corpus
   covers signed comparisons on negatives/mixed signs (`signedcmp`), `i64`
   arithmetic across the limb boundary (`i64arith`), the full bitwise/shift
   family with sign-fill and cross-limb shift amounts (`bitwise`, native exit
   `293345`), full-width `i64` multiply/divide/remainder over real clang
   `mul`/`udiv`/`urem`/`sdiv`/`srem` (`i64muldiv`, native checksum `3579139508`
   — high-limb weighted so a dropped limb or carry perturbs it), byte/sub-word
   globals and mixed-width loads and stores with little-endian neighbour checks
   (`bytes`, native checksum `412` — each width/order/clobber check adds a
   distinct flag), pointer↔integer reinterpretation through integer-carried
   pointers (`reinterp`, native checksum `331`), structs and struct fields
   (`struct`, `nested-struct` 156, `struct-assign` 56 — whole-struct copy via
   constant-length `memcpy`), struct arrays and pointer-to-struct member access
   (`struct-array` 66, `ptrstruct` 36), mixed-width packed struct fields with
   padding (`char-mix` 7988), an `i64` member inside a struct (`i64-struct` 45),
   byte-exact manual string scan (`byte-scan` 5), an aggregate-constant global
   table (`global-agg` 51 — a parser gap through v0.4, now a full success),
   nested aggregate *global* initializers (`aggglobal` 162, `aggmatrix` 314,
   `aggnested` 63 — struct-of-array-of-structs with interior padding and a
   pointer field), array-of-struct and struct-of-array local layout
   (`aggarray` 138), whole-struct `memcpy` assignment over a padded layout
   (`aggstruct` 56), a byte view of aggregate memory through `unsigned char *`
   (`aggbytes` 514 — the same bytes the typed accesses observe),
   runtime-length memory operations
   and every SART string builtin in one program (`runtime_mem` 255 — lengths
   computed at run time so the calls survive translation), plus arrays, globals,
   `llvm.*`/runtime intrinsics, memory intrinsics, and
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
   (memcpy/memmove/memset), `aggregate_tests.rs` (the exact static-data byte
   image an aggregate global initializer produces — layout offsets, zero
   padding, array element stride, the pointer source stride, `zeroinitializer`,
   determinism), `reject_tests.rs` (indirect calls, aggregate *value* types,
   zero-sized types, `undef`/`poison` globals, undeclared relocations), the driver's
   `vm_backend_tests.rs` (interpreter/VM agreement incl. the multi-cell `i64`
   corpus and profile-driven limb counts, byte-granular globals, `i1`/`i8`/`i16`
   loads/stores, and reinterpret round-trips with `inttoptr` overflow trapping
   on both engines), and the interpreter's
   `vm_differential_tests.rs` (40 tests requiring bit-for-bit
   interpreter-vs-VM agreement: `and`/`or`/`xor`/`shl`/`lshr`/`ashr` across
   `i1`–`i64`, poison-region shift amounts, negative sign-fill, cross-limb `i64`
   shifts, full-width `i64` `mul`/`udiv`/`urem` — carry-across-limbs, exact
   long-division matches, boundary divisors, and an `lcg` random sweep over both
   — plus `unreachable` trapping in both engines, `i1`/`i8`/`i16` memory
   round-trips, cell-preserving `ptrtoint`/`inttoptr`/`bitcast`
   reinterpretation with the `inttoptr i64` overflow trap, the
   `llvm.*` bit intrinsics across families and widths, runtime-length
   `llvm.mem*`, the SART string/memory builtins, the distinct
   `Abort`/`Panic`/`Trap`/`DivisionByZero` failure categories, and an unknown
   bodyless callee rejected by both engines).
5. **Compatibility benchmark** (`scratcharch-compat` + `scratcharch test-compat`,
   see [`LLVM_COMPATIBILITY_BENCHMARK.md`](../design/LLVM_COMPATIBILITY_BENCHMARK.md)):
   runs the whole committed corpus (40 fixtures as of the v0.5 gate) through
   parser → SAIR → optimizer → interpreter → ISA lowering → VM → ScratchGraph,
   records a
   PASS/FAIL/UNSUPPORTED per stage and the semantic-core Overall, cross-checks
   interpreter vs VM vs native full-width, and enforces the recorded
   expectations as a regression gate. Numbers are tracked in
   [`LLVM_COMPATIBILITY_BASELINE.md`](../design/LLVM_COMPATIBILITY_BASELINE.md)
   and `tests/corpus/llvm/results/`.

`scripts/run_c_tests.sh` drives the C corpus as a gate.

## 10. Change log

- **v0.6 (2026-09-10)**: the aggregate data model (§4, §5.4; normative contract
  in [`AGGREGATE_DATA_MODEL.md`](../design/AGGREGATE_DATA_MODEL.md)).
  `scratcharch_target::layout` becomes the **single authority for aggregate
  geometry**: `AggregateType` (`Scalar`/`Array`/`Struct`) plus `TypeLayout`
  (`size`/`align`/`field_offsets`) answer scalar size and alignment, array
  stride, struct alignment, struct field offsets, aggregate size and padding, and
  the translator holds no layout arithmetic of its own — `type_size_align`,
  global seeding and every GEP offset call into `DataLayout`, so a field offset
  cannot drift between the GEP path, the allocator and the static-data
  serializer. Aggregates stay a *layout* concern rather than a value
  representation: `IrType` gains **no** aggregate variant (there is still no
  aggregate-by-value ABI), and an aggregate *value* type at an operation is
  rejected with a named diagnostic instead of being flattened. Aggregate global
  initializers (`@t = %S { … }`, `[N x %S] […]`, `[[3 x i32] […]]`,
  `c"…"` byte arrays, pointer relocations, `zeroinitializer`) are parsed
  recursively and serialized into `StaticData.image` little-endian at
  `DataLayout` offsets, with inter-field and trailing padding exactly zero; a
  shape mismatch is an explicit diagnostic, never a silent flatten. Zero-sized
  types (`[0 x T]`, `{}`) are rejected (`LayoutError::ZeroSized`). Aggregate
  `getelementptr` descends one level per index through layout constants
  (array stride = element size, struct field offset = the layout offset, index 0
  = the whole object's size). The VM, the ISA and the Scratch backend gain
  **no new construct**: aggregate memory reaches them as byte offsets plus scalar
  leaves over the existing byte-exact paths, so the six new real-clang fixtures
  (`aggstruct` 56, `aggarray` 138, `aggglobal` 162, `aggmatrix` 314,
  `aggnested` 63, `aggbytes` 514) are VM-exact and construct on Scratch. The
  `global-agg` corpus fixture moves from a parser capability gap to a full
  success; the corpus grows 34 → 40, the semantic core 29 → 36, and the gate is
  green at Overall 90% / VM 90% / Scratch 83% with 0 semantic mismatches.
  Aggregate-by-value ABI, vector types, atomics, exception handling, function
  pointers and zero-sized/flexible-array types stay explicitly out of scope
  (§7, §8).
- **v0.5 (2026-09-10)**: the VM runtime resolver. Bodyless runtime/intrinsic
  calls are no longer `Interpreter only`: at `load_program` the ISA VM resolves
  every named `Call` **once, per name, per program** against the shared
  `scratcharch-runtime` `IntrinsicRegistry` (whose new `IntrinsicSignature`
  carries the operand-stack arity), the canonical `llvm.mem*` variants, and the
  `llvm.bswap/ctpop/ctlz/cttz.iN` bit intrinsics, rewriting it to an internal
  `Code::CallRuntime` entry (spec §2/§5.5/§6.1). The ISA is unchanged — no
  libc-specific instruction — and the execute loop dispatches on the resolved
  kind, never on a name string. Compiler-synthesized `__sair_*` helpers remain
  separate from runtime functions. Interpreter and VM now share the
  bit-intrinsic leaf `scratcharch_runtime::bit_intrinsic_value`, so the two
  engines cannot diverge (including the no-poison zero-operand `ctlz`/`cttz`
  case). VM failure categories stay distinct — `Abort`, `Panic`, `Trap`
  (runtime), `DivisionByZero` — and never collapse into the `unreachable` ISA
  trap. An unresolvable name stays a load-time `undefined function`. The
  runtime-intrinsic corpus fixtures (`string`, `intrinsics`) become full
  successes, the `vm-failure` class disappears (VM 79% → 85%), and the corpus
  grows to 34 with `runtime_mem` — runtime-length memory ops plus every SART
  string builtin in one program. The differential suite grows 31 → 40 tests.
- **v0.4 (2026-09-09)**: real-world coverage expansion. The compat corpus grows
  to 33 fixtures with aggregate programs — structs and struct fields,
  nested/whole-struct assignment, struct arrays and pointer-to-struct member
  access, mixed-width packed fields with padding, an `i64` member inside a
  struct, byte-exact string scanning, and an aggregate-constant global table.
  Constant-length `llvm.memcpy`/`llvm.memmove`/`llvm.memset` (and
  `__scratcharch_memcpy`) are now expanded by the translator to width-exact `i8`
  load/store sequences, moving the canonical memory ops from `Interpreter only`
  to Supported on the VM; runtime-length/volatile/oversized calls and the bit /
  `strlen` intrinsics stay interpreter-only (§6.1). The aggregate-constant
  global initializer is a recorded parser gap (§8.5). Struct/aggregate layout is
  validated against real clang output — no aggregate-by-value ABI.
  Function-pointer and floating-point feasibility studies land as
  `docs/design/FUNCTION_POINTERS.md` / `FLOATING_POINT.md`. The ScratchGraph
  memory model becomes **byte-exact** (SCRATCH_MEMORY.md): a byte-addressable
  list-backed heap with width-exact little-endian load/store, exact static-data
  seeding, and the mathematical-signed value convention, closing every
  width-cast/pointer Scratch gap (`globals`, `signedcmp`, `i64arith`,
  `reinterp`, `struct-array`, `ptrstruct`, `byte-scan`, `char-mix`,
  `i64-struct` → Scratch construct; scratch-backend-failure 12 → 3; Scratch
  gate 45% → 76%). The backend remains a construction surface with a documented
  validation boundary (§6.2, SCRATCH_NUMERIC_MODEL.md §5).
- **v0.3 (2026-09-09)**: additive `Load8`/`Store8`/`Trap` word-ISA primitives
  (EXECUTION_MODEL.md §5.6) realise `unreachable` as a well-defined terminal
  trap on the VM; bitwise ops (`and`/`or`/`xor`) become Supported at every
  width (per-limb word ops for `i64`) and every shift is realised exactly on
  the VM by the program-level software helpers `__sair_shl64`/`__sair_lshr64`/
  `__sair_ashr64` (§5.7); full-width `i64` multiply/divide/remainder close with
  two further software helpers `__sair_mul64`/`__sair_udivrem64` (§5.8), which
  also carry dynamic scaled `getelementptr` to Supported. The reinterpret and
  byte-memory completion then closes the last VM gaps of that milestone:
  `Load8`/`Store8` give width-exact `i1`/`i8`/`i16` loads and stores
  (neighbour-preserving, little-endian) and make the static-data segment
  byte-exact, so sub-word/byte globals are VM-supported; `bitcast`/`ptrtoint`/
  `inttoptr` lower to zero-cost cell-preserving copies (`ptrtoint i64`
  zero-extends; `inttoptr i64` traps on a nonzero high limb), so reinterpret
  casts are VM-supported. Two real-clang fixtures (`bytes` = 412, `reinterp` =
  331) join the corpus with three-way native/interpreter/VM agreement, and the
  differential suite grows to 31 tests. The `VM unsupported`
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
