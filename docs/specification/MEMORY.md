# ScratchArch Memory Model

> Specification version: **v0.1 (draft for freezing)**
> Status: normative for the `sa48` reference profile; parametric for all profiles.
> Companion documents: [`ISA.md`](./ISA.md), [`ABI.md`](./ABI.md).

---

## 1. Motivation

The memory model defines how the ScratchArch Abstract Machine (SAM) stores and
retrieves data. It is the bridge between the ISA's register values and the
persistent state of a computation.

The predecessor project, `llvm2scratch`, proved that a flat address space
simulated on a single Scratch list can support LLVM's full range of memory
operations: `alloca`, `load`, `store`, `getelementptr`, and `memcpy`. The
prototype's key insight — **a flat byte-addressable address space is sufficient
even on a runtime that has no native memory** — is the foundation of this
document.

ScratchArch discards the prototype's Scratch-specific details (the `!mem`
list, 1-indexed addressing, optional word-addressing for size reduction) and
formalizes a clean, runtime-independent memory model.

### What problem the memory model solves

| Problem from the prototype | Memory-model resolution |
|---|---|
| "Memory" was a Scratch list (`!mem`), a storage artifact | Memory is an abstract flat address space; how it is realized is a runtime concern |
| Accurate byte spacing was optional and incomplete | Byte addressing is the **only** model; all types occupy their natural byte size |
| Address 0 was safe because Scratch lists are 1-indexed | Address 0 is the null pointer; accessing it is undefined behavior |
| `i8_gep_div` controlled byte vs 4-byte word addressing | Only byte addressing exists; the `i8_gep_div` optimization was a Scratch-specific hack |
| Stack pointer and heap pointer were Scratch variables (`!stack pointer`, `!heap pointer`) | The stack pointer is a well-defined ABI register (`!sp`); heap management is a runtime library concern, not an architectural one |
| Function pointer addresses existed beyond memory and were never loaded | Formalized as a reserved virtual address region (see §3.5) |

---

## 2. Address space model

### 2.1 Flat linear address space

The SAM has a single, flat, byte-addressed address space. Every memory
operation works with a **byte address** — an unsigned integer of width `Wptr`
bits.

| Profile | `Wptr` | Addressable range | Addressable unit |
|---|---|---|---|
| **`sa48`** (reference) | 32 | 0 … 2³²−1 (4 GiB) | 1 byte (8 bits) |
| `sa64` (native) | 64 | 0 … 2⁶⁴−1 (16 EiB) | 1 byte (8 bits) |

> **Why byte addressing as the universal standard?** LLVM, C, and most
> intermediate representations assume byte-addressed memory. The prototype's
> word-addressing mode (`i8_gep_div`) was an optimization to reduce Scratch
> list size, not a fundamental model. By fixing byte addressing, the
> architecture guarantees that `sizeof(T)`, pointer arithmetic, and `memcpy`
> behave identically to LLVM's expectations. A runtime that prefers larger
> addressing units may coalesce bytes internally as long as the observable
> semantics match.

### 2.2 Address spaces

v0.1 defines exactly one address space **(address space 0)**. LLVM's named
address spaces are reserved for future extension.

### 2.3 Address arithmetic

Addresses are unsigned `Wptr`-bit integers. All address arithmetic wraps modulo
`2^Wptr`. This matches LLVM's pointer arithmetic semantics and the prototype's
32-bit wrapping behavior.

### 2.4 Null pointer

The value `0` is the **null pointer**. Any attempt to `load`, `store`, or
`getelementptr` that produces a null pointer and then accesses it has
**undefined behavior**. Implementations may trap, return a distinguished value,
or silently produce wrong results — the architecture does not prescribe the
behavior.

> **Why null is undefined, not a no-op.** The prototype's null address (0)
> happened to be safe because Scratch lists are 1-indexed; accessing element 0
> returned `""`. This is a Scratch-specific coincidence. A native
> implementation cannot safely read or write address 0. Making it undefined
> aligns with C and LLVM semantics.

