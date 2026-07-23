# SAIR — ScratchArch Intermediate Representation

> Design version: 0.1
> Status: draft for implementation

---

## 1. Motivation

ScratchArch needs an IR layer between LLVM IR and the ScratchArch ISA because the two
serve fundamentally different roles. Lowering LLVM IR directly to ISA is possible but
loses the structured information needed for optimization, type-driven code generation,
and profile-aware lowering.

SAIR exists to answer: **"What does this program mean in ScratchArch terms?"**
before answering "How do we execute it?".

### The problem with direct LLVM→ISA lowering

| Problem | Consequence |
|---|---|
| LLVM has hundreds of opcodes plus intrinsics | Direct lowering matches each individually, missing patterns |
| LLVM poison/undef semantics | Must be resolved or carried through; SAIR commits to total wrapping early |
| LLVM assumes native integer widths | SAIR can represent cell-aware decomposition explicitly |
| LLVM struct/array GEP is complex | SAIR can decompose aggregates before lowering |
| LLVM calling convention is host-ABI | SAIR uses ScratchArch ABI natively |

### What SAIR solves

1. **Structural lowering**: SAIR statements map 1:1 to ISA instructions for simple cases
2. **Type clarity**: SAIR preserves full type information in ScratchArch terms
3. **Optimization boundary**: Passes (constant folding, DCE, phi elimination) run on SAIR
4. **Independent verification**: SAIR modules validate independently of any LLVM frontend

### What SAIR does not solve

1. **Runtime dispatch**: Block dispatch is a runtime concern (ISA.md §5.1)
2. **Cell decomposition**: Multi-cell lowering is an SAIR→ISA pass concern
3. **Register allocation**: SSA virtual registers are unbounded (ISA.md §2.5)
4. **Scheduling**: Instruction reordering is future work

---

## 2. Comparison: LLVM IR → SAIR → ScratchArch ISA

```
Layer          Purpose                         Values              Control flow
─────          ───────                         ──────              ────────────
LLVM IR        Language-independent optimizer   SSA + poison/undef  Blocks + terminators + phi

SAIR           Architecture-aware IR            SSA, typed,         Blocks + terminators + phi
                                                wrapping only

ScratchArch    Architecture definition          SSA virtual         Blocks + terminators + phi
ISA            (ISA.md)                         registers

VM runtime     Executable encoding             Stack operations    Label-based dispatch
(scratcharch-  of the ISA                      + memory
vm)
```

### SAIR is not an LLVM clone

LLVM IR tracks poison, undef, `nsw`/`nuw`. SAIR commits to ScratchArch's simpler model:
- **All arithmetic is wrapping** (ISA.md §2.3). No poison flags.
- **All values are unsigned storage** with signed operations (ISA.md §2.3).
- **Cell model** is external; SAIR operates on logical typed values.
- **Memory model** matches MEMORY.md: flat, byte-addressed, little-endian.

### SAIR is not a VM bytecode

- SAIR is not executable. It must be lowered to ISA first.
- SAIR uses infinite SSA registers, not a stack.
- SAIR has phi nodes — the VM does not execute phi directly.

---

## 3. Core design decisions

### 3.1 SSA form

**Decision**: SAIR uses SSA with phi nodes.

**Rationale**: Both LLVM IR and ScratchArch ISA (§2.5) use SSA virtual registers.
SAIR preserves SSA to make LLVM lowering straightforward and enable SSA-based optimizations.

**Rejected alternatives**:
- *Stack-based IR*: Too far from LLVM; loses SSA information needed for optimization.
- *Fixed register file*: ScratchArch has unbounded virtual registers (§2.5). Fixed
  registers require premature allocation.

### 3.2 Typed values

**Decision**: Every SAIR value has a type from the ScratchArch type system.

**Rationale**: Instructions are typed. Types enable verification and informed lowering
(cell count, alignment). Matches ISA.md §3.

### 3.3 Basic blocks

**Decision**: Basic blocks with exactly one terminator, no fall-through.

**Rationale**: Matches both LLVM IR and ISA.md §5.1.

### 3.4 Phi nodes

**Decision**: Phi nodes with edge-selected semantics (ISA.md §5.3).

**Rationale**: SSA requires phi at joins. Edge semantics match ISA.md and enable
lowering via edge assignments.

### 3.5 Memory operations

**Decision**: `alloca`, `load`, `store`, `getelementptr` matching MEMORY.md §5.
**Status**: GEP implemented in v0.1, fully supported by builder, interpreter, and validation.

### 3.6 Function representation

**Decision**: Name, return type, parameter types, list of basic blocks. First block is entry.

### 3.7 Calling convention

**Decision**: ScratchArch ABI (ABI.md). Positional cell transmission, caller-save registers.

---

## 4. SAIR instruction set (v0.1)

### Arithmetic (all wrapping per ISA.md §2.3)

