# ScratchGraph Specification

## Purpose

ScratchGraph is a semantic intermediate representation (IR) for Scratch
programs used by the ScratchArch compiler stack. It exists so that the
compiler can target Scratch without being tied to a specific serialization
format such as Scratch 3 `project.json`, the `sb3` archive layout, or the
Scratch VM's internal AST.

### Why not generate project.json directly?

- `project.json` is a flat, ID-oriented serialization format. It contains
  block IDs, parent/next pointers, and VM-specific mutation objects that are
  tedious to generate and test directly from compiler IR.
- Different Scratch consumers use different formats: the online editor expects
  `project.json`, the offline editor expects `sb3`, and educational tools often
  use `scratchblocks` text.
- A semantic IR makes it possible to write exporter tests, optimizations, and
  alternative backends without parsing JSON.

## Relation to project.json

`project.json` is a serialization of a Scratch project. ScratchGraph is a
semantic model of the same project. The mapping is many-to-one:

```text
ScratchGraph project
        |
        |  JsonExporter
        v
   project.json
```

Multiple valid `project.json` documents can represent the same ScratchGraph
project (e.g. different block IDs or coordinates). The exporter chooses one
canonical encoding.

## Relation to the Scratch VM AST

The Scratch VM parses `project.json` into a runtime object graph with blocks,
threads, and targets. ScratchGraph sits at a higher level of abstraction:

| Scratch VM concept        | ScratchGraph concept                     |
| ------------------------- | ---------------------------------------- |
| Target (stage/sprite)     | `Stage` / `Sprite`                       |
| Block stack               | `Script`                                 |
| Custom block definition   | `Procedure`                              |
| Variable                  | `Variable`                               |
| List                      | `List`                                   |
| Broadcast message         | `Broadcast`                              |
| Block opcode              | `Stmt` / `Expr` variant + opcode string  |

ScratchGraph does not model block IDs, coordinates, execution threads, or
runtime state. Those belong to the exporter or the VM.

## Data model

### Project

A Scratch project consists of a single stage and zero or more sprites.

```rust
pub struct Project {
    pub stage: Stage,
    pub sprites: Vec<Sprite>,
}
```

### Stage and Sprite

Both stage and sprite contain scripts, procedures, variables, and lists. Only
the stage can hold broadcasts in Scratch, so broadcasts live on the stage. A
sprite may own multiple independent scripts, each with its own event hat.

```rust
pub struct Stage {
    pub name: String,
    pub variables: Vec<Variable>,
    pub lists: Vec<List>,
    pub broadcasts: Vec<Broadcast>,
    pub scripts: Vec<Script>,
    pub procedures: Vec<Procedure>,
}

pub struct Sprite {
    pub name: String,
    pub variables: Vec<Variable>,
    pub lists: Vec<List>,
    pub scripts: Vec<Script>,
    pub procedures: Vec<Procedure>,
}
```

### Scripts and Hats

A script is a stack of statements triggered by an event hat. The optional
`name` field is metadata for the compiler and is not serialized to
`project.json`. v0.4 introduces `ScriptEntry` as the clear entry point for a
script, separating event hats from procedure definitions.

```rust
pub struct Script {
    pub entry: ScriptEntry,
}

pub struct ScriptEntry {
    pub hat: EventHat,
    pub name: Option<String>,
    pub body: Vec<Stmt>,
}

pub enum EventHat {
    GreenFlag,
    KeyPressed(String),
    SpriteClicked,
    BroadcastReceived(String),
    CloneStart,
}
```

### Procedures

A procedure is a reusable block definition with named parameters. Scratch
custom blocks cannot return values natively; ScratchGraph v0.3 models return
values through a per-call frame slot in the runtime call stack (see
[`SCRATCH_ABI.md`](./SCRATCH_ABI.md)).

```rust
pub struct Procedure {
    pub prototype: ProcedurePrototype,
    pub body: Vec<Stmt>,
    /// Total frame size = 2 (saved FP + return slot) + local_count.
    pub frame_size: u32,
}

pub struct ProcedurePrototype {
    pub name: String,
    pub params: Vec<ProcedureParam>,
}
```

### Variables and Lists

Variables and lists carry a scope so exporters know which target owns them.
Compiler-generated temporaries are typically stored on the stage for
convenience but are marked as `Temporary`.

```rust
pub enum VariableScope {
    Global,
    SpriteLocal,
    Temporary,
}

pub struct Variable {
    pub id: String,
    pub name: String,
    pub scope: VariableScope,
}

pub enum ListScope {
    Global,
    SpriteLocal,
}

pub struct List {
    pub id: String,
    pub name: String,
    pub scope: ListScope,
}
```

### Statements

Statements represent Scratch command blocks.

```rust
pub enum Stmt {
    Expr(Expr),
    SetVariable { var: String, value: Expr },
    ChangeVariable { var: String, delta: Expr },
    AddToList { list: String, value: Expr },
    DeleteAllOfList { list: String },
    SetListItem { list: String, index: Expr, value: Expr },
    DeleteListItem { list: String, index: Expr },
    InsertListItem { list: String, index: Expr, value: Expr },
    Broadcast { message: Expr },
    HeapAlloc { result_offset: u32, size: Expr },
    Call { proc: String, args: Vec<Expr> },
    EnterFrame { slots: u32 },
    PopFrame { slots: u32 },
    FrameSet { offset: u32, value: Expr },
    If { condition: Expr, then_body: Vec<Stmt>, else_body: Vec<Stmt> },
    Repeat { times: Expr, body: Vec<Stmt> },
    RepeatUntil { condition: Expr, body: Vec<Stmt> },
    Forever { body: Vec<Stmt> },
    Stop { option: StopOption },
}
```

### Expressions

Expressions represent Scratch reporter blocks.

```rust
pub enum Expr {
    Literal(Value),
    Variable(String),
    List(String),
    ListItem { list: String, index: Box<Expr> },
    ListLength { list: String },
    ProcedureParam(String),
    Operator { opcode: String, args: Vec<Expr> },
    HeapLoad { addr: Box<Expr> },
    HeapIndex { base: Box<Expr>, offset: Box<Expr> },
    /// Read the current frame pointer `__scratcharch_fp`.
    FrameBase,
    /// Read `__scratcharch_stack[fp + offset]`.
    FrameGet { offset: u32 },
}

pub enum Value {
    Number(f64),
    String(String),
    Bool(bool),
}
```

## Future exporters

ScratchGraph is intentionally format-agnostic. Planned exporters include:

- **Scratch 3 JSON exporter** (`JsonExporter`): produces `project.json` for the
  Scratch 3 online/offline editor.
- **sb3 exporter**: produces a ZIP archive containing `project.json` and
  required asset metadata.
- **scratchblocks exporter**: produces human-readable text for documentation
  and code review.

Each exporter implements the `ScratchExporter` trait and operates only on a
`Project` value.

## Version

This specification describes ScratchGraph v0.4 as implemented in the
ScratchArch Scratch Backend Foundation v0.4 milestone.