### 2.5 Memory ordering

v0.1 defines a **sequentially consistent** memory model. All memory operations
appear to execute in program order. There is no concurrency. Atomics and weak
ordering are reserved for future versions.

---

## 3. Memory layout

### 3.1 Regions

The address space is partitioned into logical regions. The boundaries are
determined at program load time and remain fixed during execution.

```
Address
0                    ┌──────────────────────┐  ← null region
                     │  (reserved, no access)│
                     ├──────────────────────┤
1                    │                      │
   …                 │   Global variables   │
                     │                      │
G-1                  ├──────────────────────┤
                     │                      │
G                    │     Heap region      │  ← grows upward
   …                 │  (runtime-managed)   │
                     │                      │
S-1                  ├──────────────────────┤
                     │                      │
S                    │    Stack region      │  ← grows downward
   …                 │                      │
                     │                      │
M-1                  ├──────────────────────┤
                     │                      │
M                    │  Function ptr addrs  │  ← virtual addresses only
   …                 │                      │
M+F-1                └──────────────────────┘
```

Where:

| Symbol | Meaning | Set by |
|---|---|---|
| `G` | First address after globals (heap base) | Compiler (from global variable layout) |
| `S` | Stack base (bottom of stack region) | Compiler (`memory_size` configuration) |
| `M` | Total memory size (one past the last valid byte address) | Compiler (default 2¹⁶ = 65,536 for `sa48`) |
| `F` | Number of function pointer addresses | Compiler (one per distinct function) |

### 3.2 Global region (`[1, G)`)

Global variables are assigned consecutive byte-aligned addresses starting at
address 1. Each global occupies `sizeof(T)` bytes (see §4). The compiler
assigns addresses based on the module's global variable declarations, in
declaration order.

### 3.3 Heap region (`[G, S)`)

The heap region is **not managed by the architecture**. It exists in the
address space so that runtime libraries can implement heap allocators (malloc,
sbrk, etc.) using the same flat address space. The architecture does not
define any heap operations.

> **Design decision.** The prototype managed the heap explicitly with
> `!heap pointer` and an `sbrk()` function. For ScratchArch, heap management
> is a **runtime library** concern, not an architectural one. The architecture
> provides the address space layout; a runtime library provides allocation
> algorithms. This is consistent with LLVM, which does not define a heap
> model — that is the C runtime's responsibility.

### 3.4 Stack region (`[S, M)`)

The stack grows **downward** from `M-1` toward `S`. The stack pointer register
(`!sp`) starts at `M` (one past the highest valid address) and decreases as
`alloca` instructions execute. Details of stack frame management are in
[`ABI.md`](./ABI.md) §7; the address-space mechanics are in §5.1 below.

The minimum stack address is `S`. An `alloca` that would push `!sp` below `S`
has **undefined behavior** (stack overflow).

### 3.5 Function pointer region (`[M, M+F)`)

Function pointers are **virtual addresses** in this region. They do not
correspond to any storage in the address space. A `load` or `store` to an
address in this range has **undefined behavior**.

Each function in the program is assigned a unique address `M + i` for
`i = 0, 1, ..., F-1`. The mapping from function to address is determined by
the compiler and is fixed at program load time.

> **Why a reserved region?** The prototype placed function pointer addresses
> "beyond memory" at `memory_size + 1`. Formalizing a reserved region makes
> the invariants explicit: (a) function addresses never alias real memory,
> (b) the runtime can detect function pointers by range check, and (c) the
> region size is computable at compile time (one address per function).

---

## 4. Type sizes and alignment

### 4.1 `sizeof(T)` — size in addressable units (bytes)

