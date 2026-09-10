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

|Problem|Consequence|
|---|---|
|LLVM has hundreds of opcodes plus intrinsics|Direct lowering matches each individually, missing patterns|
|LLVM poison/undef semantics|Must be resolved or carried through; SAIR commits to total wrapping early|
|LLVM assumes native integer widths|SAIR can represent cell-aware decomposition explicitly|
|LLVM struct/array GEP is complex|SAIR can decompose aggregates before lowering|
|LLVM calling convention is host-ABI|SAIR uses ScratchArch ABI natively|

### What SAIR solves

1. **Structural lowering**: SAIR statements map 1:1 to ISA instructions for simple cases.
2. **Type clarity**: SAIR preserves full type information in ScratchArch terms.
3. **Optimization boundary**: Passes (constant folding, DCE, phi elimination) run on SAIR.
4. **Independent verification**: SAIR modules validate independently of any LLVM frontend.

### What SAIR does not solve

1. **Runtime dispatch**: Block dispatch is a runtime concern (ISA.md §5.1).
2. **Cell decomposition**: Multi-cell lowering is an SAIR→ISA pass concern.
3. **Register allocation**: SSA virtual registers are unbounded (ISA.md §2.5).
4. **Scheduling**: Instruction reordering is future work.

---

## 2. Comparison: LLVM IR → SAIR → ScratchArch ISA

