# ScratchArch CLI Design

> Design version: **v0.5**
> Crate: `crates/scratcharch-cli`
> Binary: `scratcharch`

The `scratcharch` CLI provides a unified command-line interface for all
Developer Toolchain features: analysis, decompilation, graph visualization,
project inspection, and semantic diffing.

## Subcommands

| Subcommand  | Description |
|-------------|-------------|
| `analyze`   | Static analysis report (text or JSON) |
| `decompile` | Decompile a Scratch project to SAIR |
| `graph`     | DOT graph output (cfg, callgraph, or full project) |
| `inspect`   | Quick summary of targets, scripts, and procedures |
| `diff`      | Semantic diff between two Scratch projects |

## CLI Architecture

```
scratcharch
├── analyze
│   ├── --format text|json
│   └── -o FILE          (optional output file)
├── decompile
│   ├── -o FILE           (output SAIR file)
│   └── --target TARGET   (optional target filter)
├── graph
│   ├── --kind cfg|callgraph|project
│   └── -o FILE           (optional output file)
├── inspect
│   └── INPUT             (project.json path)
└── diff
    ├── --format text|json
    ├── --threshold N     (similarity threshold %)
    └── INPUT_A INPUT_B   (two project.json paths)
```

## Implementation

Each subcommand lives in its own module under `crates/scratcharch-cli/src/`:

- `main.rs` — clap `Parser` with `#[command(subcommand)]`
- `analyze_cmd.rs` — calls `Report::build()` + output formatting
- `decompile.rs` — decompiles Scratch IR → text SAIR representation
- `graph.rs` — DOT output via `DotOutput` trait
- `inspect.rs` — summary statistics from `Project`
- `diff.rs` — calls `semantic_diff()` from the analyzer crate

All subcommands share a common flow: read JSON → `parse_project_json` →
invoke analyzer/transform → produce output.

## Dependencies

- `clap` v4 (derive) for argument parsing
- `serde_json` for JSON output format
- `scratcharch-analyzer` for analysis, DOT, diff
- `scratcharch-scratchgraph` for `parse_project_json` and `Project`

## Testing

Integration tests live in `crates/scratcharch-cli/tests/cli_tests.rs`. Each test
runs the binary via `Command` and checks stdout or output files for expected
content.  The `CARGO_BIN_EXE_SCRATCHARCH` environment variable (set by `cargo
test`) is preferred for locating the binary.
