# ScratchArch Semantic Diff Design

> Design version: **v1.0**
> Crate: `crates/scratcharch-analyzer` (module `diff`)
> Spec contract: `docs/specification/SCRATCH_SEMANTICS.md` §4 (categories), §5 (equivalence)
> Related: `docs/design/ROUNDTRIP_VALIDATION.md`

Semantic diff compares two Scratch projects by *meaning* rather than by JSON
structure. The comparison pipeline is:

```text
semantic_diff(a, b)
  a ──normalize──▶ NormalizedProject        block IDs, declaration ordering,
  b ──normalize──▶ NormalizedProject        and broadcast declaration sites erased
     └──▶ DiffResult  (categorized differences)
```

Two projects with identical scripts, procedures, variables/lists, and control
flow are semantically equivalent even when block IDs, field ordering, sprite
order, or which target *declares* a broadcast differ. The normalizer
(`scratcharch-scratchgraph::semantic`, `SemanticNormalizer`) erases that
representation noise first, so the diff never reports it — this is what makes
the roundtrip & validation framework (`docs/design/ROUNDTRIP_VALIDATION.md`)
trust the empty result.

## API

```rust
// re-exported from scratcharch_analyzer:
pub use diff::{semantic_diff, semantic_diff_normalized,
               DiffCategory, DiffEntry, DiffFormat, DiffResult};

pub fn semantic_diff(a: &Project, b: &Project) -> DiffResult;
pub fn semantic_diff_normalized(na: &NormalizedProject, nb: &NormalizedProject) -> DiffResult;

pub struct DiffResult { pub entries: Vec<DiffEntry> }
impl DiffResult {
    pub fn is_empty(&self) -> bool;                       // no differences
    pub fn count(&self, category: DiffCategory) -> usize; // per-category tally
    pub fn has(&self, category: DiffCategory) -> bool;    // category present?
    pub fn format(&self, fmt: DiffFormat) -> String;      // Text | Json
}

pub struct DiffEntry {
    pub category: DiffCategory,
    pub location: String, // owning target ("Stage", a sprite name) or "project"
    pub subject: String,  // what changed, e.g. "procedure 'rect_area'"
    pub detail: String,   // per-category context, e.g. "signature changed"
}
impl DiffEntry { pub fn summary(&self) -> String; }  // one-line rendering

pub enum DiffCategory {
    Added, Removed, Changed, Moved,
    ScopeChanged, ControlFlowChanged, RuntimeChanged,
}
impl DiffCategory { pub fn label(self) -> &'static str; }

pub enum DiffFormat { Text, Json }
```

## What the normalizer erases (never reported)

The diff runs over `SemanticNormalizer` output, which drops the noise a
Scratch toolchain legitimately introduces and reintroduces:

- **Block IDs** — a rename/re-order of `b0..bN` ids is not a semantic change.
- **Declaration ordering** — the order variables/lists/procedures are declared
  in, and sprite ordering, is representation.
- **Broadcast declaration site** — which target *declares* message `go` is
  ignored; only the set of message names and who listens/sends matters.
- **Value normalization** — `-0.0` folds to `0.0` before comparison.

Noise erasure is a *normalizer* concern: `semantic_diff` calls
`SemanticNormalizer::new()` on both sides itself. Callers that already hold
normalized projects use `semantic_diff_normalized`.

## Categories

Each difference is classified (labels match the fidelity table in
`SCRATCH_SEMANTICS.md` §4):

| Category | Meaning | Example |
|---|---|---|
| `Added` | Element exists only on the right | a new sprite, script, procedure, variable, or broadcast |
| `Removed` | Element exists only on the left | a deleted broadcast receiver script |
| `Changed` | Matched content whose *leaf* content differs | `set x to 1` → `set x to 2`; procedure signature change |
| `Moved` | Identical element present on both sides, relative order changed | two green-flag scripts swapped |
| `ScopeChanged` | Variable/list changed global ↔ sprite-local | `x` declared Global in A, SpriteLocal in B |
| `ControlFlowChanged` | Nesting, a loop bound/condition, or a `Stop` changed | `repeat 10` → `repeat 20`; a `forever` inserted |
| `RuntimeChanged` | Difference confined to runtime frame/heap ABI ops | an `EnterFrame`/`PopFrame`/`FrameSet`/`HeapAlloc` added or removed |

### Scope matching and aggregation

- **Targets** are matched by name (stage + sprites). Targets present on one side
  only are `Added`/`Removed` at location `"project"`.
- **Procedures** are matched by name; a same-name procedure with different
  parameters reports `Changed` with detail `"signature changed"`, and a body
  difference is classified separately.
- **Scripts** are matched by *content* (hat + canonical body). Identical
  content that changed relative position reports `Moved`; unmatched content
  pairs by hat and reports the body classification, or `Added`/`Removed`.
- **Bodies** are compared by longest-common-subsequence over canonical
  statements. Differences aggregate upward: a body whose *only* change is a
  deleted dead assignment reports `Changed`; a body that also restructures
  control reports `ControlFlowChanged`. Precedence is `ControlFlowChanged` >
  `RuntimeChanged` > `Changed`, matching the severity order of
  `SCRATCH_SEMANTICS.md` §4.
- **Scope** changes on variables/lists report `ScopeChanged` rather than
  `Added` + `Removed`, because a rename would mask the real change.

## Output

`DiffResult::format(DiffFormat::Text)` prints a count, per-category tallies,
and one numbered `summary()` line per entry:

```text
Semantic differences: 2
  Changed: 1
  ControlFlowChanged: 1
1. [Changed] Stage: script (when green flag clicked) — body
2. [ControlFlowChanged] Stage: script (when green flag clicked) — body
```

Empty results format as `No semantic differences found.`. `DiffFormat::Json`
emits `{ "count": N, "differences": [ { "category", "location", "subject",
"detail" }, ... ] }`.

## CLI

```bash
scratcharch diff a.json b.json               # text (default)
scratcharch diff a.json b.json --format json
```

Both inputs load through the `.json` project parser. Empty diffs additionally
print `Projects are semantically equivalent.`.

## Limitations

- **No data-dependent behavior.** The diff compares program structure after
  normalization, not executions. Two programs that differ only under particular
  runtime inputs are compared structurally, not traced.
- **Value semantics at leaves.** Literal and name changes are reported
  faithfully but no deeper algebraic equivalence is attempted — `x` and
  `x + 0` still differ.
- **Broadcast semantics are name-resolved.** Message identity is by name; the
  diff does not reason about which sends reach which receivers beyond the
  declared hat scripts, so an unreachable receiver still counts as content.
- **Single-target-local vs global scope** is compared per target; it does not
  attempt cross-target shadowing analysis.