| Type `T` | `sizeof(T)` (bytes) |
|---|---|
| `void` | 0 |
| `iN` | `ceil(N / 8)` |
| `ptr` | `ceil(Wptr / 8)` |
| `half` | 2 |
| `float` | 4 |
| `double` | 8 |
| `fp128` | 16 |
| `[M x E]` | `M × sizeof(E)` |
| `{T₁, T₂, …, Tₙ}` (aligned) | Sum of member sizes with natural alignment padding (see §4.3) |
| `<{T₁, T₂, …, Tₙ}>` (packed) | Sum of member sizes **without** alignment padding |
| `func(...)` | Not storable (no size) |
| `<M x T>` (vector) | Reserved for v0.2; no size in v0.1 |

> **Why `ceil(N/8)` for `iN` and not `ceil(N/W)`?** W is the cell width for
> computation (registers), not the memory unit. Memory is byte-addressable
> regardless of profile. An `i32` is always 4 bytes in memory; an `i64` is
> always 8 bytes. The cell decomposition for register storage is an ABI
> concern ([`ABI.md`](./ABI.md) §2), independent of memory layout.

### 4.2 `alignof(T)` — natural alignment

| Type `T` | `alignof(T)` |
|---|---|
| `iN` | `min(ceil(N / 8), 16)` — natural integer alignment, capped at 16 |
| `ptr` | `ceil(Wptr / 8)` |
| `half` | 2 |
| `float` | 4 |
| `double` | 8 |
| `fp128` | 16 |
| `[M x E]` | `alignof(E)` |
| `{T₁, T₂, …, Tₙ}` (aligned) | `max(alignof(T₁), …, alignof(Tₙ))` |
| `<{T₁, T₂, …, Tₙ}>` (packed) | 1 |

### 4.3 Struct padding (aligned structs)

In an aligned struct `{T₁, T₂, …, Tₙ}`, each member `Tₖ` is placed at the
minimum offset `≥ offsetₖ₋₁ + sizeof(Tₖ₋₁)` that satisfies
`offset mod alignof(Tₖ) = 0`. The total size is rounded up to a multiple of
`alignof(struct)`.

Packed structs `<{…}>` have no padding: members are placed at consecutive
offsets.

### 4.4 Size and alignment examples

| Type | `sa48` | `sa64` |
|---|---|---|
| `i8` | 1 byte, align 1 | 1 byte, align 1 |
| `i32` | 4 bytes, align 4 | 4 bytes, align 4 |
| `i64` | 8 bytes, align 8 | 8 bytes, align 8 |
| `ptr` | 4 bytes, align 4 (Wptr=32) | 8 bytes, align 8 (Wptr=64) |
| `double` | 8 bytes, align 8 | 8 bytes, align 8 |
| `[4 x i32]` | 16 bytes, align 4 | 16 bytes, align 4 |
| `{i8, i32}` (aligned) | 8 bytes (1 + 3 pad + 4), align 4 | 8 bytes, align 4 |
| `<{i8, i32}>` (packed) | 5 bytes, align 1 | 5 bytes, align 1 |

---

## 5. Memory instructions

### 5.1 `alloca`

```
%addr = alloca T [, i32 %count]
```

**Semantics.** Reserves `sizeof(T) × count` bytes on the stack and returns the
base address of the allocated block. The stack pointer `!sp` is decreased by
the total allocation size. The returned address is the new value of `!sp` (the
lowest address of the allocated range).

```
!sp ← !sp − sizeof(T) × count
%addr ← !sp    ; address of allocated block
```

> **Why `!sp` is the returned address (low end).** This matches the convention
> where the stack grows downward and the stack pointer points to the most
> recently allocated byte. After `alloca`, `!sp` points to the start of the
> new allocation, and the allocation extends upward to `!sp + sizeof(T) − 1`.
> This is also the convention used by the prototype.

**Alignment.** The allocated address is aligned to `alignof(T)`. The runtime
must ensure `!sp` is adjusted to satisfy alignment before the address is
returned.

**Count parameter.** If `count` is present and not 1, the allocation is an
array of `count` elements. The total size is `sizeof(T) × count`. Dynamic
arrays (count not a compile-time constant) are supported; overflow of the
multiplication is undefined behavior.

