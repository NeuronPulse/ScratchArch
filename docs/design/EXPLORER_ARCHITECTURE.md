# ScratchArch Explorer Architecture

> Design version: **v0.6**
> Crate: `crates/scratcharch-explorer`

The Explorer crate provides a unified data-model query interface for both
SAIR modules and ScratchGraph projects. It is separate from the CLI so that
the query API can be reused by GUI tools, IDEs, automated formatters, and
CI checks without depending on `clap` or `main.rs`.

## Motivation

Before v0.6, every tool that needed to inspect a project had to either:

1. Parse JSON/SAIR manually and iterate over raw IR types.
2. Call into the CLI (`scratcharch inspect`) and parse stdout.

Neither approach scales. GUI tools need structured JSON, IDE plugins need
stable Rust types, and CI checks need a library API that returns
machine-readable summaries.

## Crate layering

```text
scratcharch-cli
    │
    │  uses
    ▼
scratcharch-explorer   (new - v0.6)
    │
    ├── scratch  →  ScratchGraph project summaries
    └── sair     →  SAIR module summaries
```

The explorer crate depends on:
- `scratcharch-ir` (for `IrModule`, types, instructions)
- `scratcharch-scratchgraph` (for `Project`, `Stage`, `Sprite`, etc.)
- `serde` + `serde_json` (for structured output)

It does **not** depend on `scratcharch-analyzer`, `scratcharch-driver`,
or any CLI crate. This keeps the dependency footprint small.

## Data model

### SAIR explorer (`SairExplorer`)

```
SairModuleSummary
├── entry: String
├── function_count: usize
├── total_instructions: usize
├── total_blocks: usize
└── functions: Vec<SairFunctionSummary>
        ├── name, return_type, param_count, value_count
        ├── block_count, instruction_count
        └── blocks: Vec<SairBlockSummary>
                ├── label, instruction_count
                ├── terminator: String
                └── source_location: Option<String>
```

### Scratch explorer (`ScratchExplorer`)

```
ScratchProjectSummary
├── project_name: String
├── sprite_count: usize
└── targets: Vec<TargetSummary>
        ├── name, is_stage
        ├── script_count, procedure_count
        ├── variable_count, list_count, broadcast_count
        ├── scripts: Vec<ScriptSummary>
        │       ├── hat, name, body_length
        └── procedures: Vec<ProcedureSummary>
                ├── name, param_count, body_length, frame_size
```

## Output formats

Every explorer provides two output methods:

| Method       | Returns      | Use case            |
|--------------|-------------|---------------------|
| `to_text()`  | `String`    | Terminal display    |
| `to_json()`  | `String`    | Machine consumption |

## CLI integration

The `scratcharch inspect` command uses the explorer crate internally:

```bash
# Text output (Scratch project)
scratcharch inspect project.json

# JSON output (machine-readable)
scratcharch inspect project.json --json

# SAIR module summary
scratcharch inspect module.sair

# SAIR module as JSON
scratcharch inspect module.sair --json
```

The `inspect` subcommand auto-detects the input format (SAIR text vs.
JSON) by trying to parse as SAIR first, then falling back to Scratch
project JSON.

## Testing

All six explorer methods are tested in
`crates/scratcharch-explorer/tests/explorer_tests.rs`:

- `test_explorer_sair_basic` — SAIR summary structure
- `test_explorer_sair_text_output` — SAIR text rendering
- `test_explorer_sair_json_output` — SAIR JSON serialization
- `test_explorer_scratch_basic` — Scratch summary structure
- `test_explorer_scratch_json_output` — Scratch JSON serialization
- `test_explorer_scratch_text_output` — Scratch text rendering

## Future work

- Add an `IrExplorer` for the core ISA (instructions, values, programs).
- Add a query filtering API (e.g. "find all functions with >10 blocks").
- IDE LSP backend using the explorer as the data provider.
