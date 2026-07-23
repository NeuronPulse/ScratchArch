# ScratchArch Target Profile

> Specification version: **v0.1 (draft for freezing)**
> Status: normative
> Companion documents: [`ISA.md`](./ISA.md), [`ABI.md`](./ABI.md), [`MEMORY.md`](./MEMORY.md),
> [`EXECUTION_MODEL.md`](./EXECUTION_MODEL.md).

---

## 1. Motivation

ScratchArch is an **architecture family**, not a single machine. The ISA, ABI,
memory model, and execution semantics are parametric — they define a family of
machines that differ in concrete numeric parameters. A **target profile** pins
those parameters to specific values, producing a concrete machine
configuration.

The relationship is:

```
ScratchArch (architecture family)
    ↓ parameterized by
TargetProfile (concrete configuration)
    ↓ realized by
Runtime implementation (VM, interpreter, hardware)
```

This document defines what a target profile is, enumerates the known profiles
(v0.1), and explains how a profile governs code generation, execution, and
runtime behavior.

---

## 2. Architecture vs Profile vs Implementation

### 2.1 Architecture (ScratchArch)

The architecture is the **specification layer**. It defines:

- The instruction set (ISA.md)
- The type system and value representation
- Control flow and basic blocks
- Calling convention (ABI.md)
- Memory layout and operations (MEMORY.md)
- Execution semantics (EXECUTION_MODEL.md)

These specifications are parametric. For example, the ISA says "cell width `W`
is a profile parameter" without fixing a specific value.

### 2.2 Profile

The profile binds the parameters to concrete values:

| Parameter | Symbol | Effect |
|---|---|---|
| Cell width | `W` | Width of the fundamental scalar unit (bits) |
| Pointer width | `Wptr` | Width of addresses (bits) |
| Endianness | — | Byte ordering in memory |
| Integer model | — | Arithmetic semantics (e.g., modular wrapping) |
| Memory model | — | Address space structure |
| ABI version | — | Calling convention rules |

A profile is **immutable**: once frozen, it defines an exact machine contract.
Programs compiled for a profile are not guaranteed to run on a different
profile without recompilation.

### 2.3 Implementation (Runtime)

The implementation realizes the profile on a specific platform. Different
implementations of the same profile must produce the same observable results:

| Implementation | Platform | Profile |
|---|---|---|
| `scratcharch-sair-interpreter` | Host Rust (any OS) | SA48 |
| `scratcharch-vm` | Host Rust (any OS) | SA48 |
| TurboWarp backend | Scratch/TurboWarp | SA48 |
| Native VM | C/Rust, bare metal | SA48, SA64 |

All implementations of SA48 are interchangeable from the program's
perspective (performance may differ; results must not).

---

## 3. Profile: SA48 (Reference)

### 3.1 Definition

```
Name:             SA48
Cell width:       48 bits
Pointer width:    32 bits
Endianness:       Little
Integer model:    Modular wrapping (mod 2^N)
Memory model:     Flat byte-addressable
ABI version:      v0.1
```

### 3.2 Properties

| Property | Value | Rationale |
|---|---|---|
| `sizeof(ptr)` | 4 bytes | 32-bit address space → 4 GiB addressable |
| `alignof(ptr)` | 4 bytes | Natural alignment for 4-byte pointer |
| Addressable range | 0 … 2³²−1 (4 GiB) | Wptr = 32 |
| Max memory (default) | 65,536 bytes | Profile recommendation; runtime may increase |
| Double-precision floats | Native | Cell is 48 bits; float is stored as double in a single cell |
| `i64` decomposition | 2 cells (low 48 + high 16) | ceil(64/48) = 2, little-endian |

### 3.3 Integer Handling

| Type | Cells | Bytes in memory | Notes |
|---|---|---|---|
| `i1` | 1 | 1 | Stored as 0/1 in a cell |
| `i8` | 1 | 1 | |
| `i16` | 1 | 2 | |
| `i32` | 1 | 4 | |
| `i64` | 2 | 8 | Low 48 bits in cell 0, high 16 bits in cell 1 |
| `ptr` | 1 | 4 | 32-bit pointer fits in one 48-bit cell |

### 3.4 Motivation for 48-bit Cell

