# ScratchArch Semantic Diff Design

> Design version: **v0.5**
> Crate: `crates/scratcharch-analyzer` (module `diff`)
> CLI: `scratcharch diff`

Semantic diff compares two Scratch projects by their behaviour rather than by
their JSON structure. Two projects that produce identical scripts, call graphs,
and control flow are semantically equivalent even if field ordering or metadata
differs.

## Algorithm

```text
semantic_diff(a: &Project, b: &Project) -> Vec<DiffEntry>
```

### Phase 1: Target matching
For each target (stage / sprite) in project A, find the corresponding target in
project B by name. Targets that appear in only one project are `Added` or
`Removed`.

### Phase 2: Script matching
For each matched target pair, compare:
1. Script count — a mismatch is a structural change
2. Block structure — compare flattened block IDs per script

### Phase 3: Procedure matching
For each matched target pair, compare:
1. Procedure count
2. Prototype names and parameter counts
3. Procedure body block sequences

### Phase 4: Call graph comparison
Build `CallGraph` for each project and compare:
- Node sets (added/removed procedures)
- Edge sets (added/removed calls)

### Phase 5: Similarity scoring
```
similarity = 1 - (changes / total_elements)
```

Results below `threshold` (default 0.8) are flagged as significantly different.

## Types

```rust
pub struct DiffResult {
    pub added: Vec<DiffEntry>,
    pub removed: Vec<DiffEntry>,
    pub changed: Vec<DiffEntry>,
    pub similarity: f64,
}

pub struct DiffEntry {
    pub target: String,      // stage or sprite name
    pub kind: DiffKind,      // Script, Procedure, Call, Variable
    pub description: String, // human-readable description
    pub details: String,     // machine-readable detail string
}

pub enum DiffKind {
    ScriptAdded,
    ScriptRemoved,
    ProcedureChanged,
    CallAdded,
    CallRemoved,
    VariableUsageChanged,
}
```

## CLI Usage

```bash
# Text output (default)
scratcharch diff old.json new.json

# JSON output
scratcharch diff old.json new.json --format json

# Custom threshold
scratcharch diff old.json new.json --threshold 0.9
```

## Limitations

- Does not model data-dependent behavioural differences (different input → different output).
- Block parameter values are compared structurally, not semantically.
- Broadcast matching is resolved by name only.
