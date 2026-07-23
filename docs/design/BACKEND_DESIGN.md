# ScratchArch Backend Design

> Design version: 0.1
> Status: implemented for the "ScratchArch Backend Foundation v0.1" milestone
> Companion documents: [`PHI_LOWERING.md`](./PHI_LOWERING.md), [`SAIR_DESIGN.md`](./SAIR_DESIGN.md), [`EXECUTION_MODEL.md`](../specification/EXECUTION_MODEL.md), [`ISA.md`](../specification/ISA.md)

---

## 1. Motivation

The ScratchArch backend converts SAIR (the architecture-aware intermediate
representation) into executable ISA programs. The v0.1 backend has two goals:

1. **Completeness**: lower the full v0.1 SAIR subset — multi-block functions,
   phi nodes, GEP, function calls, and returns — to the ISA accepted by
   `scratcharch-vm`.
2. **Target awareness**: use `scratcharch-target` profiles to decide how
   logical SAIR types decompose into cells and how function parameters and
   return values are laid out.

The backend deliberately avoids Scratch-specific constructs. It is a generic
 lowering pass that could feed a native VM, an interpreter, or a future
generator for another runtime.

---

## 2. Architecture overview

```text
+-----------+     +-----------------+     +------------------+
| LLVM IR   | --> |  SAIR module    | --> |  SAIR optimizer  |
+-----------+     +-----------------+     +------------------+
                                                   |
                                                   v
                                          +------------------+
                                          |  IsaLowerer      |
                                          |  (scratcharch-ir)|
                                          +------------------+
                                                   |
                                                   v
                                          +------------------+
                                          |  Program         |
                                          |  (scratcharch-core)
                                          +------------------+
                                                   |
                                                   v
                                          +------------------+
                                          |  scratcharch-vm  |
                                          +------------------+
```

The `IsaLowerer` is the central component. It is intentionally separate from
the VM so that:

- The same lowerer can be reused by future backends.
- The VM remains a pure execution engine.
- Tests can inspect the generated ISA program independently of execution.

---

## 3. The local-slot model

### 3.1 Why locals?

SAIR uses infinite SSA virtual registers. The ISA also describes SSA virtual
registers in [`ISA.md`](../specification/ISA.md) §2.5. The VM, however, is
stack-based: each instruction consumes operands from and pushes results onto an
operand stack.

For straight-line, single-use values this maps directly to the stack. But
multi-block control flow and phi nodes need values to survive across blocks,
where the operand stack shape at block entry is hard to predict. The VM
therefore provides **first-class local slots**: per-function, per-activation
storage locations accessed by `local.get <slot>` and `local.set <slot>`.

Locals are not a VM hack; they are the VM realization of the stable storage
that SSA destruction requires. They are also the natural place for:

- Phi results after SSA elimination.
- Values with multiple uses (avoiding repeated stack shuffling).
- Future multi-cell values and aggregate decomposition.
- Caller-save values around function calls.

### 3.2 Slot allocation

For each function, `IsaLowerer` allocates a contiguous range of slots indexed
from `0`:

1. **Parameters**: each parameter occupies `cell_count(param_type)` slots.
2. **SSA values**: each result-producing instruction occupies
   `cell_count(result_type)` slots, allocated in `ValueId` order.
3. **Temp slot**: one extra slot used to break phi copy cycles.

The number of slots is recorded in `Function::local_count`. The number of
parameter cells is recorded in `Function::param_cells`, and the number of
return cells in `Function::return_cells`.

### 3.3 Mapping values to slots

`LowerCtx` maintains:

- `first_slot[value_id]`: the first slot index of the value.
- `value_cells[value_id]`: how many consecutive slots the value occupies.

Emitting a load for a value pushes all of its cells onto the operand stack
(least-significant cell first, matching the little-endian cell order defined in
[`ABI.md`](../specification/ABI.md) §2). Emitting a store pops cells from the
stack and writes them into the value's slots.

---

## 4. Lowering pipeline

`IsaLowerer::lower_function` runs the following steps:

### 4.1 Validate

`IrModule::validate()` is run before any per-function work. Invalid SAIR is
rejected early with a clear error.

### 4.2 Compute result ids

Because phi nodes are identified by their position in a block, the lowerer
builds a map from `(block_label, instruction_index)` to the global `ValueId`
produced by that instruction. This allows phi copy collection to locate the
slot range of a phi result.

### 4.3 Allocate lowering context

`LowerCtx::new` walks the function's `values` array (parameters first, then all
result-producing instructions in block order) and assigns slots using
`cells_for_type` from the target profile.