The 48-bit cell width is not arbitrary: it derives from the IEEE-754 double
precision mantissa (53 bits minus 5 bits of headroom for intermediate
arithmetic). On a float-substrate runtime (Scratch/TurboWarp), all values are
stored as double-precision floats. A 48-bit exact integer guarantees that
multiplication of two 24-bit halves stays within 53 bits (the mantissa
precision). This enables the half-split multiplication algorithm proven by the
archaeology.

### 3.5 SA48 Realization Note

When the ISA references "the sa48 realization note" for an instruction, it
refers to the set of algorithms proven by the `llvm2scratch` archaeology:

- **Multiply**: Half-split `a·b = a₀b₀ + 2^w(a₀b₁ + a₁b₀) + 2^{2w}a₁b₁` with
  `w = ceil(N/2)` and modular reduction of the middle term.
- **Signed division**: Four-case sign tree on `sⁿ(a)/sⁿ(b)`.
- **Bitwise ops**: Byte-chunk lookup tables and identity formulas.
- **IEEE-754 bitcast**: Logarithmic exponent estimate + mantissa extraction.

These techniques are proven correct for SA48 and are the recommended
realization, but any algorithm producing the specified result is conforming.

---

## 4. Future Profiles

### 4.1 SA32

```
Cell width:       32 bits
Pointer width:    32 bits
Endianness:       Little
Integer model:    Modular wrapping
Memory model:     Flat byte-addressable
ABI version:      v0.1+
```

SA32 is a native 32-bit profile. All arithmetic is native 32-bit. The 32-bit
cell matches the pointer width, so pointers occupy one cell. Main application:
native 32-bit VMs and embedded systems. Not yet defined in v0.1.

### 4.2 SA64

```
Cell width:       64 bits
Pointer width:    64 bits
Endianness:       Little
Integer model:    Modular wrapping
Memory model:     Flat byte-addressable
ABI version:      v0.1+
```

SA64 is a native 64-bit profile. Single-cell `i64` arithmetic. 64-bit address
space. Main application: native VMs and 64-bit hosts. Not yet defined in v0.1.

### 4.3 SA48X (Extended)

A hypothetical future profile with 48-bit cell but wider (48-bit) pointers,
enabling a larger address space while keeping the float-substrate cell width.
Not yet defined.

---

## 5. Profile Selection and Code Generation

### 5.1 Compile-Time Selection

The target profile is selected at compile time and baked into the generated
code. Changing the profile between compilation and execution is undefined
behavior. In a toolchain pipeline:

```
Source (C/LLVM)
    ↓
Select target profile (--target=sa48)
    ↓
Compile/Lower (respects profile parameters)
    ↓
Execute on any runtime implementing the profile
```

### 5.2 Profile-Conscious Lowering

The lowering passes must consult the target profile for:

- **Cell decomposition**: Splitting `i64` into multiple cells for SA48, or
  keeping it single-cell for SA64.
- **Pointer size**: 4-byte pointers for SA48, 8-byte for SA64.
- **Stack frame layout**: Slot sizes and alignment derived from profile.
- **Multi-cell load/store**: Emitting byte sequences for values wider than `W`.

### 5.3 Runtime Profile Check

A runtime implementation may support multiple profiles by parameterizing
internal structures (cell size, address width) at startup. The `TargetProfile`
structure in `scratcharch-target` provides the canonical runtime
representation.

---

## 6. Implementation

The `scratcharch-target` crate (`crates/scratcharch-target/`) provides:

- `TargetProfile` struct with SA48 factory method
- `validate()` method ensuring profile consistency
- `Display` implementation for human-readable profile description
- `MemoryLayout` helper for address space partitioning
- `AbiConvention` helper for cell-level ABI calculations

---

## 7. Frozen Decisions (v0.1)

1. SA48 is the reference profile; SA32 and SA64 are reserved for future
   versions (§3, §4).
2. All profiles share the same ISA semantics; only parameters differ (§2.1).
3. Cell width must be a multiple of 8 bits (§3.1).
4. Pointer width must be ≤ cell width (§3.1).
5. The pointer width determines the maximum addressable range (§3.2).
6. Profiles are immutable once frozen; programs are compiled for a specific
   profile (§2.2).
7. Different implementations of the same profile must produce identical
   results (§2.3).
