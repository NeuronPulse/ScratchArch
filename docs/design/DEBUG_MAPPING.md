# ScratchArch Debug Mapping Design

> Design version: **v0.6**
> Status: Cross-layer source location foundation

This document describes how debug information propagates through the
ScratchArch compiler pipeline: from SAIR instructions to ScratchGraph nodes
to `project.json` block IDs.

## Overview

The debug mapping layer answers two questions:

1. "Which `project.json` block corresponds to this SAIR instruction?"
2. "Which SAIR instruction produced this ScratchGraph statement?"

## Data model

### SAIR `DebugLoc`

Existing (v0.4). Each SAIR instruction may carry an optional debug location
with `(file, line, column)`.

Source: `crates/scratcharch-ir/src/debug.rs`

```rust
pub struct DebugLoc {
    pub file: String,
    pub line: u32,
    pub column: Option<u32>,
}
```

### ScratchGraph `SourceLocation`

New (v0.6). Each ScratchGraph IR node can be mapped to its original JSON
block via a `SourceLocation`. The mapping is stored as a side-table so the
IR types themselves remain free of serialization details.

Source: `crates/scratcharch-scratchgraph/src/debug.rs`

```rust
pub struct SourceLocation {
    pub project_id: Option<String>,
    pub sprite_name: String,
    pub script_id: Option<String>,
    pub block_id: Option<String>,
    pub opcode: Option<String>,
}
```

### `TargetSourceMap` / `SourceMap`

A per-target index of parsed block IDs to opcode metadata. Built by
`SourceMapBuilder` during `project.json` parsing.

```rust
pub struct TargetSourceMap {
    pub target_name: String,
    pub blocks: Vec<BlockSourceEntry>,
}

pub struct BlockSourceEntry {
    pub block_id: String,
    pub opcode: String,
    pub is_top_level: bool,
    pub parent: Option<String>,
}
```

## Pipeline layers

```text
Frontend (LLVM IR / SAIR)
    │
    │  DebugLoc (file, line, column)
    ▼
SAIR (IrModule)
    │
    │  SourceLocation (project_id, sprite, script, block, opcode)
    ▼
ScratchGraph (Project)
    │
    │  SourceMap (target → [block_id → opcode])
    ▼
project.json (blocks dict)
```

### Forward mapping (SAIR → ScratchGraph → JSON)

When the SAIR → ScratchGraph lowerer translates a SAIR instruction that
carries a `DebugLoc`, it should attach a corresponding `SourceLocation` to
the resulting ScratchGraph statement. The lowering pass is not required to
preserve every `DebugLoc`; the data model supports lossy round-trips.

### Reverse mapping (JSON → ScratchGraph → SAIR)

`SourceMapBuilder` traverses the `blocks` dictionary of each target and
records every block's `opcode`, `topLevel` flag, and `parent`. This allows
future decompilation passes to answer "which Scratch block did this
ScratchGraph statement come from?"

## CLI integration

The `scratcharch debug` subcommand displays the source map for a given
`project.json`:

```bash
scratcharch debug project.json
```

Output:

```
Source map for: project.json
  target: Stage
    abc123 opcode=event_whenflagclicked top=true parent=None
    def456 opcode=data_setvariableto top=false parent=Some("abc123")
```

## Future work

- Attach `SourceLocation` to `Stmt` and `Expr` variants during parser round-trip.
- Forward `DebugLoc` into `SourceLocation` during SAIR → ScratchGraph lowering.
- The `scratcharch debug --sair module.sair` cross-reference mode to walk
  the mapping bidirectionally.