**Lifetime.** The allocated memory is valid until the containing function
returns (see [`ABI.md`](./ABI.md) §7.2). Immediately after `ret`, the memory
may be reused.

### 5.2 `load`

```
%val = load T, ptr %addr
```

**Semantics.** Reads `sizeof(T)` consecutive bytes from memory starting at
`%addr`, interprets them as a little-endian encoding of type `T`, and produces
a value of type `T`.

```
bytes[i] = memory[⟦%addr⟧ + i]   for i = 0, 1, ..., sizeof(T)-1
%val = decode_le(T, bytes)
```

The loaded value, once in a register, is subject to cell decomposition
([`ABI.md`](./ABI.md) §2) if `sizeof(T) × 8 > W`. The decomposition is derived
from the little-endian byte encoding: the first `W` bits (bytes 0…ceil(W/8)-1)
form cell 0, the next `W` bits form cell 1, and so on.

> **Realization note (sa48).** A `load i64` reads 8 bytes from memory. The
> resulting 64-bit value decomposes into 2 cells (lower 48 bits → cell 0,
> upper 16 bits → cell 1). The load itself is defined on bytes; cell
> decomposition is a register concern that follows from the byte
> representation.

**Unaligned access.** If `%addr` is not a multiple of `alignof(T)`, the
behavior is undefined unless the load is annotated `align 1` (packed access).
This matches LLVM's alignment semantics.

### 5.3 `store`

```
store T %val, ptr %addr
```

**Semantics.** Encodes `%val` as `sizeof(T)` bytes in little-endian order and
writes them to consecutive memory addresses starting at `%addr`.

```
bytes = encode_le(T, %val)        ; sizeof(T) bytes
memory[⟦%addr⟧ + i] = bytes[i]   for i = 0, 1, ..., sizeof(T)-1
```

The source value's register representation (possibly multi-cell) is converted
to a single byte sequence. This is the inverse of `load`.

**Unaligned access.** Same rule as `load`.

### 5.4 `getelementptr`

```
%ptr = getelementptr T, ptr %base, <type> %idx, ...
```

**Semantics.** Computes the address of a subelement of an aggregate value
following LLVM's GEP formula exactly. `T` is the pointee type of the base
pointer. Each index descends into the aggregate type structure.

```
addr = ⟦%base⟧
for each index (type, value):
    if type is an integer:
        addr += value × sizeof(containing_type)
    elif type is a struct index (i32):
        addr += offset_of_field(containing_type, value)
```

> **Why not simplify GEP?** GEP is the most LLVM-specific instruction and the
> hardest to get right. The prototype's `getGepOffsets` and `applyGepOffsets`
> faithfully implement LLVM semantics, and there is no benefit to diverging.
> A correct GEP is essential for LLVM backend compatibility.

**Opaque pointers.** The base pointer is opaque (no pointee type). The element
type `T` is provided as an explicit parameter to the GEP instruction. This
matches LLVM's opaque pointer model.

**Inbounds.** GEP may be annotated `inbounds`. If `inbounds` is present,
pointer arithmetic that would produce an address outside the allocated object
(based on the base pointer) has undefined behavior. This is an optimization
hint like LLVM's `inbounds GEP`; the architecture does not require the runtime
to check bounds.

### 5.5 `memcpy`, `memmove`, `memset`

These are **intrinsics** (not core instructions) with reference expansions in
the intrinsic registry ([`ISA.md`](./ISA.md) §8):

```
memcpy(ptr %dst, ptr %src, iN %len, i1 %is_volatile)
```

Expands to a byte-by-byte copy loop (or an optimized sequence when `len` is a
small compile-time constant). `memmove` handles overlapping regions correctly.
`memset(ptr %dst, i8 %val, iN %len)` writes `%val` to each byte.

A runtime may provide native implementations of these intrinsics.

---

## 6. Endianness and byte ordering

### 6.1 Little-endian byte order

All multi-byte values in memory are stored in **little-endian** byte order: the
least significant byte is at the lowest address.