### 4.4 Split critical edges

A **critical edge** is an edge from a block with multiple successors to a block
with multiple predecessors. Edge-based phi lowering needs to insert copy code
on each edge; if an edge is critical, that code cannot be placed uniquely in
either endpoint. The lowerer therefore splits every critical edge by inserting
a trampoline block:

```text
      pred
     /    \
    /      \
succ_a    succ_b
```

becomes:

```text
      pred
     /    \
    /      \
trn_a    trn_b
  |        |
succ_a  succ_b
```

The trampoline block has a single predecessor and a single successor, so phi
copies can be placed unambiguously at its start.

### 4.5 Collect phi copies

For each phi node `%phi = phi T [%v1, %bb1], [%v2, %bb2], ...`:

- For each incoming edge `(%bbi, %bb_phi)`, emit one `Copy { dst: phi_slot + offset, src: vi_slot + offset }` per cell.

All copies for a given edge are collected together and lowered as a single
parallel copy sequence.

### 4.6 Place copies

Copy placement follows the edge semantics:

- **Unconditional branch to single-predecessor successor**: copies go at the end
  of the predecessor.
- **Conditional branch**: copies go at the start of the successor (the successor
  has only one predecessor because critical edges were split).
- **Trampoline block**: copies go at the start of the trampoline.

This guarantees that copies execute exactly once per edge taken and do not
interfere with the terminator or other blocks.

### 4.7 Resolve parallel copies

Copies on the same edge are **simultaneous**: all sources must be read before
any destination is written. If the copy graph contains cycles (e.g. two phis
swap values), the resolver breaks one cycle at a time by saving one value into
the temp slot, copying the rest, and then restoring from the temp slot.

### 4.8 Emit instructions

Each basic block is emitted in order:

1. Set the block label on the next instruction.
2. Emit incoming phi copies (if any) at the block start.
3. Lower each non-phi SAIR instruction to ISA instructions.
4. Emit outgoing phi copies (if any) before the terminator.
5. Lower the terminator.

### 4.9 Function prologue

The first instructions of every function pop the parameter cells from the
operand stack into local slots in reverse order, so that the first argument
ends up in the lowest slot(s) for parameter `0`.

---

## 5. Instruction lowering

|SAIR|ISA lowering|
|---|---|
|`%d = add i32 %a, %b`|`local.get a; local.get b; i32.add; local.set d`|
|`%d = sub/mul/div/rem/eq/lt/gt`|analogous to `add`|
|`%d = const T v`|push constant; `local.set d`|
|`%d = alloca T`|`const_i32 sizeof(T); alloc; local.set d`|
|`%d = load T, %p`|`local.get p; load; local.set d`|
|`store T %v, %p`|`local.get p; local.get v; store`|
|`%d = call @f(...)`|evaluate args; `call @f`; `local.set d` (if non-void)|
|`%d = gep T, %base, %idx`|`local.get base; local.get idx; const_i32 sizeof(T); i32.mul; i32.add; local.set d`|
|`%d = phi T [...]`|lowered via edge copies; no direct instruction|
|`br %bb`|`jump %bb`|
|`cond_br %c, %t, %f`|`local.get c; branch %t %f`|
|`ret %v`|`local.get v; return`|
|`ret void`|`return`|

The `store` instruction emits the address before the value because the VM
expects the value on top of the operand stack and the address underneath it.

---

## 6. SSA destruction

SAIR is in strict SSA form. The ISA execution model does not have phi
instructions, so the lowerer performs **SSA destruction** before instruction
selection.

### 6.1 Strategy: edge-based copy insertion

The chosen strategy is the edge copy (simultaneous phi) approach described in
[`PHI_LOWERING.md`](./PHI_LOWERING.md):

- For every incoming edge to a phi block, copy each phi operand into the phi's
  slot.
- In the phi block, the phi value is already in its slot; uses load from it.

This matches the ISA phi semantics in [`ISA.md`](../specification/ISA.md) §5.3:
phi operands are selected on the control-flow edge and read simultaneously.

### 6.2 Correctness argument

- SSA guarantees each value has exactly one definition.
- Each predecessor block computes its phi operand before the branch.
- Copy placement ensures the write happens after the operand is computed and
  before the successor reads it.
- Critical-edge splitting ensures each edge has a unique location for copies.
- Parallel-copy resolution preserves the simultaneous-read semantics for cyclic
  phi operands.

### 6.3 Comparison with alternatives

