# Aggregate Data Model

> Milestone: **LLVM Compatibility v0.5**. Normative status matrix:
> [`docs/specification/LLVM_COMPATIBILITY.md`](../specification/LLVM_COMPATIBILITY.md)
> §6. LLVM→SAIR mapping: [`LLVM_TRANSLATION.md`](./LLVM_TRANSLATION.md).
> Byte memory model: [`SCRATCH_MEMORY.md`](../specification/SCRATCH_MEMORY.md)
> and [`docs/specification/MEMORY.md`](../specification/MEMORY.md).

This document defines how LLVM aggregate types (`[N x T]` arrays and
`{ T1, T2, … }` structs) are represented end to end across
LLVM IR → SAIR → static data → the ISA VM and the ScratchGraph backend.

The one-sentence version: **aggregates are a layout concern, not a value
representation.** An aggregate never becomes a SAIR value, never becomes a VM
cell, and never becomes a Scratch list element. It becomes *bytes at
DataLayout-computed offsets*, and every access to it is an ordinary scalar
load/store at one of those offsets.

---

## 1. Aggregate type model

`scratcharch_target::layout` owns the model:

```rust
pub enum LayoutScalar { I1, I8, I16, I32, I64, F64, Ptr }

pub enum AggregateType {
    Scalar(LayoutScalar),
    Array(Box<AggregateType>, u32),   // [count x element]
    Struct(Vec<AggregateType>),       // { field, … }
}

pub struct TypeLayout {
    pub size: u32,               // bytes, including trailing padding
    pub align: u32,              // max of component alignments
    pub field_offsets: Vec<u32>, // struct fields only; empty otherwise
}
```

`LayoutScalar` is deliberately defined here rather than reused from
`scratcharch_ir::IrType`: `scratcharch-ir` depends on `scratcharch-target`, so
the reverse dependency is impossible, and — more importantly — aggregates are
not IR values and must not pretend to be.

### No aggregate `IrType` variant

`IrType` stays scalar and `Copy`. There is **no** aggregate-by-value ABI in this
milestone (§7), so an aggregate can never be the operand or result of a SAIR
instruction, and adding an aggregate variant to `IrType` would invite exactly
the flattening this milestone forbids. The translator rejects an aggregate
*value* type at an operation with a named diagnostic rather than inventing a
representation:

```
array value type [N x T] has no SAIR value representation;
  access its scalar leaves through GEP over the aggregate layout
struct value type %Name has no SAIR value representation; …
anonymous struct value type has no SAIR value representation; …
```

Structural equality matters: two `%struct.Pair`s with identical field lists
produce the same `AggregateType` and therefore the same `TypeLayout`. The
layout is a pure function of the type shape, so it can be computed at any point
in the pipeline without a symbol table.

## 2. DataLayout rules

`DataLayout` is the **single authority** for aggregate geometry. Consumers do
not recompute sizes, strides, alignments or field offsets — the translator's
global seeding, `type_size_align`, GEP and the static-data serializer all call
into it.

| Quantity | Rule |
|---|---|
| `scalar_size(s)` | `i1`/`i8`=1, `i16`=2, `i32`=4, `i64`=8, `f64`=8, `ptr`=`pointer_size_bytes` |
| `scalar_align(s)` | `i1`/`i8`=1, `i16`=2, `i32`=4, `i64`=8, `f64`=8, `ptr`=`pointer_align_bytes` |
| array size | `element_size × count` (elements are contiguous; the element size already includes its trailing padding) |
| array alignment | the element's alignment |
| array stride | `array_stride(element) = element_size` |
| struct field offset | `align_up(cursor, field_align)` walking fields in order |
| struct alignment | `max(field_align)` (1 for an empty field list) |
| struct size | `align_up(cursor, struct_align)` — trailing padding included |
| padding | derivable via `TypeLayout::padding_after(offset, size)`: the gap to the next field offset, or to the end of the type |
| `align_up(v, a)` | `a <= 1 → v`, else `(v + a - 1) & !(a - 1)` |

Two layouts exist and are chosen explicitly:

- **`DataLayout::x86_64()`** (8-byte pointer, 8-byte pointer alignment) — the
  *source* layout for the real-clang corpus. Struct offsets in a
  `x86_64-pc-linux-gnu` module are defined by clang's data layout, so they must
  be computed with clang's pointer size or they will not match the IR's own GEP
  constants.
- **`DataLayout::from_profile(profile)`** — the *execution* layout (SA48: 4-byte
  pointer, 4-byte pointer alignment).

The LLVM frontend uses `x86_64()`; the ISA and Scratch backends consume the
already-resolved byte offsets and do not re-lay-out anything.

### Two sizes per scalar

Pointers are the one place the source and execution models disagree, and the
model keeps both numbers rather than picking one:

- **Layout size** (`scalar_size`) is the source stride — 8 bytes for a pointer —
  and it is what struct field offsets are computed from, so they agree with
  clang.