For a value `v` of type `iN` stored at address `A`:

```
memory[A + 0] = byte 0  (bits 0…7)
memory[A + 1] = byte 1  (bits 8…15)
...
memory[A + k] = byte k  (bits 8k…8k+7)
```

### 6.2 Consistency with cell ordering

Little-endian byte order in memory is consistent with little-endian cell order
in registers ([`ABI.md`](./ABI.md) §2.2). For a value wider than a cell:

- Cells in registers: cell 0 = least significant `W` bits
- Bytes in memory: byte 0 = least significant 8 bits

The conversion between memory bytes and register cells is straightforward:
cell `i` is reconstructed from bytes `i × ceil(W/8)` through
`(i+1) × ceil(W/8) - 1` (with the last cell possibly using fewer bytes for
non-byte-aligned W).

> **Example (sa48, W=48).** Cell 0 is reconstructed from bytes 0–5 (48 bits),
> cell 1 from bytes 6–11, etc. The unused bits in the most significant cell
> are zero on load and ignored on store.

---

## 7. Memory initialization

### 7.1 Global initialization

Global variables are initialized from the module's initializer values before
the program's entry function executes. The initialization is equivalent to a
sequence of `store` instructions:

- Integer globals: initialized to the module-specified constant.
- Aggregate globals: initialized element-by-element.
- Zero-initialization: used when no explicit initializer is present.

### 7.2 Stack and heap initialization

