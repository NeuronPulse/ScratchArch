# Aggregate ABI and Value Passing

> Milestone: **LLVM Compatibility v0.6**. Normative status matrix:
> [`docs/specification/LLVM_COMPATIBILITY.md`](../specification/LLVM_COMPATIBILITY.md).
> The frozen calling convention this builds on:
> [`docs/specification/ABI.md`](../specification/ABI.md).
> Layout authority: [`AGGREGATE_DATA_MODEL.md`](./AGGREGATE_DATA_MODEL.md) (v0.5)
> and `scratcharch-target::layout`. LLVM→SAIR mapping:
> [`LLVM_TRANSLATION.md`](./LLVM_TRANSLATION.md).

This document defines how an LLVM aggregate (`[N x T]` array, `{ T1, T2, … }`
struct) crosses a **function boundary** — as a parameter and as a return value —
across LLVM IR → SAIR → the SAIR interpreter, the ISA VM, and the ScratchGraph
backend.

The one-sentence version: **an aggregate crossing a call boundary is a pointer
to byte-exact memory, moved by the existing machinery.** v0.5 established that an
aggregate is a layout concern, not a value representation; v0.6 realizes
parameter and return passing on top of exactly that, and adds **no** aggregate
instruction, **no** aggregate `IrType`, and **no** second calling convention.

---

## 1. What the ABI has to match

The frontend consumes real clang `-O0` output for `x86_64-pc-linux-gnu`, so the
aggregate ABI is not ours to invent: it is whatever clang already emitted into
the IR. Empirically, at `-O0`, clang's classification of a record splits into
four shapes, and each one reaches the translator as a different piece of IR:

