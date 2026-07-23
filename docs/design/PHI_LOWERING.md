# Phi Lowering in ScratchArch

> Document version: 0.1
> Status: design notes for future implementation

## Problem

SAIR uses SSA form with `phi` nodes. The ISA does not have a phi instruction;
phi values must be lowered to ISA instructions before execution. The ISA uses
a stack-based model, while phi requires selecting a value based on the
incoming control-flow edge.

## Approach: Edge Copy (Simultaneous Phi)

ISA.md §5.3 defines phi semantics as edge-selected reads: on entering a block
via edge `(pred, block)`, each phi in `block` takes the value from its `pred`
operand, and all phis read simultaneously. This is also known as the **edge
copy** lowering.

### Lowering strategy

For each phi node `%r = phi T [%v1, %bb1], [%v2, %bb2], …`:

1. At the end of each predecessor block `%bb_i`, before its terminator,
   insert: `store T %v_i, ptr %slot_r`
2. In the successor block, the phi itself is lowered to nothing — the value
   already lives in `%slot_r`. When the first non-phi instruction uses `%r`,
   it loads from `%slot_r`.

This is correct because:

- SSA form guarantees that each value has exactly one definition
- Each predecessor block's phi source is computed before the branch
- The store happens before control transfer, so the slot is ready

### Cycle breaking

If phi operands form a cycle (e.g. `%a = phi [%b, ...], ...` and
`%b = phi [%a, ...], ...`), the edge stores must be **simultaneous**.
A naive lowering that emits stores in order would overwrite one value before
the other is read.

Solution: insert a **temporary copy** for each cyclic value:

```
tmp = %a
store %b → slot_b
store tmp → slot_a
```

This is equivalent to LLVM's `DemoteRegToStack` + `SSA destruction`.

### Current status in lower.rs

The reference `IsaLowerer` (in `lower.rs`) uses stack-based lowering for
single-block functions and rejects phi nodes with `LowerError::PhiNotSupported`.
Full phi lowering is deferred to the slot-based lowering strategy described
in `SAIR_INTERPRETER.md`.

## Reference: Simultaneous Phi Semantics

Given:
```
join:
  %r1 = phi T [%v1_a, %bb_a], [%v1_b, %bb_b]
  %r2 = phi T [%v2_a, %bb_a], [%v2_b, %bb_b]
```

On entry via `%bb_a`, the observable result is:
- `%r1` = `%v1_a`
- `%r2` = `%v2_a`

Both values are captured before any phi is written. This is LLVM-standard
semantics and matches ISA.md §5.3.

## Implementation Plan

1. Compute liveness: identify values that cross block boundaries
2. Assign each such value a stack frame slot (via `alloca`)
3. For each phi:
   - In predecessor blocks: `store` source to phi slot before terminator
   - In successor block: replace phi uses with `load` from phi slot
4. Handle cycles with temporary copies
5. After phis are lowered, blocks can be lowered independently
