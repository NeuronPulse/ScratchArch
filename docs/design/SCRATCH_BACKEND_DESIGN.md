# ScratchArch Scratch Backend Design

This document describes the Scratch-specific backend layer introduced in
**ScratchArch Scratch Backend Foundation v0.1**, extended in **v0.2** and
**v0.3**, and matured in **v0.4**. The goal of this layer is to target Scratch
without coupling Scratch serialization formats to the compiler core.

## Architecture

```text
┌─────────────┐     ┌──────────────────────┐     ┌─────────────────────┐
│    SAIR     │ --> │   ScratchGraph IR    │ --> │ ScratchExporter impl│
│  (generic)  │     │ (Scratch semantics)  │     │  (JSON / sb3 / ...) │
└─────────────┘     └──────────────────────┘     └─────────────────────┘
        ^                      |                            |
        |                      v                            v
        |            scratcharch-analyzer           project.json / sb3
        |                      |
        +------ project.json -> ScratchGraph parser
```

### Why a dedicated backend layer?

- **Format isolation**: Scratch's canonical representation (`project.json`)
  changes over time and is tied to the Scratch VM. A semantic IR lets us
  generate multiple output formats from the same compiler pipeline.
- **Testing**: We can test the compiler pipeline without needing a full
  Scratch VM or rendering engine.
- **Reuse**: Future frontends can reuse SAIR and lower to ScratchGraph without
  knowing JSON details.

### Crate separation

| Crate                        | Scratch-aware? | Role                                                 |
| ---------------------------- | -------------- | ---------------------------------------------------- |
| `scratcharch-core`           | No             | ISA-level types and VM instructions                  |
| `scratcharch-ir`             | No             | SAIR SSA IR, builder, validator, ISA lowerer         |
| `scratcharch-vm`             | No             | Executes core ISA programs                           |
| `scratcharch-scratchgraph`   | **Yes**        | Semantic Scratch IR, SAIR→ScratchGraph lowering,     |
|                              |                | runtime abstraction, and project.json parser         |
| `scratcharch-analyzer`       | **Yes**        | Static analyses over ScratchGraph                    |
| exporter implementations     | **Yes**        | Translate ScratchGraph into concrete formats         |

No crate below `scratcharch-scratchgraph` may depend on Scratch concepts.

## ScratchGraph IR

`ScratchGraph` models Scratch programs as nested Rust structures:

- `Project` → `Stage` + `Sprite`s
- `Stage` / `Sprite` → variables, lists, broadcasts, scripts, procedures
- `Script` → `Hat` (event) + body of `Stmt`
- `Procedure` → prototype + body of `Stmt`
- `Stmt` → statements (set variable, call, if, repeat, stop, ...)
- `Expr` → expressions (literals, variables, operators, procedure params)

See [`docs/specification/SCRATCHGRAPH.md`](../specification/SCRATCHGRAPH.md) for
the full data model and rationale.

### Concurrent scripts

A Scratch program is not a single control-flow graph. Each sprite (and the
stage) may contain multiple independent scripts, each triggered by its own
event hat:

```text
Project
└── Sprite
    ├── Script { hat: GreenFlag, ... }
    ├── Script { hat: KeyPressed("space"), ... }
    └── Script { hat: BroadcastReceived("go"), ... }
```

ScratchGraph models this directly: `Sprite::scripts` is a vector of `Script`
values. The SAIR→ScratchGraph lowerer currently emits only the entry function
as a green-flag script on the stage; future frontends can populate additional
sprite scripts by constructing `Project` values directly or extending the
lowering pipeline.

## Lowering boundary: SAIR → ScratchGraph

### Functions → procedures

Every SAIR `IrFunction` becomes a ScratchGraph `Procedure` on the stage.
The function name becomes the procedure name.

```text
SAIR function `foo`  -->  Stage procedure `foo`
```

Parameters are mapped to `ProcedureParam` values. Scratch custom blocks do not
have typed parameters in v0.1, so parameters are treated as string/number
reporters.

### Entry function → green-flag script

The module's entry function is invoked by a generated green-flag script.
Because Scratch green-flag scripts cannot take arguments, any entry-function
parameters are passed as zero literals in v0.1. This is acceptable for
architecture tests but will be extended later.

```text
SAIR module.entry  -->  Script { hat: GreenFlag, body: [Call entry, Stop] }
```

### Variables → frame slots

SAIR is SSA form: each `ValueId` represents a single definition. ScratchGraph
v0.3 maps non-parameter SSA values to frame slots inside a runtime call stack,
not to individual Scratch variables. Each function has a frame size of
`2 + L`, where `L` is the number of non-parameter SSA values:

| Frame offset | Content                    |
| ------------ | -------------------------- |
| 0            | Saved caller frame pointer |
| 1            | Return value slot          |
| 2 ..         | Non-parameter SSA values   |

```text
SAIR value %5  -->  FrameGet { offset: 2 + (5 - param_count) }
```

Parameters are passed through Scratch custom-block inputs and read with
`ProcedureParam` expressions, so they do not occupy frame slots.

Compiler-generated runtime values such as `__scratcharch_fp` and the runtime
stack list `__scratcharch_stack` are declared on the stage as temporary
variables and lists. User-visible variables can still be modeled directly with
`Variable` and `SetVariable` when a frontend constructs ScratchGraph by hand.

