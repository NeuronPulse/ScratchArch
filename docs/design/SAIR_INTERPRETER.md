# SAIR Interpreter

> Document version: 0.1
> Status: reference implementation

## Overview

The SAIR interpreter (`scratcharch-sair-interpreter`) directly executes SAIR
programs without lowering to the ScratchArch ISA. It serves as a **reference
execution model** for SAIR semantics and enables testing of SAIR features
(phi, GEP, multi-block CFG) without requiring a working ISA backend.

## Architecture

The interpreter walks the SAIR `IrModule` directly:

```
Interpreter
├── module: IrModule          // SAIR program to execute
├── frames: Vec<Frame>        // call stack (activation frames)
├── memory: Vec<u8>           // flat byte-addressable memory
├── sp: u32                   // stack pointer (grows downward)
└── instr_id_maps             // block+instr_idx → ValueId mapping
```

### Frame (Activation)

Each function call creates a fresh `Frame`:

```
Frame
├── func_name: String
├── values: Vec<Option<RuntimeValue>>  // SSA value storage
├── return_ty: IrType
├── current_block: String
├── instr_idx: usize
└── prev_block: Option<String>     // predecessor block for phi
```

The `values` array is indexed by `ValueId`. Parameter values occupy the first
N slots (matching the function's parameter list). Instruction results occupy
subsequent slots in the order they appear across all blocks.

### Value Storage

`RuntimeValue` mirrors `scratcharch_core::value::Value`:

```rust
enum RuntimeValue {
    I1(bool),
    I8(u8),
    I16(u16),
    I32(u32),
    F64(f64),
    Pointer(u32),
}
```

Values are stored in a flat `Vec<Option<RuntimeValue>>` indexed by `ValueId`.
The mapping from `(block_label, instr_idx)` to `ValueId` is pre-computed at
interpreter construction time via `InstrIdMap`, which visits all instructions
in function order and assigns sequential IDs to each instruction that produces
a result.

### Execution Loop

```
while true:
    get current block
    while instr_idx < block.instructions.len():
        execute instruction
        instr_idx += 1
    execute terminator
    match terminator result:
        Continue:           // branch/cond_br → loop with new block
        Return(Some(val)):  // pop frame, return val to caller
        Return(None):       // pop frame, void return
```

## Supported Instructions

| Category | Instructions |
|----------|-------------|
| Integer arithmetic | Add, Sub, Mul, Div, Rem (wrapping per ISA.md §2.3) |
| Comparison | Eq, Lt, Gt (unsigned, produce i1) |
| Constants | I1, I8, I16, I32, F64 |
| Memory | Alloca, Load, Store |
| Calls | Call (with return value), Return |
| Control flow | Branch, CondBranch |
| SSA | Phi (with edge-selected semantics) |
| Address computation | GEP (dynamic and struct field indices) |

## Phi Handling

Phi reads the `prev_block` field from the current frame to determine which
predecessor block was taken, then selects the corresponding incoming value.
This is correct because:

1. Before a branch/cond_br, the terminator sets `prev_block` to the current
   block label
2. The successor block's first instructions (phi) read `prev_block`
3. The phi selects the appropriate value from the incoming list

## Memory Model

Follows MEMORY.md:

- Flat byte-addressable memory (Vec<u8>)
- Little-endian byte order
- Stack grows downward from `memory_size`
- Stack limit at `stack_limit` (default 4096)
- Address 0 is null pointer (access returns NullPointer error)
- `alloca` decrements `sp` and returns the new `sp` as the allocated address
- `load`/`store` read/write `sizeof(T)` bytes with LE encoding

## GEP

GEP follows MEMORY.md §5.4: computes the address of a subelement given a
base pointer and index list. Dynamic indices use the element type's
`size_in_bytes()` for stride computation. Struct field indices use a fixed
4-byte stride (simplified for untyped structs).

## Error Handling

| Error | Cause |
|-------|-------|
| UndefinedValue | ValueId not found in frame |
| TypeMismatch | Operation on wrong value type |
| DivisionByZero | Div/Rem by zero |
| MemoryOutOfBounds | Access beyond memory bounds |
| NullPointer | Access through address 0 |
| StackOverflow | alloca past stack limit |
| UndefinedFunction | Call to unknown function |
| UndefinedBlock | Branch to unknown label |
| CallStackEmpty | Return with no caller |
| CallStackOverflow | Call depth exceeded 1024 |
| InvalidGepIndex | GEP index of wrong type |
| UnsupportedInstruction | Instruction not yet implemented |

## Relationship with IsaLowerer

The interpreter and the ISA lowerer serve complementary roles:

| Aspect | Interpreter | IsaLowerer |
|--------|-------------|------------|
| Input | SAIR (IrModule) | SAIR (IrModule) |
| Output | Runtime value | ISA Program (for VM) |
| Phi | ✅ Full support | ❌ Rejected |
| Multi-block | ✅ Full support | ❌ Rejected (single-block only) |
| GEP | ✅ Full support | ❌ Rejected |
| Types | All SAIR types | i32/f64 only |
| Purpose | Reference semantics | Production execution on VM |

The interpreter is the **truth** for SAIR semantics. The lowerer is the
**optimization path** for simple programs that can run on the VM.
