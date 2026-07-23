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

## Heap abstraction

ScratchGraph uses one stage-owned list as the heap:

```text
__scratcharch_heap
```

- **Pointer**: a 0-based index into `__scratcharch_heap`.
- **Cell**: one Scratch list element. One cell stores one scalar value.
- **Allocation**: append zero-valued cells to the list and return the old list
  length as the pointer.
- **Load**: read the list element at `pointer + 1` (Scratch lists are
  1-indexed).
- **Store**: write the list element at `pointer + 1`.
- **Pointer arithmetic (`gep`)**: compute a new pointer by adding a byte offset
  to the base pointer.

The 0-based/1-based conversion happens at the exporter boundary. ScratchGraph
itself treats pointers as 0-based indices so that pointer arithmetic matches
SAIR semantics.

## Mapping from SAIR memory instructions

| SAIR instruction             | ScratchGraph representation                                              |
| ---------------------------- | ------------------------------------------------------------------------ |
| `alloca T, N`                | 1. Set result variable to `length of __scratcharch_heap`.                |
|                              | 2. Repeat `size_in_bytes(T) * N` times: append `0` to the heap list.     |
| `load T, addr`               | Set result variable to `HeapLoad { addr }`, which reads the heap at      |
|                              | `addr + 1` at export time.                                               |
| `store value, addr`          | `SetListItem { list: "__scratcharch_heap", index: addr + 1, value }`.    |
| `gep elem_ty, base, indices` | `HeapIndex { base, offset }` where offset is the sum of index * element  |
|                              | size for each index.                                                     |

### Struct-field offsets

`gep` supports both dynamic array indices and static struct-field indices.
Struct-field offsets are computed from `IrType::size_in_bytes()` of the
aggregate element type. Padding between fields is not modeled in v0.2; the
offset for field `i` is `i * elem_ty.size_in_bytes()`.

## Lowering to Scratch lists

The Scratch 3 JSON exporter expands the semantic operations as follows:

- `HeapAlloc { result, size }`:

  ```scratchblocks
  set [result v] to (length of [__scratcharch_heap v])
  repeat (size)
      add (0) to [__scratcharch_heap v]
  end
  ```

- `HeapLoad { addr }`:

  ```scratchblocks
  (item (addr + 1) of [__scratcharch_heap v])
  ```

- `Store`:

  ```scratchblocks
  replace item (addr + 1) of [__scratcharch_heap v] with (value)
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
- There is no `free` or `realloc` in v0.2. Allocations are monotonically
  appended to the heap list.
- The heap is not reset between green-flag runs unless a script explicitly
  deletes all items.

## Limitations

The v0.2 memory model intentionally does not implement:

- Alignment and padding.
- `free` / `realloc`.
- Multi-cell values (for example `i64` on a 48-bit cell profile).
- Separate heaps per sprite or per thread.
- Stack-allocated memory distinct from the heap list.

These are recognized gaps that future milestones may address without changing
the architecture of the abstraction.

## Relation to `docs/specification/MEMORY.md`

`docs/specification/MEMORY.md` defines the architecture-level memory model for
the ISA and VM: flat byte-addressable memory, type sizes, alignment, and
undefined behavior. This document defines the Scratch-specific projection of
that model onto Scratch lists. The two specifications are independent; SAIR
remains agnostic to how a given backend materializes memory.
