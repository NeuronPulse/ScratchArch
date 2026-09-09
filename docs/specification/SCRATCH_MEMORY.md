# ScratchGraph Memory Specification

This document defines the ScratchGraph memory abstraction used by the
ScratchArch Scratch backend. It sits between SAIR's generic memory
instructions and concrete Scratch storage (lists and variables).

## Purpose

Scratch has no native pointer or heap. It provides:

- **Variables**: single scalar values, scoped to a target.
- **Lists**: one-dimensional, dynamically-sized, 1-indexed sequences of values.

The ScratchGraph memory abstraction maps SAIR pointer operations onto a single
logical heap backed by a Scratch list. This keeps the backend semantic and
independent of any particular Scratch serialization format.

## Byte-exact heap abstraction

ScratchGraph uses one stage-owned list as the heap:

```text
__scratcharch_heap
```

The heap is **byte-addressable**: one list element stores one addressable
**byte** as an f64 number. This is the same byte-addressable model as the SAIR
interpreter and ISA VM (`docs/specification/MEMORY.md`) projected onto Scratch
lists — a pointer is a 0-based **byte offset**, and every width-exact load and
store reads or writes exactly `size_in_bytes(ty)` bytes.

```text
heap item  ─  item(1)  item(2)  item(3)  item(4)  item(5)  item(6) ...
address    ─   0        1        2        3        4        5      ...
```

- **Pointer**: a 0-based byte offset into `__scratcharch_heap`. The list is
  1-indexed, so byte at address `a` is list item `a + 1` (the 0-based/1-based
  conversion happens at the exporter boundary).
- **Allocation** (`alloca T, N`): set the result to the current list length
  (the next free byte offset), then append `size_in_bytes(T) * N` zero bytes.
  The heap therefore grows monotonically; no `free` / `realloc` exists.
- **Load `T`**: read `size_in_bytes(T)` consecutive items, little-endian.
- **Store `T`**: write `size_in_bytes(T)` consecutive items, little-endian.
  A store touches only its own bytes — neighboring bytes are preserved.
- **Pointer arithmetic (`gep`)**: compute a new pointer by adding a byte offset
  to the base pointer.

### Byte widths

| Type        | Bytes | Little-endian byte count                              |
| ----------- | ----- | ----------------------------------------------------- |
| `i1`        | 1     | one byte holding 0 or 1                                |
| `i8`        | 1     | one byte                                               |
| `i16`       | 2     | `b0 + 256·b1`                                          |
| `i32`       | 4     | `b0 + 256·b1 + 65536·b2 + 16777216·b3`                 |
| `i64`       | 8     | `b0 + 256·b1 + … + 256⁷·b7`                            |
| `ptr`       | 4     | four bytes (a 32-bit offset)                           |

Widths come from `IrType::size_in_bytes()`; the Scratch backend never
re-derives a size or offset that SAIR already knows.

## Value convention: mathematical signed integers