- **Value width** (`LayoutScalar::value_width()`) is the number of bytes a
  memory instruction actually moves — 4 for a pointer on SA48, 1 for `i1`/`i8`,
  2 for `i16`, 8 for `i64`.

A pointer leaf therefore *reserves* its 8-byte source stride but *stores* the
32-bit SAIR address in the low bytes; the remaining bytes are zero padding that
is never interpreted. This is why an aggregate whose fields include a pointer is
still byte-exact in the observed bytes and byte-zero in the unobserved stride.

### Zero-sized types are rejected

A type occupying no storage (`{}`, `[0 x T]`, LLVM's flexible-array-member
idiom) has no layout and is rejected with an explicit diagnostic:

```
LayoutError::ZeroSized → "cannot lay out zero-sized type [0 x i32]"
LayoutError::Overflow  → "layout of … exceeds the 32-bit address space"
```

Rejecting is deliberate: silently giving a zero-sized type size 1 would corrupt
every following field offset. (See §7 — flexible array members remain future
work.)

### Determinism

The layout is a pure function of `(DataLayout, AggregateType)`. Field order,
`align_up`, and `checked_mul`/`checked_add` are the only operations, so the same
type lays out identically in the translator, the static-data serializer, the
interpreter and the ScratchGraph lowerer — and it is target-aware because the
pointer size and alignment come from the `DataLayout` value, not a global
constant.

## 3. StaticData representation

A module's globals are laid out once and serialized into a single byte image:

```rust
pub struct StaticData { pub image: Vec<u8> }
```

- **Placement.** Globals are assigned in declaration order; each starts at
  `align_up(cursor, align_of_type)`. The image is indexed relative to
  `STATIC_DATA_BASE`, and each global's address is the *absolute*
  `STATIC_DATA_BASE + offset` so an instruction can name a global with a plain
  `const_i32`.
- **Seeding is the backend's job, not the VM's.** The SAIR interpreter seeds the
  image internally; the ISA VM is seeded by `seed_vm_static` in
  `scratcharch-driver`; the ScratchGraph lowerer seeds the byte-exact heap.
  A backend that forgets to seed sees zeros — which is why the differential
  tests exist.
- **Byte-exact.** The image is `vec![0u8; total]`, so every padding byte is
  zero, and each leaf is written at its natural byte offset with its exact byte
  count, little-endian.

Recursive initializer serialization (`write_global_init`) handles:

| Initializer | Meaning |
|---|---|
| `Zero` | `zeroinitializer`, `undef`, `poison`, or an external global without an initializer — the already-zero storage is left untouched |
| scalar literal | an integer / `true` / `false` / `null` leaf, little-endian at the leaf's exact width |
| `GlobalRef(name)` | a pointer leaf: the 4-byte SAIR address of `addr_of[name]`, zero-padded to the 8-byte source stride |
| `Bytes(Vec<u8>)` | a `[N x i8] c"…"` string; the byte count must equal `N` exactly |
| `Array(Vec<Init>)` | element count must equal `N`; element `k` is written at `base + k × array_stride(element)` |
| `Struct(Vec<Init>)` | field count must equal the field count; field `k` is written at `base + layout.field_offsets[k]` |

**Unsupported initializers produce explicit diagnostics, never a silent
flatten.** A shape mismatch is a translation error naming both sides:

```
string initializer holds 6 bytes for a 5-byte array
array initializer holds 3 elements for a 4-element array
struct initializer holds 2 fields for a 3-field struct
initializer … does not match array type …
pointer global initializer references undefined global '@x'
```

Padding is never written by an initializer, so inter-field and trailing padding
bytes are exactly zero — which is what a byte view (§6) observes, and what the
native reference observes too.

## 4. GEP semantics

`getelementptr` folds into a **byte offset** through `ByteOffset`, an
accumulator of a folded constant part plus a list of dynamic (scaled index)
terms. The indexing rule:

| Index | Advance |
|---|---|
| index 0 | the *whole pointed-to object*: `layout_of(elem_ty).size` |
| into `[N x T]` | `array_stride_of_llvm(T)` = `layout_of(T).size`; descend into `T` |
| into `{ T1, … }` | `field_offset_of_llvm(struct, k)`; descend into `Tk` (the index must be a constant; a dynamic field index is rejected — the flat memory model has no per-field dispatch) |
| scalar | pointer arithmetic over the scalar itself: the scalar's layout size |

Every offset comes from `DataLayout`. The translator holds **no** layout
arithmetic of its own — in particular it does not recompute struct field offsets
from a field list, so a field offset cannot drift between the GEP path, the
allocator, and the static-data serializer.

Nesting composes naturally because the accumulator descends one level per index,
so `t.rows[1].b.y` is the chain

```
array → struct → struct → scalar
  = 1 × sizeof(Line)          (array stride)
  + field_offset(Table,rows)  = 0
  + field_offset(Line,b)      = 8
  + field_offset(Pair,y)      = 4
```

