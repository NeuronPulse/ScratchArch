# Differential corpus

Committed representative Scratch programs used for automatic regression of the
Scratch pipeline.

## Layout

```
tests/corpus/
  corpus.rs          harness: loads every fixture, validates it, checks it
                     against its canonical builder
  members.rs         member definitions as ScratchGraph IR builders
  scratch/           committed data (loadable project.json) + manifest.json
```

## What a member is

A *corpus member* is a small, representative program from the native
roundtrippable subset (see `docs/specification/SCRATCH_SEMANTICS.md` §5). The
committed fixture is the `project.json` that `JsonExporter` produces for the
member — i.e. the exact bytes a tool would hand to `scratcharch verify` or load
through the Sb3 reader. Members cover every category the framework is built
around: `basic`, `events`, `procedures`, `recursion`, `lists`, `memory`.

## What the harness checks

For each member the harness:

1. loads `scratch/<id>.json` from disk via `parse_project_json` (the CLI's
   `.json` input path) — parse must succeed;
2. runs graph validation — the fixture must be well-formed;
3. runs `verify_project` — graph + SB3 byte roundtrip + semantic preservation +
   per-pass transform preservation must all pass;
4. normalizes the fixture and compares it with the canonical member builder —
   the committed file and the canonical IR must agree.

Each fixture is also end-to-end verifiable by hand:

```bash
cargo run -p scratcharch-cli -- verify \
  crates/scratcharch-validation/tests/corpus/scratch/broadcast_relay.json
```

## Adding or changing a member

Members are authored in `members.rs`. After editing, regenerate the committed
fixtures:

```bash
SCRATCHARCH_REGEN_CORPUS=1 cargo test -p scratcharch-validation \
  --test corpus regenerate_corpus_fixtures -- --ignored --nocapture
```

Regeneration is deterministic (byte-identical on rerun). Review the diff like
any golden-file change. If a member changed, fixtures change with it; if a
parser or exporter change alters a member's *meaning*, the `matches_builder`
test fails and must be fixed rather than blessed.

## Adding members without regenerating

Adding a fixture purely as data (e.g. a hand-exported real-world project) is
possible: drop `<id>.json` into `scratch/`, add a matching manifest entry, and
the harness will load and validate it. Members without a `members.rs` builder
skip the canonical-equality check; to get it, add a builder too.