|Approach|Why not used|
|---|---|
|LLVM `DemoteRegToStack` + mem2reg|Requires explicit alloca/load/store in SAIR; keeps SAIR cleaner to do it during lowering.|
|Pure stack lowering without locals|Becomes complex for multi-use and cross-block values; locals are a stable, testable abstraction.|
|Native phi instruction|Would expose phi to every backend and runtime; edge copies keep the ISA simpler.|

---

## 7. Target interaction

### 7.1 Target profile

`IsaLowerer` is constructed with a `TargetProfile`. The reference profile is
`sa48` (48-bit cell, 32-bit pointer, little-endian, modular wrapping). All
cell-count decisions go through `TargetProfile::cells_for_type`.

### 7.2 Type decomposition

For a value of type `T`:

```text
cell_count(T) = ceil((sizeof(T) * 8) / cell_width)
```

Current SAIR types (`i1`, `i8`, `i16`, `i32`, `f64`, `ptr`, `void`) are all
single-cell on `sa48`. The lowerer verifies single-cell execution for all
arithmetic and load/store operations; wider types would decompose naturally by
emitting multiple `local.get`/`local.set` operations and multi-cell arithmetic
sequences.

### 7.3 Parameter and return decomposition

- A parameter of `N` cells occupies `N` consecutive slots starting at slot `0`.
- The function prologue pops exactly `param_cells` values from the operand stack.
- On return, the function pushes exactly `return_cells` values onto the operand
  stack.
- The caller of a non-void function stores the returned cells into the result
  slot(s).

This is verified in tests using an artificial `sa16` profile where `i32` is
two cells: `param_cells == 2` and `return_cells == 2`.

### 7.4 No new SAIR types

The milestone deliberately did **not** add `i64` or aggregate types to SAIR.
Multi-cell support is implemented through generic `cell_count` logic in the
lowerer, so it is ready for wider types when the architecture requires them,
but the v0.1 IR surface remains unchanged.

---

## 8. Calling convention

The v0.1 calling convention is a simple stack-based ABI:

1. Caller evaluates each argument and pushes its cells onto the operand stack.
2. Caller executes `call @f`.
3. The VM creates a new frame with `local_count` slots and initializes the
   first `param_cells` slots from the operand stack (via the function prologue).
4. Callee executes and pushes `return_cells` values onto its operand stack
   before `return`.
5. The VM returns control to the caller, leaving the return cells on the
   caller's operand stack.
6. Caller stores the return cells into the result slot(s).

This matches the ScratchArch ABI in [`ABI.md`](../specification/ABI.md) §5:
positional cell transmission, caller-save discipline.

---

## 9. Future native backends

The `IsaLowerer` is designed so that future backends can reuse most of the
pipeline:

- **Slot allocation and phi destruction** are target-independent.
- **Instruction selection** can be swapped by replacing the `lower_instr` step.
- **Native register backends** can treat slots as virtual registers and run a
  register allocator instead of emitting `local.get`/`local.set`.
- **Multi-cell arithmetic** can be expanded in the instruction-lowering table
  without changing SAIR.

A future TurboWarp or native-code backend would likely:

1. Reuse `LowerCtx` to know which values need stable storage.
2. Emit its own prologue/epilogue instead of `local.get`/`local.set`.
3. Keep the same critical-edge splitting and parallel-copy resolution.

---

## 10. Known limitations

- **Single-cell execution**: arithmetic and load/store currently require
  `cell_count == 1`. Multi-cell integer operations compile-time error with a
  clear message; the metadata path is verified by artificial profiles.
- **Runtime intrinsics**: `__scratcharch_memcpy`, `__scratcharch_strlen`, etc.
  are dispatched by the SAIR interpreter. The VM backend does not yet lower or
  link runtime calls; programs that call runtime intrinsics fail to lower.
- **GEP struct fields**: dynamic struct-field offsets require type layout
  information not yet plumbed through the lowerer.
- **No native code emission**: the backend emits `scratcharch-core` ISA, not
  host machine code or TurboWarp blocks.

---

## 11. Verification

The backend is tested at multiple levels:

- Unit tests in `crates/scratcharch-ir/tests/ir_tests.rs`: diamond phi,
  conditional branches, loop phi, nested branches, GEP, factorial, fib,
  target-aware slot allocation.
- VM unit tests in `crates/scratcharch-vm/tests/vm_tests.rs`: locals,
  control flow, calls, recursion.
- End-to-end driver tests in `crates/scratcharch-driver/tests/vm_backend_tests.rs`:
  LLVM IR → SAIR → optimization → ISA → VM for the C compatibility programs.

All tests run with `cargo test`; zero warnings are enforced by `cargo clippy`.
