# ScratchArch Pipeline Design

> Design version: **v0.7**
> Crate: `crates/scratcharch-pipeline`

The pipeline crate wraps the entire ScratchArch compilation pipeline into a
single API.  Before v0.7, callers had to orchestrate five or six crates
manually.  Now a single function call handles the full chain.

## Compiler pipeline

```text
Input
 │
 ├── .ll ──────→ LLVM IR lexer/parser (scratcharch-llvm)
 │
 ├── .json ────→ project.json parser (scratcharch-scratchgraph::parser)
 │
 └── .sb3 ─────→ (future: ZIP extract → JSON)
                      │
                      ▼
                 SAIR module (scratcharch-ir)
                      │
                      ├── validation
                      │
                      ├── optimization (scratcharch-opt)
                      │
                      ▼
                 ScratchGraph (scratcharch-scratchgraph::lower)
                      │
                      ├── JSON export (scratcharch-scratchgraph::json_exporter)
                      │
                      └── SAIR decompile (scratcharch-pipeline::decompile)
                              │
                              ▼
                         SAIR text output
```

## Intermediate representation boundaries

| Stage | Type | Crate |
|-------|------|-------|
| Input text | `&str` / file | — |
| LLVM IR AST | `llvm_ir::Module` | `scratcharch-llvm` |
| SAIR module | `IrModule` | `scratcharch-ir` |
| Optimized SAIR | `IrModule` | `scratcharch-opt` |
| ScratchGraph | `Project` | `scratcharch-scratchgraph` |
| project.json | `serde_json::Value` | `scratcharch-scratchgraph::json_exporter` |
| SAIR text | `String` | `scratcharch-ir::text` |

## Pipeline API

```rust
pub struct PipelineConfig {
    pub opt_level: String,    // "none" | "basic" | "aggressive"
    pub dump_dir: Option<PathBuf>,
    pub optimize: bool,
}

pub enum PipelineOutput {
    Json(serde_json::Value),
    SairText(String),
}

pub struct PipelineResult {
    pub output: PipelineOutput,
    pub intermediates: HashMap<String, String>,
    pub diagnostics: DiagnosticSink,
}
```

Three entry points:

| Method | Input → Output |
|--------|---------------|
| `run_llvm_to_json` | LLVM IR text → `project.json` |
| `run_json_roundtrip` | `project.json` → `project.json` (through ScratchGraph) |
| `run_decompile` | `project.json` → SAIR text |
| `run_file` | Auto-detect `.ll` vs `.json` and dispatch |

## Diagnostic system

Reference: LLVM diagnostic style.

```rust
pub struct Diagnostic {
    pub level: DiagnosticLevel,     // Error | Warning | Note
    pub message: String,
    pub stage: Option<String>,      // "llvm-parse", "sair-validate", etc.
    pub source_file: Option<String>,
    pub source_line: Option<u32>,
    pub source_column: Option<u32>,
    pub suggestion: Option<String>,
}
```

Output examples:

```
error: undefined variable x [analyze]
 --> sprite1/main:10:5
 help: check variable scope
```

JSON output:

```json
[
  {
    "level": "Error",
    "message": "undefined variable x",
    "stage": "analyze",
    "source_file": "sprite1/main",
    "source_line": 10,
    "source_column": 5,
    "suggestion": "check variable scope"
  }
]
```

## Debug workflow (`--dump`)

The `scratcharch pipeline input.ll --dump /tmp/debug` produces:

```
/tmp/debug/
├── stage0-input.ll           # original input
├── stage1-scratchgraph.json  # ScratchGraph summary (via explorer)
├── stage2-sair.txt           # raw SAIR module
├── stage3-optimized.sair     # after optimization
└── stage4-output.json        # final output
```

This lets developers inspect every transformation without adding print
statements.

## Decompiler (ScratchGraph → SAIR)

The `Decompiler` in `src/decompile.rs` walks a ScratchGraph `Project` and
produces a valid SAIR `IrModule`:

- Each ScratchGraph `Procedure` → one SAIR `IrFunction` (void return)
- Variables → `alloca` + `load`/`store` slots
- `If` → `cond_br` with then/else/merge blocks
- `Repeat`/`RepeatUntil` → loop header + body blocks
- `Forever` → infinite loop block
- `Call` → SAIR `call` instruction
- `SetVariable`/`ChangeVariable` → `load`/`add`/`store` sequences
- Arithmetic operators → SAIR `add`/`sub`/`mul`/`div`/`eq`/`lt`/`gt`

The decompiler does **not** require phi nodes: all mutable state goes
through memory (alloca + load/store), which keeps the generated SAIR simple
and compatible with all optimization passes.

## Testing

All pipeline crate tests live in `crates/scratcharch-pipeline/tests/pipeline_tests.rs`.

| Test | What it verifies |
|------|-----------------|
| `test_pipeline_llvm_to_json` | Full LLVM → SAIR → ScratchGraph → JSON |
| `test_pipeline_roundtrip_empty_project` | JSON → ScratchGraph → JSON |
| `test_pipeline_decompile_empty` | JSON → ScratchGraph → SAIR |
| `test_pipeline_invalid_input` | Error handling for bad LLVM |
| `test_pipeline_diagnostics` | DiagnosticSink text output |
| `test_diagnostic_with_source_location` | Source location formatting |
| `test_diagnostic_json_output` | Diagnostic JSON serialization |
| `test_decompile_with_procedure` | Decompiler with SetVariable |