| SAIR | Semantics |
|---|---|
| `%d = add iN %a, %b` | `(⟦a⟧ + ⟦b⟧) mod 2^N` |
| `%d = sub iN %a, %b` | `(⟦a⟧ − ⟦b⟧) mod⁺ 2^N` |
| `%d = mul iN %a, %b` | `(⟦a⟧ · ⟦b⟧) mod 2^N` |
| `%d = div iN %a, %b` | `floor(⟦a⟧ / ⟦b⟧)`; `⟦b⟧=0` is undefined |
| `%d = rem iN %a, %b` | `⟦a⟧ − ⟦b⟧·floor(⟦a⟧/⟦b⟧)` |

### Comparison (produce i1)

| SAIR | Semantics |
|---|---|
| `%d = eq iN %a, %b` | `⟦a⟧ == ⟦b⟧` |
| `%d = lt iN %a, %b` | `⟦a⟧ < ⟦b⟧` (unsigned) |
| `%d = gt iN %a, %b` | `⟦a⟧ > ⟦b⟧` (unsigned) |

### Memory (per MEMORY.md §5)

| SAIR | Semantics |
|---|---|
| `%d = alloca T` | Allocate `sizeof(T)` bytes, return address |
| `%d = load T, ptr %p` | Read `sizeof(T)` LE bytes from `%p` |
| `store T %v, ptr %p` | Write `%v` as `sizeof(T)` LE bytes to `%p` |

### Control flow

| SAIR | Description |
|---|---|
| `br label %bb` | Unconditional branch |
| `br i1 %c, label %t, label %f` | Conditional branch |
| `ret T %v` | Return value |
| `ret void` | Return void |

### Function call

| SAIR | Description |
|---|---|
| `%d = call @f(T %a, ...)` | Call function, capture return value |

### Phi

| SAIR | Description |
|---|---|
| `%d = phi T [%v1, %bb1], ...` | Select value by incoming edge |

---

## 5. Lowering: SAIR → ScratchArch ISA

### 5.1 Approach

Lowering converts SSA form to stack operations. The reference lowerer handles
single-block functions with the operand stack as primary value carrier.

For the common case of straight-line code with each value used exactly once,
the lowering is a direct stack transformation:
- Each instruction pops its operands and pushes its result

For multi-use values, `DUP` is emitted before the second use.

For cross-block values (phi), future work will add frame-allocation-based lowering.

### 5.2 Instruction lowering table

| SAIR | ISA lowering |
|---|---|
| `%d = add T %a, %b` | `I32_ADD` (operands already on stack) |
| `%d = sub T %a, %b` | `I32_SUB` |
| `%d = mul T %a, %b` | `I32_MUL` |
| `%d = div T %a, %b` | `I32_DIV` |
| `%d = rem T %a, %b` | `I32_REM` |
| `%d = eq T %a, %b` | `EQ` |
| `%d = lt T %a, %b` | `LT` |
| `%d = gt T %a, %b` | `GT` |
| `%d = alloca T` | `CONST size; ALLOC` |
| `%d = load T, %p` | `LOAD` (addr already on stack) |
| `store T %v, %p` | `STORE` (val then addr on stack) |
| `br label %bb` | `JUMP bb` |
| `br i1 %c, t, f` | `BRANCH t, f` (cond on stack) |
| `ret T %v` | `RETURN` |
| `%d = call @f(...)` | `CALL @f` (args on stack) |

### 5.3 Phi lowering (future)

See [`PHI_LOWERING.md`](./PHI_LOWERING.md) for the detailed design.

Phi nodes will be lowered by edge assignments: before the terminator of each
predecessor block, emit the phi operand value to a shared slot. The successor
block loads from that slot at its start. The interpreter (`scratcharch-sair-interpreter`,
see [`SAIR_INTERPRETER.md`](./SAIR_INTERPRETER.md)) already implements this
semantics directly.

### 5.4 Calling convention

Per ABI.md §5:
- Arguments pushed onto the operand stack before `CALL`
- Return value is on the stack after `CALL`
- Caller-save: live values are preserved via stack discipline

---

## 6. Known limitations (v0.1)

1. **Phi lowering not yet implemented**: Single-block functions only for ISA
   lowering. Multi-block/phi/GEP programs are rejected with clear errors.
   The interpreter handles these fully; the ISA side awaits slot-based lowering.
2. **i32-only ISA arithmetic**: SAIR types support i1/i8/i16/i32/f64/ptr, but
   the ISA instruction set only implements i32 operations. Narrower types are
   truncated/extended during lowering; wider types are deferred.
3. **No struct/array types**: Aggregates are reserved for future work.
4. **No optimization passes**: Constant folding, DCE, phi elimination are future work.
5. **No cell decomposition**: i64 values on sa48 are not yet split into cells.

---

## 7. Future work

1. **Slot-based multi-block lowering**: Use frame pointer (Pick) + frame-relative
   addressing to lower phi/GEP/multi-block functions to ISA.
2. **Struct/array types**: Decompose aggregates during lowering.
3. **Cell decomposition**: Profile-aware multi-cell lowering for sa48.
4. **Optimization passes**: Constant folding, dead code elimination, phi elimination.
5. **Intrinsic lowering**: Map `memcpy`/`memset` etc. to reference expansions.
