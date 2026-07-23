# ScratchArch Scratch Backend Design

This document describes the Scratch-specific backend layer introduced in
**ScratchArch Scratch Backend Foundation v0.1** and extended in **v0.2**. The
goal of this layer is to target Scratch without coupling Scratch serialization
formats to the compiler core.

## Architecture

```text
┌─────────────┐     ┌──────────────────────┐     ┌─────────────────────┐
│    SAIR     │ --> │   ScratchGraph IR    │ --> │ ScratchExporter impl│
│  (generic)  │     │ (Scratch semantics)  │     │  (JSON / sb3 / ...) │
└─────────────┘     └──────────────────────┘     └─────────────────────┘
                                                            |
                                                            v
                                                    project.json / sb3
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
| `scratcharch-scratchgraph`   | **Yes**        | Semantic Scratch IR and SAIR→ScratchGraph lowering   |
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

### Variables → Scratch variables

SAIR is SSA form: each `ValueId` represents a single definition. ScratchGraph
maps every non-parameter SSA value to a variable. The variable name is `v{id}`
by default, or the value's debug name if present. Because SAIR currently has no
module-level globals, these compiler-generated values are placed on the stage
and marked `Global` for now.

```text
SAIR value %5  -->  Stage variable "v5"
```

ScratchGraph also distinguishes variable scope so future frontends can place
variables on sprites:

| Scope         | Owner            | Typical use                              |
| ------------- | ---------------- | ---------------------------------------- |
| `Global`      | Stage            | Shared across all sprites and the stage  |
| `SpriteLocal` | A single sprite  | Owned by one sprite                      |
| `Temporary`   | Stage (usually)  | Compiler-generated hidden state          |

Phi nodes and stores become variable updates, which aligns with Scratch's
variable-centric execution model.

### Calls → custom block calls

A SAIR `Call` instruction becomes a `Stmt::Call` to the corresponding
procedure.

```text
%r = call @add(%a, %b)
  -->  Stmt::Call { proc: "add", args: [a, b] }
       Stmt::SetVariable { var: "%r", value: Variable("__ret_add") }
```

Scratch custom blocks do not return values natively. v0.2 models return values
with a hidden stage variable named `__ret_<func>`:

- The callee writes its return value to `__ret_<func>` immediately before
  `Stop`.
- The caller copies `__ret_<func>` into its own result SSA variable right
  after the `Call`.

This convention is purely a ScratchGraph concern and does not leak into SAIR.
It is not re-entrant: recursion or concurrent calls to the same function will
overwrite the slot.

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
- Re-entrant procedure return values (e.g. per-call frame slots or a stack).
- Full C memory model with alignment, padding, `free`, and `realloc`.
- Additional exporters (`sb3`, `scratchblocks`).
