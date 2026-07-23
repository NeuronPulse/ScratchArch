# ScratchArch Reference VM — Implementation Notes

> Document version: 0.1 (matching ISA/ABI/MEMORY v0.1)

---

## 1. Overview

This is the first reference implementation of the ScratchArch Abstract Machine (SAM).
It implements the `sa48` profile (W=48, Wptr=32) with a simplified stack-based instruction
set targeting correct, readable execution — not performance.

Two crates:

- **`scratcharch-core`** — architecture definitions (types, values, instructions, program IR)
- **`scratcharch-vm`** — runtime execution (operand stack, memory, call stack, instruction loop)

---

## 2. VM Architecture

### 2.1 State

The VM struct holds:

| Field | Purpose |
|---|---|
| `functions: Vec<FuncCode>` | Resolved instruction sequences per function |
| `current_func: usize` | Index of the currently executing function |
| `pc: usize` | Program counter (index within current function) |
| `stack: OperandStack` | Operand stack (holds typed runtime values) |
| `call_stack: CallStack` | Call frames for function call / return |
| `memory: LinearMemory` | Byte-addressable linear memory |
| `sp: u32` | Stack pointer for `alloca` (grows downward) |
| `running: bool` | Execution flag |

### 2.2 Execution loop

```
while running:
    fetch instruction at [current_func][pc]
    increment pc
    decode & execute
```

The loop is in `execute.rs`. Each instruction variant has an isolated `match` arm.

### 2.3 Operand stack

Typed values (`Value` enum from `scratcharch-core`) are pushed and popped. Type mismatches
are detected at runtime and reported as `VmError::TypeMismatch`.

### 2.4 Calling convention

The calling convention follows a **stack-depth save/restore** protocol:

- **CALL**: pushes a frame containing `(return_func, return_pc, sp, stack_len)`.
  The callee inherits the operand stack as-is (arguments are on top).
- **RETURN**: pops the return value, restores the caller's frame, truncates the
  operand stack to `saved_stack_len`, and pushes the return value.

This means the callee's stack usage (arguments + temporaries) is discarded on return.
Only the return value survives. This allows recursive calls to work correctly because
each call level's preserved values are below the saved stack length.

### 2.5 Memory model

Byte-addressable linear memory, little-endian. `sa48` profile: 32-bit addresses, 65536 byte
capacity by default. The stack region grows downward from the top of memory. `ALLOC`
decrements `sp` and returns the new address.

---

## 3. Implemented Instructions

### Constants
| Instruction | Stack effect |
|---|---|
| `CONST_I32 v` | `→ i32(v)` |
| `CONST_F64 v` | `→ f64(v)` |

### Stack manipulation
| Instruction | Stack effect |
|---|---|
| `DROP` | `a →` |
| `DUP` | `a → a a` |

### Integer arithmetic (i32, wrapping)
| Instruction | Stack effect | Semantics |
|---|---|---|
| `I32_ADD` | `a b → (a+b) mod 2³²` | wrapping add |
| `I32_SUB` | `a b → (a−b) mod⁺ 2³²` | wrapping sub |
| `I32_MUL` | `a b → (a·b) mod 2³²` | wrapping mul |
| `I32_DIV` | `a b → a ÷ b` | unsigned div (UB if b=0) |
| `I32_REM` | `a b → a mod b` | unsigned rem (UB if b=0) |

### Bitwise (i32)
| Instruction | Stack effect |
|---|---|
| `AND` | `a b → a & b` |
| `OR` | `a b → a \| b` |
| `XOR` | `a b → a ^ b` |
| `SHL` | `a b → a << (b & 31)` |
| `SHR` | `a b → a >> (b & 31)` (logical) |

### Comparison (i32 → i1)
| Instruction | Stack effect |
|---|---|
| `EQ` | `a b → i1(a == b)` |
| `LT` | `a b → i1(a < b)` (unsigned) |
| `GT` | `a b → i1(a > b)` (unsigned) |

### Memory
| Instruction | Stack effect | Description |
|---|---|---|
| `ALLOC` | `size → ptr` | Allocate `size` bytes on stack |
| `LOAD` | `addr → i32(val)` | Load 4 bytes as i32 (little-endian) |
| `STORE` | `val addr →` | Store i32 as 4 bytes (little-endian) |

### Control flow
| Instruction | Description |
|---|---|
| `JUMP label` | Unconditional branch to label |
| `BRANCH t f` | Pop i1 condition; branch to t if true, f if false |
| `CALL func` | Call function by name |
| `RETURN` | Return from function (stops VM if at top level) |

---

## 4. Error handling

Errors are modeled as `VmError` enum:

| Variant | Cause |
|---|---|
| `StackUnderflow` | Pop from empty operand stack |
| `TypeMismatch` | Instruction expects a specific type (e.g., `i32` but got `f64`) |
| `DivisionByZero` | I32_DIV or I32_REM with zero divisor |
| `MemoryError` | Out-of-bounds access, null pointer, stack overflow |
| `CallStackEmpty` | RETURN with no caller |
| `UndefinedFunction` | CALL to unknown function name |
| `UndefinedLabel` | JUMP/BRANCH to unknown label |
| `InvalidAddress` | Reserved for future use |

---

## 5. Known limitations

1. **No SSA form.** The ISA (§2.5) defines SSA virtual registers, but this reference VM
   uses a stack machine for simplicity. An SSA-based implementation would differ.

2. **No multi-cell values.** The `sa48` profile splits `i64` across 2 cells; this
   VM only implements single-cell types (`i32`, `f64`, `ptr`).

3. **i32-only arithmetic.** Full ISA supports arbitrary `iN` widths; this VM
   implements only `i32` arithmetic and `i1` comparisons.

4. **No floats in memory.** `f64` constants and arithmetic work on the stack, but
   LOAD/STORE assume i32 encoding.

5. **No heap.** The memory model defines a heap region (§3.3 of MEMORY.md), but no
   heap allocator is implemented.

6. **No function pointers.** Indirect call via function pointer (ABI §8) is not
   implemented.

7. **No variadic functions.** Hidden parameters (`!return.address`, `!fnptr`,
   `!vararg.ptr`) from ABI §5.3 are not implemented.

8. **No `getelementptr`.** GEP from MEMORY.md §5.4 is not implemented; only simple
   `ALLOC`/`LOAD`/`STORE` are available.

9. **No overflow checking.** Division by zero is caught; all other overflow
   wraps silently per ISA §2.3.

10. **Shared operand stack for arguments.** Arguments are passed on the shared
    operand stack. The stack-depth save/restore protocol handles this correctly
    for most patterns, but functions must not read values below their arguments.

---

## 6. Future work

- Implement the full SSA instruction set from ISA.md
- Support multi-cell values for the `sa48` profile (i64, large arrays)
- Add `getelementptr` for aggregate access
- Add function pointers and indirect calls
- Implement intrinsics (memcpy, memset, ctpop, etc.)
- Add heap allocation via runtime library
- Optimize: threaded dispatch, pre-allocated stacks
- Write an LLVM backend targeting this VM
