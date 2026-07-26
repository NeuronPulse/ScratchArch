# ScratchArch Scratch Analyzer Design

> Design version: **v0.4**
> Crate: `crates/scratcharch-analyzer`

This document describes the static analysis crate added in Scratch Backend
Foundation v0.4. The analyzer proves that ScratchGraph is rich enough to support
industrial toolchains: control-flow graphs, dead code detection, variable usage
analysis, and call-graph analysis.

## Why a separate analyzer crate?

ScratchGraph is the semantic layer for Scratch programs. Analysis should happen
at that semantic layer, not on raw `project.json` and not on SAIR. A dedicated
crate keeps analysis logic separate from lowering, exporting, and runtime
simulation. It also lets us test analyses without needing a full Scratch VM.

## Architecture

```text
scratcharch-scratchgraph
        |
        v
scratcharch-analyzer
├── cfg          Control-flow graph construction
├── callgraph    Procedure call graph + recursion detection
├── reachability Unreachable script detection
└── variables    Variable/list read/write usage
```

No analyzer module depends on `scratcharch-core`, `scratcharch-ir`,
`scratcharch-vm`, or `scratcharch-llvm`. This preserves the layered architecture.

## Control-flow graph

`cfg::CfgAnalysis` builds a CFG for a flat statement body. Because ScratchGraph
keeps nested control structures (`If`, `Repeat`, etc.) as nested statement
vectors, the CFG recursively expands those bodies into separate node regions.

Nodes:

- `Entry`
- `Exit`
- `Statement { body_index }`

Edges:

- `Sequence`
- `TrueBranch` / `FalseBranch`
- `BackEdge`

The CFG is useful for dead-code elimination, loop analysis, and coverage
checking.

## Call graph and recursion

`callgraph::CallGraphAnalysis` scans all scripts and procedures for `Stmt::Call`
sites and builds a directed graph of caller → callee edges. It then detects:

- **Direct recursion**: a procedure calls itself.
- **Mutual recursion**: two procedures call each other.

The graph also exposes `callees(caller)` for inlining decisions and impact
analysis.

## Reachability

`reachability::ReachabilityAnalysis` determines which scripts can be triggered
statically:

- `GreenFlag` and `KeyPressed` scripts are always reachable.
- `SpriteClicked` and `CloneStart` scripts are reachable only on sprites.
- `BroadcastReceived` scripts are reachable only if a matching broadcast is sent
  somewhere in the project.

This is a conservative static approximation. Dynamic broadcast names or runtime
sprite creation may make additional scripts reachable.

## Variable usage

`variables::VariableUsageAnalyzer` counts reads and writes for each variable and
list across all scripts and procedures. It reports:

- **Dead variables**: written but never read.
- **Unread initializations**: read but never written.

Future passes can use this to eliminate dead stores or warn about uninitialized
reads.

## Future analyses

- Live-variable analysis using the CFG.
- Constant propagation through ScratchGraph expressions.
- Procedure inlining heuristics based on call graph depth.
- Detection of unsupported Scratch blocks for SAIR lowering.

## Testing

Tests live in `crates/scratcharch-analyzer/tests/analyzer_tests.rs` and cover
CFG construction, recursion detection, variable usage, and reachability. They
construct ScratchGraph projects directly so they do not depend on the parser or
exporter.