Phi nodes become `FrameSet` updates on incoming control-flow edges, copying the
incoming value into the merge block's phi slot. This aligns with Scratch's
sequential execution model while preserving SSA edge semantics.

### Calls → custom block calls

A SAIR `Call` instruction becomes a frame push, a `Stmt::Call` to the
corresponding procedure, a copy of the per-frame return slot, and a frame pop.

```text
%r = call @add(%a, %b)
  -->  Stmt::EnterFrame { slots: add_frame_size }
       Stmt::Call { proc: "add", args: [a, b] }
       Stmt::FrameSet { offset: offset(%r), value: FrameGet { offset: 1 } }
       Stmt::SetVariable { var: "__scratcharch_fp", value: FrameGet { offset: 0 } }
       Stmt::PopFrame { slots: add_frame_size }
```

Scratch custom blocks do not return values natively. v0.3 models return values
with a per-call frame slot in the runtime call stack:

- The caller pushes a frame for the callee with `EnterFrame`.
- The callee writes its return value to frame offset 1 before popping its locals
  and stopping.
- The caller copies frame offset 1 into its own result SSA slot, restores the
  frame pointer from the saved FP at offset 0, and pops the callee's frame.

This convention is purely a ScratchGraph concern and does not leak into SAIR.
It is **re-entrant**: recursion and nested calls each have their own return
slot and local slots because every activation occupies a separate region of
`__scratcharch_stack`.

See [`docs/specification/SCRATCH_ABI.md`](../specification/SCRATCH_ABI.md) for
the full frame layout and calling convention.

### Control flow → Scratch control blocks

| SAIR construct                          | ScratchGraph construct                               |
| --------------------------------------- | ---------------------------------------------------- |
| `br` to next block                      | fall-through (no explicit block)                     |
| `ret`                                   | `Stmt::Stop { ThisScript }`                          |
| diamond `cond_br`                       | `Stmt::If { condition, then_body, else_body }`       |
| natural loop (`repeat until` pattern)   | `Stmt::RepeatUntil { condition, body }`              |

The lowerer performs structural analysis on the SAIR CFG:

1. Walk blocks starting from the entry block.
2. On a conditional branch, attempt to recognize a natural loop by checking
   whether one successor can reach the current block again (the back edge)
   without passing through the other successor or already-visited blocks.
3. If a loop is found, recursively lower the loop body with the header added
   to the visited set. This allows nested `if/else` and nested natural loops
   inside the body.
4. If it is not a loop, treat it as an `if/else`. Recursively lower each
   branch and find the common merge block.
5. Phi nodes are skipped during direct instruction emission; their values are
   materialized through variable updates on the incoming edges.

Irreducible CFGs and loops with multiple exits remain unsupported.

### Memory and runtime features

v0.2 introduces a ScratchGraph heap abstraction backed by a single stage list
named `__scratcharch_heap`. Pointers are 0-based indices into this list;
Scratch's list blocks are 1-indexed, so the exporter adds one at the boundary.

| SAIR instruction | ScratchGraph representation                                           |
| ---------------- | --------------------------------------------------------------------- |
| `alloca T, N`    | `HeapAlloc` of `size_in_bytes(T) * N` cells; result = old heap length |
| `load`           | `HeapLoad` at pointer                                                 |
| `store`          | `SetListItem` on `__scratcharch_heap` at pointer + 1                  |
| `gep`            | `HeapIndex` adding byte offsets                                       |

See [`docs/specification/SCRATCH_MEMORY.md`](../specification/SCRATCH_MEMORY.md)
for the full memory model and its limitations.

Runtime library functions (`memcpy`, `strlen`, etc.) are ordinary SAIR functions
and lower to procedures like any other function. They are not special-cased in
the backend.

## Exporter abstraction

`ScratchExporter` is a trait that converts a `Project` into a concrete output:

```rust
pub trait ScratchExporter {
    type Output;
    type Error: core::fmt::Display;
    fn export(&self, project: &Project) -> Result<Self::Output, Self::Error>;
}
```

Implementations live in separate modules:

- `JsonExporter` → `serde_json::Value` (Scratch 3 `project.json`)
- Future: `Sb3Exporter` → ZIP archive bytes
- Future: `ScratchblocksExporter` → plain text

The exporter is the only component that knows JSON opcodes, block IDs, and
mutation fields.

## Future work

- Sprite-level procedures and variables instead of placing everything on the
  stage.
- Integration with the driver crate so users can run
  `LLVM IR → SAIR → ScratchGraph → project.json` in one command.
- Full C memory model with alignment, padding, `free`, and `realloc`.
- Additional exporters (`sb3`, `scratchblocks`).
- Scratch scheduler implementation for multi-script concurrency and cooperative
  yielding (see [`SCRATCH_SCHEDULER.md`](./SCRATCH_SCHEDULER.md)).
- Roundtrip parsing: `project.json → ScratchGraph → SAIR` for decompilation
  (see [`SCRATCH_ROUNDTRIP.md`](./SCRATCH_ROUNDTRIP.md)).
- Extend `ProjectParser` to cover more Scratch 3 blocks and mutation shapes.
- Use `scratcharch-analyzer` passes to optimize ScratchGraph before export.
- Implement a real Scratch VM interpreter on top of `RuntimeState`.
