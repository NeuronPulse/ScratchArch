# ScratchArch Scratch Roundtrip Design

> Design version: **v0.3**
> Status: conceptual; no implementation in this milestone.

This document describes the long-term goal of making ScratchGraph a reversible
representation: a Scratch project serialized as `project.json` can be parsed back
into ScratchGraph, and from there back into SAIR. This is the foundation for a
future decompiler, importer, and optimizer for existing Scratch projects.

## Goal

```text
           compile                    export
    SAIR  -------->  ScratchGraph  ---------->  project.json
     ^                                    |
     |                                    |
     +-----------  future roundtrip  <----+
```

The forward direction is implemented today: SAIR lowers to ScratchGraph, and
exporters emit `project.json`. The reverse direction is **not** implemented in
v0.3, but ScratchGraph is being shaped so that it can represent everything
needed to recover SAIR-like semantics from a parsed project.

## Why reversibility matters

- **Decompilation**: Turn existing Scratch projects into structured IR for
  analysis, refactoring, or migration to other targets.
- **Optimization of Scratch projects**: Parse `project.json`, optimize at the
  ScratchGraph level, and re-export.
- **Bidirectional tooling**: Keep source code and generated Scratch projects in
  sync.
- **Education and verification**: Compare compiler output against hand-written
  Scratch programs by round-tripping both to the same IR.

## What ScratchGraph must preserve

For roundtripping to be possible, ScratchGraph must be able to represent:

| Concept | Forward lowering | Reverse requirement |
| ------- | ---------------- | ------------------- |
| Explicit control flow | `If`, `Repeat`, `RepeatUntil`, `Forever`, `Stop` | Recover loops and conditionals from Scratch control blocks |
| Variables | `Variable` with scope | Map Scratch variables back to SSA-like values |
| Events / scripts | `Script` with `Hat` | Parse event hats and recreate script boundaries |
| Procedures | `Procedure` | Reconstruct custom block definitions and call sites |
| Memory operations | `HeapAlloc`, `HeapLoad`, `HeapIndex`, list ops | Recognize heap idiom and recover pointer operations |
| Runtime ABI | `EnterFrame`, `PopFrame`, `FrameSet`, `FrameGet` | Optionally strip frame machinery to recover plain SAIR calls |

## Forward vs reverse challenges

### Forward is lossy by design

The compiler intentionally lowers high-level SAIR into Scratch-oriented
primitives. Some information is intentionally discarded because Scratch does not
need it:

- SAIR basic block labels become implicit in Scratch control structure.
- SSA value ids become frame offsets or variables.
- Types are erased; Scratch values are dynamically typed numbers and strings.

### Reverse must reconstruct what was lost

A future roundtrip pass would need to:

1. **Parse `project.json`** into a raw block graph (block IDs, parents, next
   pointers, inputs, fields, mutations).
2. **Build ScratchGraph** by recognizing common block patterns:
   - event hats → `Script`
   - `procedures_definition` + `procedures_prototype` → `Procedure`
   - `procedures_call` → `Stmt::Call`
   - `control_if_else` / `control_if` → `Stmt::If`
   - `control_repeat` / `control_repeat_until` / `control_forever` → loops
   - `data_setvariableto` / `data_changevariableby` → variable statements
   - list operations → list statements or heap operations
3. **Lift to SAIR** (optional, for decompilation):
   - Convert structured control flow back to basic blocks with terminators.
   - Rename variables back to SSA value ids.
   - Reconstruct types heuristically or leave values untyped with annotations.

### Recovering the runtime ABI

v0.3 introduces frame primitives (`EnterFrame`, `PopFrame`, `FrameSet`,
`FrameGet`) to implement reentrant calls. In the reverse direction, a decompiler
would likely **erase** these primitives and recover plain SAIR `Call` and
`Return` instructions:

```text
project.json
    |
    |  parse + recognize ABI idiom
    v
ScratchGraph (with frame ops)
    |
    |  ABI normalization pass
    v
ScratchGraph (without frame ops)
    |
    |  lift
    v
SAIR
```

The ABI normalization pass would match patterns such as:

- `EnterFrame` + `Call` + `FrameSet` from return slot + restore FP + `PopFrame`
  → a single `Call` statement.
- `FrameSet { offset: 1, value }` + `PopFrame` + `Stop` → a `Return`.

This keeps the decompiler from having to reason about stack layout unless it
wants to.

## Roundtrip fidelity levels

Not all roundtrips need to be identical. We define three useful levels:

| Level | Goal | What must match |
| ----- | ---- | --------------- |
| **Structural** | Rebuild a semantically equivalent ScratchGraph | Same scripts, procedures, variables, control flow |
| **Semantic** | Rebuild SAIR that executes like the original | Same observable behavior on supported inputs |
| **Bit-identical** | Re-export the same `project.json` | Same block IDs, coordinates, and layout (usually impossible and unnecessary) |

ScratchGraph targets **structural** roundtrip in the short term and **semantic**
roundtrip in the long term. Bit-identical re-export is explicitly not a goal:
many valid `project.json` files represent the same ScratchGraph project.

## Future design considerations

### Preserving debug names

A future importer should capture Scratch variable names and procedure names and
store them in ScratchGraph. This helps reconstruct meaningful SAIR value and
function names during decompilation.

### Handling Scratch-specific idioms

Some Scratch constructs have no direct SAIR equivalent today:

- `wait until` / `wait N seconds`
- `broadcast ... and wait`
- clone creation and sprite interactions
- graphic effects and motion blocks

Roundtrip can either:

- Keep these as opaque ScratchGraph statements (structural roundtrip only), or
- Extend SAIR with runtime intrinsics that model them (semantic roundtrip).

### Testing strategy

A future roundtrip test suite should:

1. Generate ScratchGraph from SAIR.
2. Export to `project.json`.
3. Parse `project.json` back to ScratchGraph.
4. Compare the two ScratchGraph projects structurally.
5. For decompilation, lower the recovered ScratchGraph back to SAIR and run the
   SAIR interpreter to verify behavior.

## Limitations of v0.3

- No parser for `project.json` exists.
- No importer or decompiler exists.
- Frame primitives are emitted only in the forward direction.
- Event hats, lists, and heap operations are forward-only; reverse recognition
  patterns are not yet defined.

## Relation to other documents

- [`docs/specification/SCRATCHGRAPH.md`](../specification/SCRATCHGRAPH.md):
  defines the ScratchGraph data model that must be rich enough for roundtrip.
- [`docs/specification/SCRATCH_ABI.md`](../specification/SCRATCH_ABI.md):
  defines the frame ABI that a future decompiler would normalize away.
- [`docs/design/SCRATCH_SCHEDULER.md`](./SCRATCH_SCHEDULER.md): the scheduler
  model would be needed to roundtrip multi-script concurrency faithfully.