**Constant vs dynamic offsets.** When every index is constant the accumulator
finishes as a single **`i32` constant**, which is what the VM can lower. As soon
as one index is dynamic the offset is built in **`i64`** (clang types array
indices `i64`), which the interpreter handles and the VM reports an explicit
diagnostic for. This is a property of the existing scalar path, not of
aggregates: aggregate GEP only changes *which* constants are accumulated.

## 5. VM realization

**Nothing new.** There is no aggregate instruction in the ISA and none was
added. An aggregate access lowers to the existing memory path:

- scalar leaves `i8`/`i16`/`i32`/`i64`/`ptr` use the already-verified exact
  memory path, including the width-exact `i1`/`i8`/`i16` `Load8`/`Store8`
  sequences;
- aggregate global initializers reach the VM through the same static-data
  seeding the scalar globals use, so the bytes a leaf reads are the bytes the
  serializer wrote;
- a `llvm.memcpy` over an aggregate (clang's whole-struct assignment at `-O0`)
  is already expanded by the translator when its length is constant, so it too
  lowers to plain loads/stores.

Aggregation is a *layout* concern, not a new VM value representation — which is
exactly why the VM needed no change to gain full aggregate support, and why an
aggregate fixture cannot be VM-rejected for being an aggregate.

## 6. ScratchGraph realization

The ScratchGraph backend also needed no aggregate construct. It uses the
established **byte-exact heap** (`__scratcharch_heap`, a 1-indexed Scratch list
with one list item per byte) documented in
[`SCRATCH_MEMORY.md`](../specification/SCRATCH_MEMORY.md):

- `ScratchGraphLowerer::lower` seeds `module.static_data.image` into the heap at
  `STATIC_DATA_BASE + 1 + i`, one item per byte, so an aggregate global's padded
  image lands in the heap byte-for-byte;
- scalar leaves read and write their exact byte width (i1/i8 = 1, i16 = 2,
  i32 = 4, i64 = 8, ptr = 4), little-endian;
- consequently the *same bytes* are observable through a typed field access, a
  nested GEP chain, and an `unsigned char *` byte view of the aggregate.

This is explicitly **not** the old one-cell-per-address model: a cell-per-address
heap would make `(char *)&bg` and `bg.a` disagree by construction.

## 7. Scope boundaries (future work)

Deliberately **not** implemented in v0.5. Each is a separate milestone, not a
half-done aggregate feature:

- **Aggregate-by-value ABI.** Passing or returning a struct by value, and the
  spilling/coercion rules that go with it. This is the reason `IrType` has no
  aggregate variant: the value representation is a prerequisite, and inventing
  one here would fix the ABI by accident.
- **Flexible array members / zero-sized types.** `struct S { int n; int a[]; }`
  and explicit `[0 x T]` are rejected with `LayoutError::ZeroSized` rather than
  given a made-up size. Supporting them means deciding what a trailing
  unbounded array *is* at the memory-model level.
- **Vector types** (`<4 x i32>`) — parser-level gap, unrelated to memory layout.
- **Atomic operations** — parser-level gap (`atomicrmw`).
- **Exception handling** — no landing pads / unwind tables.
- **Function pointers** — the SAIR/ISA call ABI requires a statically-named
  callee; see [`FUNCTION_POINTERS.md`](./FUNCTION_POINTERS.md).
- **Packed / explicitly-aligned structs** (`<{ … }>`, `align N` attributes).
  The `align N` attributes clang emits are never semantically observable in this
  subset and are ignored; a genuinely packed struct would need its own layout
  rule and is not supported.

## 8. Verification

| Surface | Where |
|---|---|
| Layout rules (padding, nested, arrays, nested arrays, array-of-struct, struct-of-array, stride, determinism, zero-sizing) | `crates/scratcharch-target/src/layout.rs` unit tests |
| Static-data byte images from aggregate initializers (offsets, padding, pointer stride, zeroinitializer, determinism) | `crates/scratcharch-llvm/tests/aggregate_tests.rs` |
| Aggregate value-type and zero-sized-type rejection diagnostics | `crates/scratcharch-llvm/tests/reject_tests.rs` |
| Real-clang fixtures, recompiled every run | `crates/scratcharch-llvm/tests/corpus_clang_tests.rs` (fresh clang) |
| Five-surface record (parser / SAIR / interpreter / VM / Scratch) + native differential | `crates/scratcharch-pipeline/tests/llvm_corpus_surfaces.rs` |
| Corpus expectations and gate | `tests/corpus/llvm/manifest.json`, `results/latest.json` |

The v0.5 aggregate fixtures are `aggstruct`, `aggarray`, `aggglobal`,
`aggmatrix`, `aggnested` and `aggbytes` in `tests/c_programs/`, plus the
rehabilitated `tests/corpus/llvm/fixtures/global-agg`. Each is native-exact,
interpreter-exact and VM-exact, and constructs on Scratch.
