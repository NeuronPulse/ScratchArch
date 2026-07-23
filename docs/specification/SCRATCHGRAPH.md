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
the stage can hold broadcasts in Scratch, so broadcasts live on the stage.

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

A script is a stack of statements triggered by an event hat.

```rust
pub struct Script {
    pub hat: Hat,
    pub body: Vec<Stmt>,
}

pub enum Hat {
    GreenFlag,
    BroadcastReceived(String),
    Procedure { name: String },
}
```

### Procedures

A procedure is a reusable block definition with named parameters.

```rust
pub struct Procedure {
    pub prototype: ProcedurePrototype,
    pub body: Vec<Stmt>,
}

pub struct ProcedurePrototype {
    pub name: String,
    pub params: Vec<ProcedureParam>,
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
    Call { proc: String, args: Vec<Expr> },
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
    ProcedureParam(String),
    Operator { opcode: String, args: Vec<Expr> },
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

This specification describes ScratchGraph v0.1 as implemented in the
ScratchArch Scratch Backend Foundation v0.1 milestone.