| Record extent | SysV class | What clang emits | How it crosses |
|---|---|---|---|
| ≤ 8 bytes, INTEGER | coerced to a scalar | `define i64 @f(i64 %0)` | an ordinary scalar; no aggregate on the boundary at all |
| 9–16 bytes | coerced to a literal aggregate, returned **in registers** | `define { i64, i32 } @f(…)`, `ret { i64, i32 } %v`, `load`/`store { i64, i32 }` | [`§3`](#3-the-two-crossing-conventions) — translator-synthesized result pointer |
| > 16 bytes | MEMORY | `ptr byval(%struct.Big) align 8 %0` (parameter), `ptr sret(%struct.Big) %0` with a `void` return | [`§3`](#3-the-two-crossing-conventions) — pointer parameter, passed through |
| ≤ 16 bytes, all-scalar fields (a *parameter*) | decomposed | `define i32 @f(i32 %0, i32 %1)` | an ordinary scalar list; layout identity is clang's problem, not ours |

Two consequences set the whole design:

1. **Clang does the decomposition.** We never decompose a record into an
   arbitrary scalar list ourselves — the coerced-scalar and decomposed forms
   arrive pre-decomposed, and the annotated-pointer forms arrive as pointers. The
   case we must handle is the *aggregate-typed* form (`{ i64, i32 }`, or a named
   `%struct.S`), which clang returns in registers when the record does not fit
   the register rules but is too small for memory.
2. **Clang's `sret`/`byval` are already explicit parameters.** `sret(%T) %0` is
   an ordinary `ptr` parameter with an attribute; the callee writes through it
   and the caller allocates it. Nothing about it is hidden at the LLVM level, so
   the positional-cell ABI carries it unmodified.

## 2. Aggregate value representation: temporary slots

An aggregate never becomes a SAIR value. `IrType` stays scalar and `Copy`, and
`Instruction` gains no variant. An aggregate-typed SSA value in the IR is backed
by a **compiler-managed temporary slot** — an `alloca i8, <DataLayout size>` in
the producing function's frame — and the "value" is the slot's address:

```rust
struct AggregateSlot {
    ty: LlvmType,   // the logical aggregate type this slot holds
    size: u32,      // DataLayout::size — trailing padding included
    addr: ValueId,
}
```

`Ty` is load-bearing, not decoration: `resolve_aggregate_slot` checks it against
the type the consuming instruction declares, on every single use. Field offsets,
the copy extent and the leaf types are all derived from `DataLayout` **for that
type**, so a slot read as the wrong type would silently reinterpret the bytes. It
is reported instead:

```
aggregate value of type %T is used as %U
%x is used where an aggregate value is required, but it does not hold an
  aggregate (only load, call, insertvalue, extractvalue and byval results do)
an aggregate operand must be an aggregate-typed SSA value, got Const(…)
struct value type %T has no SAIR value representation; …
```

Every aggregate move is `emit_byte_copy(dst, src, len)` — load all `len` source
bytes into SSA values, then store each into `dst + i`. All loads precede all
stores, so overlapping regions behave exactly like a temporary buffer. The length
is always `DataLayout::size`, so padding moves with the value and is never read
as data.

`extractvalue`/`insertvalue` index paths fold to a byte offset and a leaf type
through `aggregate_leaf_offset`, which walks `field_offset_of_llvm` /
`array_stride_of_llvm` one index at a time. That is the *same* function the GEP
path uses, so an `extractvalue` and a `getelementptr` over an identical path
cannot disagree about the offset. Each index must be a constant; a dynamic field
index has no per-field dispatch in a flat memory model and is rejected.

## 3. The two crossing conventions

### 3.1 clang-emitted `sret` / `byval`: pass the pointer through

When clang emits the memory-class form, the pointer is an ordinary explicit
parameter and the translator treats it as one — the position is preserved for
free, no slot is introduced, and no second pointer is added:

```
define void @g(ptr sret(%struct.Big) %0, i32 %1)   →   func @g -> void
                                                        param ptr %0
                                                        param i32 %1
```

- **`sret`.** The callee copies the returned value's slot into the result pointer
  (`emit_byte_copy(sret_ptr, slot.addr, size)`) and returns void. The *caller*
  owns that storage, exactly as in the native ABI.
- **`byval`.** See [§4](#4-byval-the-callee-owns-a-copy). The parameter is a
  pointer, but the callee may write through it, so the prologue takes a private
  copy first.

Because `FnState::sret` is only populated for the register-returned case (§3.2),
a clang-emitted `sret` function never gains a second hidden pointer.

### 3.2 Register-returned aggregates: a synthesized result pointer

For the 9–16-byte case clang returns a literal aggregate type in registers and
there is no pointer in the IR at all. The translator supplies one, following the
frozen convention in [ABI.md §5.3](../specification/ABI.md):

- the SAIR function returns `IrType::Void` and gains one extra `IrType::Pointer`
  parameter, **appended after every explicit parameter**;
- it is named `!sret`, a spelling no `%N` source name can collide with;
- `ret <aggregate> %v` becomes a byte copy of the value's slot into that pointer,
  then `ret`.

```
define { i64, i32 } @make(i32 %x)   →   func @make -> void
                                          param i32 %0
                                          param ptr %1      ; the result pointer
```

Each call site allocates its **own** fresh slot and passes it last:

```
%a = call { i64, i32 } @make(i32 10)   →   %slot = alloca i8, 16
                                            call void @make(%0, %slot)
                                            ; %a is now %slot
```

Appending is safe because both the definition and every call site are produced by
this same translator, so the two sides of the convention are always in sync. A
call to an aggregate-returning **declaration** has no such guarantee — an
external function cannot be assumed to honour a convention it has never seen — so
it is rejected rather than guessed at:

```
declaration of @external_make returns the aggregate { i32, i32 } by value: the
  aggregate ABI is realized with a hidden result pointer, which an external
  function cannot honour
```

## 4. `byval`: the callee owns a copy

`byval` means the callee may treat the pointer as its own object. clang leaves
the copy to the backend, so the IR contains none — and a naive pass-through
lowering therefore lets the callee clobber the caller's object (observably: a
fixture that keeps 100 natively returns 198). The translator materializes the
copy in the callee prologue:

```
define i32 @f(ptr byval(%Big) align 8 %0)   →   func @f -> i32
                                                  param ptr %0
                                                  %1 = alloca i8, 20
                                                  ; 20 load i8 %0+k / store i8 %1+k
                                                  ; %0 now *denotes* %1
```

Two details make it correct:

- The copy is emitted **after** the entry block exists and **before** any
  translated instruction, so it is the first thing in the prologue.
- The parameter *name* is rebound to the copy's address in `FnState::values`.
  Every `getelementptr %Big, ptr %0, …` in the body therefore addresses the
  callee's own copy, while the caller's object is untouched. The parameter id
  itself is used only as the copy's source.

## 5. Decomposition rules

### Parameter decomposition

| IR form | SAIR form |
|---|---|
| `i64 %0` (a coerced small record) | one `param i64` |
| `i32 %0, i32 %1` (a decomposed record) | two scalar params, in declaration order |
| `ptr %0` (a `byval` record) | one `param ptr`, plus the prologue copy that rebinds `%0` |
| `ptr sret(%T) %0` (a memory-class result) | one ordinary `param ptr`, written through directly |
| an aggregate-typed parameter | *does not occur* — clang never passes a record by value without coercion, `byval` or decomposition |

### Return decomposition

| IR form | SAIR form |
|---|---|
| a scalar | that scalar as the SAIR return type |
| `void` with `ptr sret(%T) %0` | `-> void`; the result is written through the explicit pointer |
| an aggregate type (`{ … }` or `%struct.S`) | `-> void`; one appended `param ptr`, the result written through it |

A named aggregate return type (`define %Pair @f()` … `ret %Pair %v`) is the same
aggregate as its literal spelling and goes through the same path. (The parser
disambiguates the `%Pair` in `ret %Pair %v` from an SSA value by looking at the
following token, so the header and the body agree.)

### Cell ordering

Ordering is **positional and untouched**. One cell per parameter; no aggregate is
split across cells here (an aggregate operand is one pointer cell, and a coerced
or decomposed record is already several *ordinary* scalar cells). clang's own
`sret`/`byval` pointers keep the position clang gave them. The synthesized result
pointer is always **last**, after every explicit parameter — never first, never
interleaved. Aggregates and scalars may therefore interleave freely in one
signature, and both directions stay symmetric.

### Nested aggregates

A nested record decomposes exactly as its flattened extent demands: the extent
decides the class, and the layout inside it is `DataLayout`'s business. `struct
Wide { struct Pair p; struct Pair q; int k; }` is 20 bytes and so is `byval`;
`{ Pair p; int k; }` is 12 bytes and so returns in registers as `{ i64, i32 }`.
Nested *access* (rather than passing) already worked in v0.5, and `extractvalue`
of a nested leaf reuses it: a nested aggregate leaf gets a fresh slot and a copy
of the sub-aggregate's size, at the offset the index path folds to.

### Alignment and padding

- The slot extent is `DataLayout::size`, which **includes trailing padding**, and
  every copy covers the full extent — so padding bytes travel with the value and
  are never interpreted as data.
- A record with an `i64` member carries that member's 8 bytes as the two 32-bit
  limbs of the same memory layout; a `long long v; int k;` record is 16 bytes
  with 4 bytes of trailing padding at offset 12, and copying 16 bytes keeps both
  limbs and the padding intact.
- A pointer leaf *reserves* its 8-byte source stride in the layout (so field
  offsets match clang) but only the low `pointer_size_bytes` carry the SAIR
  address; the rest is zero padding that is never interpreted. A pointer member
  copies an *address*, never a pointee.
- Alignment is not a calling-convention property here: a slot is byte-addressed
  and the ISA has no alignment requirement, so an `align N` attribute is
  informational and ignored (as in v0.5).

### Empty and zero-sized aggregates

An aggregate that occupies no storage (`{}`, `[0 x T]`, the flexible-array-member
idiom) is refused by `DataLayout` (`LayoutError::ZeroSized`) *before* any ABI
question arises — it has no layout, so it has no size to copy, no offsets to
compute, and no ABI representation. Silently giving it an extent would move every
following field. Empty structs are therefore an explicit boundary (§9), not a
special case in the ABI.

## 6. Temporary-slot lifecycle

The rule the whole mechanism rests on:

> **Every aggregate SSA value is owned by one function activation, is created
> exactly once by the operation that produces it, and is never freed or shared.
> Distinct aggregate values never share a slot.**

- **Ownership.** A slot is allocated in the frame of the function that produces
  the value — a `byval` copy in the callee, a call result and an `insertvalue`
  result in the caller, a `load` result in whoever loaded it. No slot is ever
  passed *as* storage across a boundary except the explicit `sret` pointer the
  ABI defines.
- **Creation.** Exactly one slot per producing operation: `load`, `call` (one per
  call site, never shared between sites), `insertvalue`, `extractvalue` of a
  nested aggregate leaf, and the `byval` prologue copy.
- **When it ceases to be observable.** Never during the activation: a slot is not
  freed and its address does not escape as a value (an aggregate operand is only
  ever consumed by an instruction that understands it as a slot). It is frame
  storage, so it dies with the activation, and the next call's activation begins
  with a fresh frame — a stale slot address can never be observed.
- **Aliasing.** Slots never alias each other. This is what makes the two
  discriminating cases work: `insertvalue` is a *functional* update, so it
  allocates a fresh slot, copies the whole operand and patches one leaf — the
  operand keeps its old bytes; and two calls to the same aggregate-returning
  function get two independent result slots, so the second cannot overwrite the
  first. The one deliberate aliasing is the `byval` pointer *initially* pointing
  at the caller's object — which is precisely why the prologue copy exists.
- **Copy or reference?** `load`, `call`, `insertvalue`, a nested `extractvalue`
  and a `byval` prologue all produce **copies** — each owns fresh bytes.
  `extractvalue` of a *scalar* leaf loads the scalar (a value copy of the leaf,
  not a reference into the slot). Nothing in this design hands out a reference
  into a slot as a SAIR value; a slot address is only ever an operand to another
  aggregate operation.

**This is a translator-internal discipline, not public ISA semantics.** Slots are
ordinary `alloca`s and ordinary `load`/`store`s; they introduce no instruction,
no addressing mode and no observable rule beyond what the existing memory model
already specifies. A backend sees only byte loads and stores at computed offsets.

## 7. VM realization

**Nothing new** — and that is the design claim, not an omission. There is no
aggregate instruction in the ISA and none was added. The VM executes aggregates
through the primitives v0.5 already established:

- every aggregate move is the existing width-exact memory path (`i8` byte
  loads/stores, `i32`/`ptr` word accesses, `i64` as two 32-bit limbs);
- a scalar leaf is the existing scalar load/store at a `DataLayout` byte offset;
- a `byval` copy and an `sret` write are plain byte copies;
- the `!sret` parameter is an ordinary pointer cell, so it rides the existing
  multi-cell/cell-ordered argument ABI unchanged.

The one thing the VM must *not* do — interpret a padding byte as a value — cannot
happen, because nothing ever loads a padding byte: copies move bytes within
memory, and leaf accesses name a live field.

This is why no aggregate fixture can be VM-rejected for being an aggregate:
the whole class lowers to primitives the VM already had.

## 8. Interpreter realization

The SAIR interpreter is the semantic reference, and it needed no aggregate
concept either: it executes the same `alloca` / byte `load` / byte `store`
sequence the VM does, over the same `DataLayout`-computed offsets. There is **one**
logical ABI, not two — native clang, the interpreter and the VM all agree
bit-for-bit because there is a single byte-level lowering and the three engines
share the memory model.

Consequently an *interpreter-only* aggregate case cannot exist either: a module
that translates at all runs on both engines.

## 9. ScratchGraph realization

Aggregates cross into Scratch through the same **byte-exact heap**
(`__scratcharch_heap`, one list item per byte; see
[`SCRATCH_MEMORY.md`](../specification/SCRATCH_MEMORY.md)) that v0.5 introduced
for aggregate *memory*:

- a slot is heap storage; a byte copy is a Scratch list-item copy;
- a scalar leaf is a read/write of its exact byte width at a heap index;
- a `byval` prologue copy and an `sret` write are the same byte copies;
- the `!sret` pointer is an ordinary pointer value, so the frame ABI carries it
  without change.

No second aggregate ABI is invented for Scratch and no new Scratch construct was
needed: all eight aggregate-ABI fixtures construct on Scratch and clear the
Scratch stage of the compatibility dashboard. Where a case *could* not be
represented (the `i24` boundary, §10), the frontend produces an explicit
`ScratchBackendUnsupported`-class diagnostic naming the construct — never a
silent flatten or a wrong project.

## 10. Explicit boundaries

Deliberately **not** implemented. Each is a named boundary with a diagnostic or a
documented gap, never a silent approximation.

| Boundary | Behavior |
|---|---|
| **Non-power-of-two integer widths** (`i24`, `i40`, `i48`) | Rejected by name: `integer width i24 is not supported: SAIR represents i1, i8, i16, i32 and i64 only (clang coerces a struct whose fields total an odd number of bytes — e.g. three `char`s — to a width like i24)`. Clang reaches these through aggregate coercion, so this is a genuine aggregate-ABI boundary; rounding to `i32` would silently shift every later argument. Fixture: `abi-odd-width`. |
| **Aggregate-returning declarations** | Rejected by name (§3.2): a hidden result pointer is a convention an external function cannot be assumed to honour. |
| **Aggregate varargs** | Not implemented. A variadic call's aggregate operands would need the SysV register/stack spill rules, which are a different ABI from the positional-cell form. |
| **Exceptional / non-default calling conventions** | Not implemented (`landingpad`, unwind tables, `fastcc`/`coldcc`/`swiftcc` semantics). The default C convention only. |
| **Packed aggregates** (`<{ … }>`) | Not implemented. A packed record needs its own layout rule (no inter-field alignment), which would change every offset; the `align N` attributes clang emits in the default convention are ignored instead. |
| **Zero-sized / empty aggregates** (`{}`, `[0 x T]`) | Rejected by `DataLayout` before reaching the ABI (§5). |
| **Vector aggregate ABI** | Not implemented — vector *types* are still a parser-level gap. |
| **Atomic aggregate ABI** | Not implemented — `atomicrmw` is still a parser-level gap. |

## 11. Verification

| Surface | Where |
|---|---|
| Lowering shape: byte-exact copies, slot extents, fresh-slot-per-call, byval prologue copy, appended result pointer, `extractvalue`/`insertvalue` offsets, determinism | `crates/scratcharch-llvm/tests/aggregate_abi_tests.rs` |
| Boundary diagnostics (odd width, aggregate-returning declaration, aggregate type mismatch, aggregate in a scalar position) | `crates/scratcharch-llvm/tests/reject_tests.rs` |
| Real-clang fixtures, recompiled every run | `crates/scratcharch-llvm/tests/corpus_clang_tests.rs` |
| Interpreter ↔ VM agreement per fixture | `crates/scratcharch-driver/tests/vm_backend_tests.rs` |
| Five-surface record (parser / SAIR / interpreter / VM / Scratch) + native differential | `crates/scratcharch-pipeline/tests/llvm_corpus_surfaces.rs` |
| Corpus expectations, gate and regression oracle | `tests/corpus/llvm/manifest.json`, `results/latest.json` |

The v0.6 aggregate-ABI fixtures are `abi-struct-param`, `abi-struct-return`,
`abi-nested-param`, `abi-nested-return`, `abi-i64-field`, `abi-ptr-field`,
`abi-multi-agg` and `abi-mixed-args` in `tests/c_programs/`, plus the boundary
fixture `abi-odd-width` in `tests/corpus/llvm/fixtures/`. Each of the eight is
native-exact, interpreter-exact and VM-exact, and constructs on Scratch; between
them they cover both SysV classes, nesting, `i64` and pointer members, several
aggregates in one call, and scalar/aggregate interleaving.
