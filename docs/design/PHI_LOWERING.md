# Phi Lowering in ScratchArch

> Document version: 0.2
> Status: implemented in `crates/scratcharch-ir/src/lower.rs`
> Companion documents: [`BACKEND_DESIGN.md`](./BACKEND_DESIGN.md), [`ISA.md`](../specification/ISA.md) §5.3

## Problem

SAIR uses SSA form with `phi` nodes. The ISA does not have a phi instruction;
phi values must be lowered to ISA instructions before execution. The ISA uses
a stack-based model, while phi requires selecting a value based on the incoming
control-flow edge.

## Approach: Edge Copy (Simultaneous Phi)

ISA.md §5.3 defines phi semantics as edge-selected reads: on entering a block
via edge `(pred, block)`, each phi in `block` takes the value from its `pred`
operand, and all phis read simultaneously. The lowerer realizes this with the
**edge copy** strategy.

### Lowering strategy

For each phi node `%r = phi T [%v1, %bb1], [%v2, %bb2], …`:

1. Assign `%r` a local slot range (see [`BACKEND_DESIGN.md`](./BACKEND_DESIGN.md)
   §3).
2. For each predecessor edge `(%bbi, %block)`:
   - emit `local.get src_slot` followed by `local.set dst_slot` for every cell
     of `%r`, where `src_slot` is the slot of `%vi`.
3. In the successor block, the phi instruction itself is lowered to nothing —
   the value already lives in `%r`'s slot. When an instruction uses `%r`, the
   lowerer emits `local.get` from that slot.

This is correct because:

- SSA form guarantees that each value has exactly one definition.
- Each predecessor block's phi source is computed before the branch.
- The copy happens on the edge, before the successor reads the slot.

### Critical edges

If a predecessor has multiple successors **and** the successor has multiple
predecessors, the edge is **critical**: there is no single basic block where
the copy sequence can live without affecting other edges. The lowerer splits
every critical edge by inserting a **trampoline block** with a single
predecessor and a single successor. Phi copies are then placed at the start of
the trampoline.

```text
Before:
        pred
       /    \
      /      \
   succ_a   succ_b
     |        |
   join     join

After (if join has multiple predecessors):
        pred
       /    \
      /      \
   trn_a   trn_b
     |        |
   succ_a   succ_b
     |        |
   join     join
```

### Cycle breaking

If phi operands form a cycle (e.g. `%a = phi [%b, ...], ...` and
`%b = phi [%a, ...], ...`), the edge copies must be **simultaneous**. A naive
lowering that emits stores in order would overwrite one value before the other
is read.

Solution: the lowerer resolves all copies on an edge as a **parallel copy**.
When the copy graph contains a cycle, it breaks the cycle using a reserved
**temp slot**:

```text
tmp   = local.get a
      local.set tmp_slot
      local.get b
      local.set a_slot
      local.get tmp_slot
      local.set b_slot
```

This is equivalent to LLVM's `DemoteRegToStack` + SSA destruction, but it runs
on SAIR during lowering rather than requiring explicit `alloca`/`load`/`store`
in the IR.

## Copy placement rules

|Edge shape|Placement|
|---|---|
|Unconditional branch to a single-predecessor successor|End of predecessor|
|Conditional branch (successor is single-predecessor after edge splitting)|Start of successor|
|Critical edge (after splitting)|Start of trampoline|

Copies placed at the end of a block run after all non-phi instructions and
immediately before the terminator. Copies placed at the start of a block run
before any non-phi instruction.

## Reference: Simultaneous Phi Semantics

Given:

```text
join:
  %r1 = phi T [%v1_a, %bb_a], [%v1_b, %bb_b]
  %r2 = phi T [%v2_a, %bb_a], [%v2_b, %bb_b]
```

On entry via `%bb_a`, the observable result is:

- `%r1` = `%v1_a`
- `%r2` = `%v2_a`

Both source values are captured before any phi slot is written. This is
LLVM-standard semantics and matches ISA.md §5.3.

## Implementation

The implementation lives in `crates/scratcharch-ir/src/lower.rs`:

- `split_critical_edges` inserts trampoline blocks.
- `collect_phi_copies` builds the per-edge copy map.
- `copy_placement` decides where each copy sequence lives.
- `resolve_parallel_copies` breaks cycles using the temp slot.
- `emit_copies` emits the final `local.get`/`local.set` sequence.

See [`BACKEND_DESIGN.md`](./BACKEND_DESIGN.md) for the full lowering pipeline
and target-aware slot allocation.