The SAIR/VM carry integers as raw bit patterns (a negative `i32` is the
two's-complement pattern `0xFFFF_FFFB`). The Scratch backend instead works with
the **mathematical signed value** (`-5`, never `0xFFFF_FFFB`) because Scratch
arithmetic does not wrap mod 2^N and its operator palette is signed by nature.
Two rules keep the two representations consistent:

- **Store reduces mod the width first.** A store computes
  `m = value mod 2^(8·N)` (Scratch `operator_mod` is floor-mod, so `m` is the
  non-negative raw pattern). The same bytes result whether the carried f64 was
  the raw pattern or the mathematical value, because the two are congruent
  mod `2^(8·N)`.
- **Load reinterprets to signed.** A load recomposes the raw pattern `m` and,
  for `i8`/`i16`/`i32`, reinterprets the high bit: `m − 2^(8·N)` when
  `m ≥ 2^(8·N−1)`, else `m`. `i64` and `ptr` loads return the raw non-negative
  pattern (see the bounds below).

Signed comparisons are then exact as-is (the LLVM translator's signed icmp
expansion degrades to a plain Scratch comparison on mathematical values), and
arithmetic in the f64-exact range matches the interpreter/VM. See
`docs/design/SCRATCH_NUMERIC_MODEL.md` §2 for the exact classification.

### Byte-split and byte-combine formulas

Split (store), for a value already reduced to `m ∈ [0, 2^(8·N))`:

```text
b0 = m mod 256
b1 = ((m − b0) / 256) mod 256
b2 = ((m − b0 − 256·b1) / 65536) mod 256
…
```

`(m − b0 − … )` is always an exact multiple of the power of two, so Scratch
`operator_divide` is exact — no `floor` (an `operator_mathop`) is needed.

Combine (load), for bytes `b0 … b_{N−1}` at `addr`:

```text
item = (addr + 1 + k) of [__scratcharch_heap v]        // byte b_k
m    = b0 + 256·b1 + 65536·b2 + … + 256^(N−1)·b_{N−1}
```

All terms stay below 2^53 for `i64` in the exact subset, so every product and
sum is an exact f64.

## Mapping from SAIR memory instructions

| SAIR instruction             | ScratchGraph representation                                              |
| ---------------------------- | ------------------------------------------------------------------------ |
| `alloca T, N`                | 1. Set result variable to `length of __scratcharch_heap` (next byte).    |
|                              | 2. Repeat `size_in_bytes(T) * N` times: append `0` to the heap list.     |
| `load T, addr`               | Set result variable to the byte-combine of `size_in_bytes(T)` items at  |
|                              | `addr + 1 … addr + size`, followed by the signed reinterpretation where |
|                              | applicable.                                                              |
| `store value, addr`          | For each of `size_in_bytes(T)` bytes: `SetListItem` at `addr + 1 + k`.  |
| `gep elem_ty, base, indices` | `HeapIndex { base, offset }` where offset is the sum of index * element  |
|                              | size for each index (byte offsets).                                      |

`i1` loads produce a Scratch boolean (`byte ≠ 0`); `i1` stores reduce the value
`mod 2`. Both coerce to `0`/`1` in arithmetic.

## Static data seeding

Static globals are serialized into a byte-exact image
(`scratcharch_ir::module::StaticData.image`, addressed from
`STATIC_DATA_BASE = 8`, with addresses `[0, STATIC_DATA_BASE)` left as zero
padding). The entry ("when green flag") script seeds the heap before the first
call:

1. Append `STATIC_DATA_BASE + image.len()` zero bytes (so the list has one item
   per seeded byte, 1-indexed).
2. For each image byte `i`, `replace item (STATIC_DATA_BASE + 1 + i)` with the
   byte value.

This mirrors the interpreter's `memory[STATIC_DATA_BASE .. + len] = image`, so
a global at address `STATIC_DATA_BASE + off` reads the same bytes on every
backend. The first `alloca` returns the seeded length, so the heap never
overlaps the static region.

## Lowering to Scratch lists

The Scratch 3 JSON exporter expands the semantic operations as follows:

- `HeapAlloc { result, size }`:

  ```scratchblocks
  set [result v] to (length of [__scratcharch_heap v])
  repeat (size)
      add (0) to [__scratcharch_heap v]
  end
  ```

- `Load i32` (4 bytes, signed):

  ```scratchblocks
  ((b0) + ((256) * ((b1) + ((65536) * ((b2) + ((16777216) * (b3)))))))
  // then: (m) - ((4294967296) * <(m) > (2147483647)>) for the signed form
  ```

- `Store i32` (4 bytes):

  ```scratchblocks
  replace item ((addr) + (1))        of [__scratcharch_heap v] with (m mod 256)
  replace item ((addr) + (2))        of [__scratcharch_heap v] with (((m - (m mod 256)) / 256) mod 256)
  replace item ((addr) + (3))        of [__scratcharch_heap v] with ((…) mod 256)
  replace item ((addr) + (4))        of [__scratcharch_heap v] with ((…) mod 256)
  ```

- `HeapIndex { base, offset }`:

  ```scratchblocks
  (base + offset)
  ```

Future exporters (for example `sb3` or `scratchblocks`) can use the same
semantic operations without re-implementing the SAIR-to-memory mapping.

## Scope and lifetime

- `__scratcharch_heap` is declared on the stage and is therefore global to the
  project.
- There is no `free` or `realloc`. Allocations are monotonically appended to
  the heap list.
- The heap is not reset between green-flag runs unless a script explicitly
  deletes all items.

## Validation boundary

Construction of a `Project` proves **lowerability**, not execution. The Scratch
backend has no standalone executor, so verification of the memory model is
formula-level and structural:

- **Construction** — every SAIR construct lowers to a Scratch `Project` or is
  rejected with a named diagnostic. `Ok(project)` does *not* claim the program
  computes correct values at run time.
- **Formula/structural verification** — unit tests assert the emitted
  expressions: byte-split/recombine exactness, little-endian order, width
  preservation (a store touches exactly its own bytes), signed reinterpretation
  at the width boundary, static-data seeding layout, and the runtime heap ABI
  (`item = addr + 1`). These prove the lowering is faithful to the SAIR
  semantics *given* the documented `SAFE_SUBSET` bounds.
- **Execution** — a future standalone ScratchGraph reference executor (a
  separate milestone) will evaluate whole projects and close the remaining gap
  between formula verification and execution. Until then, execution semantics
  on real Scratch is *not* claimed beyond the differential construction and
  formula evidence.

## SAFE_SUBSET bounds (documented, enforced at the doc level)

The following are the exactness bounds of the byte-exact memory model. The
lowerer never silently approximates; programs outside a bound are outside the
supported subset (classified per `docs/design/SCRATCH_NUMERIC_MODEL.md`):

- `i64` stored/loaded through the heap must be in `[0, 2^53)` — every value
  must be exactly representable as an f64. Negative or ≥ 2^53 `i64` in memory
  is unrepresentable.
- `i8`/`i16`/`i32` loads reinterpret to the mathematical signed value. A
  program that consumes the **raw unsigned** bit pattern of a loaded value at
  or above `2^(N−1)` (e.g. `icmp uge i8 %load, 200`, or storing `255` and
  expecting the numeric `255` back) is outside the subset.
- Arithmetic on loaded values must stay in the f64-exact integer range without
  relying on mod-2^N wrapping (Scratch does not wrap).

## Relation to `docs/specification/MEMORY.md`

`docs/specification/MEMORY.md` defines the architecture-level memory model for
the ISA and VM: flat byte-addressable memory, type sizes, alignment, and
undefined behavior. This document defines the Scratch-specific projection of
that model onto Scratch lists: byte-exact, little-endian, width-aware, with the
mathematical-signed value convention. The two specifications are independent;
SAIR remains agnostic to how a given backend materializes memory.
