# LLVM IR → SAIR Translation

> Document version: 0.1
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

### Types

| LLVM IR | SAIR | Notes |
|---------|------|-------|
| `i1` | `IrType::I1` | Boolean type |
| `i8` | `IrType::I8` | Byte type |
| `i16` | `IrType::I16` | Short type |
| `i32` | `IrType::I32` | Primary integer type |
| `ptr` | `IrType::Pointer` | Generic pointer |
| `void` | `IrType::Void` | No return value |

Unsupported types (i64, f64, float, double, struct, array, vector) produce a
clear error at parse time.

### Values

| LLVM IR | SAIR |
|---------|------|
| `%name` | `ValueId` (via name lookup in value map) |
| `@name` | Function name (for call targets) |
| Integer literal | `Instruction::Const(Constant::I32(v))` or typed constant |
| `true`/`false` | `Constant::I1(v)` |

### Instructions

| LLVM IR | SAIR | Notes |
|---------|------|-------|
| `add` | `Instruction::Add` | Wrapping add |
| `sub` | `Instruction::Sub` | Wrapping sub |
| `mul` | `Instruction::Mul` | Wrapping mul |
| `icmp eq` | `Instruction::Eq` | Unsigned equality |
| `icmp ne` | `Eq` + `Eq` negation | `ne(a,b) = eq(eq(a,b), false)` |
| `icmp slt` | `Instruction::Lt` | **Unsigned** comparison (signed TODO) |
| `icmp sgt` | `Instruction::Gt` | **Unsigned** comparison (signed TODO) |
| `alloca` | `Instruction::Alloca` | Stack allocation |
| `load` | `Instruction::Load` | Typed load |
| `store` | `Instruction::Store` | Typed store |
| `call` | `Instruction::Call` | Direct call only |
| `ret` | `Terminator::Return` | Function return |
| `br` | `Terminator::Branch` | Unconditional branch |
| `br i1 ...` | `Terminator::CondBranch` | Conditional branch |
| `getelementptr` | `Instruction::Gep` | Dynamic indices only |

### Basic blocks

LLVM IR basic blocks (labels) map directly to SAIR `BasicBlock` objects. The
entry block must be first in both representations.

### Functions

LLVM IR `define` maps to `IrFunction`. Parameters become SSA values via
`IrBuilder::add_param()`. The function named `main` is used as the module
entry point.

## 4. Supported subset

### Fully supported

- `define` with `i32`, `i1`, `i8`, `i16`, `ptr`, `void` return types
- Parameters of supported types
- Arithmetic: `add`, `sub`, `mul`
- Comparison: `icmp eq`, `icmp ne`
- Memory: `alloca`, `load`, `store`
- Control flow: `br` (unconditional), `br i1 ...` (conditional)
- Calls: `call` (direct, with arguments)
- GEP: `getelementptr` (dynamic indices)
- Block labels and multi-block functions

### Not supported (rejected with clear error)

- `i64`, `float`, `double`, struct, array, vector types
- `fadd`, `fsub`, `fmul`, `fdiv` (floating-point operations)
- `sdiv`, `srem`, `udiv`, `urem` (division/remainder)
- `shl`, `lshr`, `ashr` (shift operations)
- `and`, `or`, `xor` (bitwise operations)
- `select` (ternary)
- `phi` (SAIR supports phi, but LLVM phi not yet parsed)
- Global variables
- Indirect calls (`call` via `ptr`)
- Metadata, debug info, attributes
- Alignment specifiers on load/store
- `volatile` loads/stores

### Limitations (known, not errors)

- `icmp slt`/`icmp sgt` use **unsigned** comparison (Lt/Gt). Works for positive
  values; signed comparison is future work.
- All arithmetic is wrapping (matching SAIR semantics).
- Single-entry module only (first `main` function is the entry point).

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

The translator tests exercise the full pipeline:

| Test | LLVM Pattern | Expected |
|------|-------------|----------|
| Return constant | `ret i32 42` | 42 |
| Arithmetic | `add i32 20, 22` | 42 |
| Function call | `call @add(i32 20, i32 22)` | 42 |
| Branch (true) | `icmp eq 1, 1` → `br i1` then/else | 1 |
| Branch (false) | `icmp eq 1, 2` → `br i1` then/else | 0 |
| Memory | `alloca`/`store`/`load` | 42 |
| Unconditional branch | `br label %exit` | 42 |
| Void return | `ret void` | None |

## 8. Future work

- **Signed comparison**: Implement proper signed less-than/greater-than for
  `icmp slt`/`sgt` (requires SAIR signed comparison support).
- **More icmp predicates**: `ule`, `uge`, `ult`, `ugt` (easy; map to existing
  unsigned Lt/Gt). `sle`, `sge` (need signed support).
- **Division/remainder**: `sdiv`, `udiv`, `srem`, `urem`.
- **Bitwise operations**: `and`, `or`, `xor`, `shl`, `lshr`, `ashr`.
- **Phi nodes**: Parse and translate LLVM phi to SAIR phi.
- **Global variables**: Direct mapping to SAIR globals.
- **Floating point**: `fadd`, `fsub`, `fmul`, `fdiv` for f64.
- **Wider types**: i64 support via cell decomposition.
- **LLVM metadata**: Ignore during parsing.
- **Alignment/volatile**: Respect on load/store.