Memory allocated by `alloca` (stack) and uninitialized heap memory have
**undefined initial contents**. Reading from uninitialized memory without first
storing to it is undefined behavior (matching LLVM's `undef` semantics).

### 7.3 Zero-initialization guarantee

The architecture does not guarantee zero-initialized memory. Runtimes may
zero-fill for safety, but programs must not depend on it.

---

## 8. Examples

### 8.1 `alloca` and `load`/`store`

```
define i32 @example() {
entry:
  %p = alloca i32                         ; !sp -= 4, %p = !sp
  store i32 42, ptr %p                    ; write 4 bytes at %p
  %v = load i32, ptr %p                   ; read 4 bytes from %p
  ret i32 %v                              ; returns 42
}
```

**Under sa48:** `i32` is 4 bytes in memory, 1 cell in registers. The stack
pointer decreases by 4. `store` writes the 4-byte little-endian encoding of 42
to addresses `%p` .. `%p+3`. `load` reads them back.

### 8.2 Struct field access (GEP)

```
define i32 @get_y(ptr %s) {
  %y = getelementptr {i8, i32}, ptr %s, i32 0, i32 1
  %v = load i32, ptr %y
  ret i32 %v
}
```

**Under sa48:** The struct `{i8, i32}` has sizeof = 8 (1 + 3 padding + 4),
alignof = 4. Field 1 (`i32`) is at offset 4 from the struct start. GEP with
indices `(0, 1)` computes `%s + 4`. The load reads 4 bytes from that address.

### 8.3 Array access

```
define i32 @sum(ptr %arr) {
entry:
  br label %loop

loop:
  %i = phi i32 [0, %entry], [%next, %loop]
  %acc = phi i32 [0, %entry], [%sum, %loop]
  %gep = getelementptr [4 x i32], ptr %arr, i32 0, i32 %i
  %v = load i32, ptr %gep
  %sum = add i32 %acc, %v
  %next = add i32 %i, 1
  %done = icmp eq i32 %next, 4
  br i1 %done, label %exit, label %loop

exit:
  ret i32 %acc
}
```

GEP computes `%arr + %i × 4` (each `i32` is 4 bytes). The `[4 x i32]` type
gives the stride.

### 8.4 Multi-cell load (sa48)

```
define i64 @load64(ptr %p) {
  %v = load i64, ptr %p
  ret i64 %v
}
```

**Under sa48:** `i64` is 8 bytes in memory. The load reads bytes `[%p .. %p+7]`
and produces a 64-bit value. In registers, this value decomposes into 2 cells:
cell 0 = lower 48 bits (bytes 0–5), cell 1 = upper 16 bits (bytes 6–7). The
store reverses this.

---

## 9. Relationship with LLVM

| LLVM concept | ScratchArch memory model |
|---|---|
| Flat address space | Same (§2.1) |
| Byte addressing | Same (§2.1) |
| `alloca` | Same semantics (§5.1) |
| `load` / `store` | Same semantics (§5.2, §5.3) |
| `getelementptr` | Same semantics with opaque pointers (§5.4) |
| `alignof` / `sizeof` | Same computation (§4) |
| Null pointer | Same: address 0, access is UB (§2.4) |
| Little-endian | Same (§6.1) |
| `undef` / `poison` for uninitialized memory | Same (§7.2) |
| `inbounds` GEP | Same semantics (§5.4) |
| `memcpy` / `memmove` / `memset` | Reference expansions in intrinsic registry (§5.5) |
| `alloca` with dynamic count | Same (§5.1) |
| `addrspace` | Reserved for v0.2 (only addrspace 0 in v0.1) |
| `volatile` load/store | Recognized but not given special semantics in v0.1; future may define ordering guarantees |
| Atomics (`atomicrmw`, `cmpxchg`, `fence`) | Not supported in v0.1 |
| `invariant.load` / `invariant.group` | Accepted as metadata; no architectural obligation |

**Backend obligations.** An LLVM→ScratchArch backend must:
- Lower `alloca` to stack-pointer adjustment with natural alignment.
- Lower `load`/`store` to byte-sequence reads/writes with little-endian encoding.
- Lower `getelementptr` to the standard LLVM offset formula.
- Ensure that the generated code matches the profile's `sizeof` and `alignof`.
- Map `memcpy`/`memmove`/`memset` to the intrinsic registry's reference expansion
  (or a faster native implementation).

---

## 10. Future extensions

- **Address spaces.** Multiple address spaces (LLVM `addrspace(N)`) with
  separate memory ranges, enabling Harvard-architecture models (code vs data),
  MMIO regions, and GPU-like memory hierarchies.
- **Atomics and concurrency.** A full memory model with
  `load atomic`/`store atomic`/`atomicrmw`/`cmpxchg`/`fence`, following the
  LLVM memory model (sequentially consistent, acquire-release, relaxed).
- **Volatile.** Defined semantics for `load volatile`/`store volatile` as
  observable side effects that cannot be optimized away.
- **Explicit unmapped pages.** Memory protection concepts (read-only, no-execute)
  for sandboxing and runtime safety.
- **Finer-grained profiles.** Profile-specific addressing units (e.g., a
  `sa48_word` profile with 4-byte addressing for reduced storage at the cost
  of byte-granularity operations).
- **Big-endian profile.** An optional big-endian byte ordering, for
  compatibility with network byte order or certain legacy targets.

---

## 11. Frozen decisions (v0.1)

1. The address space is flat, linear, and byte-addressed (§2.1).
2. Only address space 0 exists; named address spaces are reserved (§2.2).
3. Address arithmetic wraps modulo `2^Wptr` (§2.3).
4. Address 0 is the null pointer; access is undefined behavior (§2.4).
5. The memory model is sequentially consistent (no concurrency) (§2.5).
6. Memory is partitioned into null, global, heap, stack, and function-pointer
   regions (§3).
7. `sizeof(T) = ceil(N/8)` for `iN`; float sizes follow IEEE-754; aggregate
   sizes include alignment padding for aligned structs (§4).
8. `alloca` decrements `!sp` and returns the new `!sp` as the base address
   (§5.1).
9. `load` and `store` transfer `sizeof(T)` bytes with little-endian encoding
   (§5.2, §5.3).
10. `getelementptr` follows LLVM's exact semantics, including `inbounds` as an
    optimization hint (§5.4).
11. Byte order is little-endian (§6).
12. Uninitialized memory has undefined contents (§7.2).