```text
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

|SAIR|Semantics|
|---|---|
|`%d = add iN %a, %b`|`(⟦a⟧ + ⟦b⟧) mod 2^N`|
|`%d = sub iN %a, %b`|`(⟦a⟧ − ⟦b⟧) mod⁺ 2^N`|
|`%d = mul iN %a, %b`|`(⟦a⟧ · ⟦b⟧) mod 2^N`|
|`%d = div iN %a, %b`|`floor(⟦a⟧ / ⟦b⟧)`; `⟦b⟧=0` is undefined|
|`%d = rem iN %a, %b`|`⟦a⟧ − ⟦b⟧·floor(⟦a⟧/⟦b⟧)`|

### Comparison (produce i1)

|SAIR|Semantics|
|---|---|
|`%d = eq iN %a, %b`|`⟦a⟧ == ⟦b⟧`|
|`%d = lt iN %a, %b`|`⟦a⟧ < ⟦b⟧` (unsigned)|
|`%d = gt iN %a, %b`|`⟦a⟧ > ⟦b⟧` (unsigned)|

### Memory (per MEMORY.md §5)

|SAIR|Semantics|
|---|---|
|`%d = alloca T`|Allocate `sizeof(T)` bytes, return address|
|`%d = load T, ptr %p`|Read `sizeof(T)` LE bytes from `%p`|
|`store T %v, ptr %p`|Write `%v` as `sizeof(T)` LE bytes to `%p`|

### Control flow

|SAIR|Description|
|---|---|
|`br label %bb`|Unconditional branch|
|`br i1 %c, label %t, label %f`|Conditional branch|
|`ret T %v`|Return value|
|`ret void`|Return void|

### Function call

|SAIR|Description|
|---|---|
|`%d = call @f(T %a, ...)`|Call function, capture return value|

### Phi

|SAIR|Description|
|---|---|
|`%d = phi T [%v1, %bb1], ...`|Select value by incoming edge|

---

## 5. Lowering: SAIR → ScratchArch ISA

### 5.1 Approach

The reference lowerer (`IsaLowerer` in `crates/scratcharch-ir/src/lower.rs`)
converts SAIR into `scratcharch-core` ISA programs. It handles multi-block
functions, phi nodes, GEP, function calls, and returns using a **local-slot
model**: every SSA value is assigned one or more frame-local slots, and
instructions load operands from / store results to those slots.

The local-slot model is the VM realization of the SSA virtual registers
 described in [`ISA.md`](../specification/ISA.md) §2.5. It makes multi-block
lowering straightforward because values can survive across block boundaries
without requiring a predictable operand-stack shape at every block entry.

### 5.2 Lowering pipeline

1. **Validate** the SAIR module.
2. **Allocate slots** for parameters and every result-producing instruction,
   using the target profile to compute cell counts.
3. **Split critical edges** so that each phi copy sequence has a unique
   placement.
4. **Collect and place phi copies** on each edge.
5. **Resolve parallel copies**, breaking cycles with a reserved temp slot.
6. **Emit ISA instructions** block by block, including prologue, phi copies,
   non-phi instructions, and terminators.

See [`BACKEND_DESIGN.md`](./BACKEND_DESIGN.md) and [`PHI_LOWERING.md`](./PHI_LOWERING.md)
for the full design.

### 5.3 Instruction lowering table

|SAIR|ISA lowering|
|---|---|
|`%d = add T %a, %b`|`local.get a; local.get b; i32.add; local.set d`|
|`%d = sub/mul/div/rem/eq/lt/gt`|analogous to `add`|
|`%d = const T v`|push constant; `local.set d`|
|`%d = alloca T`|`const_i32 sizeof(T); alloc; local.set d`|
|`%d = load T, %p`|`local.get p; load; local.set d`|
|`store T %v, %p`|`local.get p; local.get v; store`|
|`br label %bb`|`jump %bb`|
|`br i1 %c, t, f`|`local.get c; branch t f`|
|`ret T %v`|`local.get v; return`|
|`%d = call @f(...)`|evaluate args; `call @f`; `local.set d` (if non-void)|
|`%d = gep T, %base, %idx`|`local.get base; local.get idx; const_i32 sizeof(T); i32.mul; i32.add; local.set d`|
|`%d = phi T [...]`|lowered via edge copies; no direct instruction|

### 5.4 Phi lowering

Phi nodes are lowered by **edge-based copy insertion**. For each incoming edge
to a phi block, the lowerer emits `local.get`/`local.set` copies from the
operand slot(s) to the phi result slot(s). Critical edges are split with
trampoline blocks so that each edge has a unique copy location. Cyclic phi
operands are resolved using a temp slot so that all source values are read
before any destination is written. See [`PHI_LOWERING.md`](./PHI_LOWERING.md)
for details.

### 5.5 Calling convention

Per ABI.md §5:

- Arguments pushed onto the operand stack before `CALL`.
- Return value cells are on the stack after `CALL`.
- The callee prologue pops parameter cells into local slots.
- Caller-save: live values are preserved via local slots.

---

## 6. Known limitations (v0.1)

1. **Multi-cell execution**: The ISA lowerer decomposes wide values through the
   target profile, and on `sa48` a 64-bit value is two 32-bit limbs.
   add/sub/compare/cast/load/store/select/phi run per limb; bitwise ops run as
   per-limb word ops; every shift and full-width `mul`/`udiv`/`urem`/`sdiv`/
   `srem` is realised by a demand-appended program-level software helper
   (`__sair_shl64`/`__sair_lshr64`/`__sair_ashr64`, `__sair_mul64`,
   `__sair_udivrem64`) built from the word ops — no widening ISA was needed
   (EXECUTION_MODEL.md §5.7–§5.8).
2. **No struct/array types as SAIR values**: Aggregates can be manipulated
   through pointers and GEP, but they are not first-class SAIR value types.
3. **Runtime intrinsics resolve on both engines**: `__scratcharch_memcpy`,
   `__scratcharch_strlen`, and the other runtime routines are dispatched by the
   SAIR interpreter at run time and by the VM at load time, both through the
   shared `scratcharch-runtime` `IntrinsicRegistry` (RUNTIME.md §4/§6). The VM
   rewrites the bodyless named `Call` into a `Code::CallRuntime` entry; an
   unknown name is rejected at load.
4. **No native code emission**: The backend emits `scratcharch-core` ISA, not
   host machine code or TurboWarp blocks.

---

## 7. Future work

1. **Multi-cell arithmetic**: ✅ Landed — the lowering table emits multi-cell
   add/sub/compare/cast/load/store for types wider than a cell and realises
   wide shifts and full-width `mul`/`div`/`rem` through demand-appended
   software helpers (EXECUTION_MODEL.md §5.7–§5.8).
2. **Struct/array types**: Decompose aggregates into cells during lowering.
3. **Intrinsic lowering**: Map runtime intrinsics to ISA call sequences or
   reference expansions in the VM backend. *(Landed as load-time runtime
   resolution — RUNTIME.md §6 — rather than as ISA sequences: the ISA stays
   frozen and no libc-specific instruction was added.)*
4. **Native backends**: Reuse the lowerer's slot allocation and phi destruction
   while emitting host code or TurboWarp blocks instead of `local.get`/`local.set`.
