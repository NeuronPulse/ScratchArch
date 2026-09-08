# Scratch Semantics

> Specification version: **v0.1**
> Scope: the observable semantics of a Scratch program as represented by
> ScratchGraph IR, and the contract for semantic-preserving roundtrips.
> Related: [`SCRATCHGRAPH.md`](./SCRATCHGRAPH.md), [`SCRATCH_ABI.md`](./SCRATCH_ABI.md),
> [`SCRATCH_MEMORY.md`](./SCRATCH_MEMORY.md),
> [`ROUNDTRIP_VALIDATION.md`](../design/ROUNDTRIP_VALIDATION.md).

This specification defines what it means for two Scratch programs to be
*semantically equivalent*. It is the reference the
`SemanticNormalizer`, the semantic diff, the roundtrip test matrix, and the
transform-preservation checks all build on. The central claim:

> Semantic equivalence is an IR-level property. It is **not** byte or JSON
> equality of `project.json`, and it must ignore block IDs, JSON key ordering,
> serialization detail, and other representation noise.

## 1. The semantic model

ScratchGraph models a Scratch program as a `Project`:

- **Targets**: one `Stage` plus zero or more `Sprite`s. A target owns a set of
  declared variables, lists, broadcasts, scripts, procedures, costumes, and
  sounds.
- **Scripts**: a script is an *event hat* plus an ordered *body*. A script runs
  when its hat's event fires.
- **Procedures** (custom blocks): named, parameterized bodies. Definitions are
  per-target; calls name a procedure within the same target.
- **Statements**: assignments, list operations, broadcasts, calls, structured
  control flow (`If`, `Repeat`, `RepeatUntil`, `Forever`, `Stop`).
- **Expressions**: literals, variable reads, list reads/lengths, parameter
  reads, operators, and (for ABI-lowered code) heap and frame accesses.
- **Runtime ABI** (lowered code only): `HeapAlloc`/`HeapLoad`/`HeapIndex`,
  `EnterFrame`/`PopFrame`/`FrameSet`/`FrameGet` and the frame bookkeeping they
  encode. These exist only in code that was compiled out of SAIR; real Scratch
  programs never contain them.

### 1.1 What is observable behavior

A program's behavior is the sequence of *observable effects* it produces:
variable and list mutations, broadcast events and the scripts they wake,
procedure calls, and control decisions taken. Two programs are semantically
equivalent when, started from any equivalent initial state, they produce the
same observable effects under Scratch's execution model.

Scratch runs scripts *concurrently*. Between two green flags the relative
ordering of scripts that share a hat is engine-defined, so a comparison must
not treat the *declaration order* of same-hat scripts as a semantic fact when
the runtime gives no ordering guarantee. In practice Scratch schedules hats in
target/script order, so this specification is conservative: same-hat script
order is treated as *potentially observable* (see §3, Scripts), which keeps the
framework from hiding real changes at the cost of occasionally reporting a
harmless reordering as a `Moved` entry.

## 2. The semantic units

| Unit | Identity | What is semantic | What is not semantic |
|------|----------|------------------|----------------------|
| Target | `name` | The target and everything it owns | Position in the sprite list |
| Variable | `name` (+ scope) | Name, scope (global vs sprite-local) | `id`, declaration order |
| List | `name` (+ scope) | Name, scope | `id`, declaration order |
| Broadcast | `name` | Message name; declared anywhere in the project is the same message | `id`, which target "declared" it, declaration order |
| Script | hat + ordered body | Hat kind, message/key, statement sequence | block IDs, debug name, cosmetic order of blocks within a block cluster |
| Procedure | target + `name` | Signature (name, param names/defaults) and body; calls bind by name within the owning target | definition order, block IDs |
| Statement/expression | normalized shape | Operator opcode, operand order and values, referenced names, control nesting, message text | block IDs, shadow flags, coordinates |
| Costume / Sound | — | not modeled as behavior (no block in the IR references them) | names, asset ids, order, bitmap flag, rotation centers |

### 2.1 The fidelity buckets

Every difference between two projects falls into exactly one of these buckets:

1. **Must-preserve semantics** — a difference here is a real behavioral change
   and must be reported (`Added`, `Removed`, `Changed`, `Moved`,
   `ScopeChanged`, `ControlFlowChanged`, `RuntimeChanged`).
2. **Volatile serialization detail** — may differ without any semantic effect:
   shadow flags, coordinates, `topLevel`, empty cosmetic fields, extension
   metadata, costume/sound asset content. Ignored.
3. **Volatile block IDs** — the concrete IDs of the `blocks` map in
   `project.json` are pure bookkeeping. ScratchGraph does not retain them, so
   they can never be reported as a semantic change.
4. **Volatile JSON ordering** — the iteration order of JSON object keys
   (`variables`, `blocks`, `inputs`, `fields`, …) and the array order of
   declaration maps are not semantic. Normalization sorts/dedupes where order
   carries no meaning.

### 2.2 Broadcast semantics

Broadcast messages are **project-global**. A `broadcast` statement anywhere —
on the stage, in a sprite, or inside a procedure — can trigger a
`BroadcastReceived` hat on any target. Therefore:

- message *declaration site* is not semantic (bucket 3);
- DCE must never remove a `BroadcastReceived` script whose message can be sent
  by any remaining code, including sends whose message expression is computed
  (non-literal); such sends are treated as *dynamic* and keep every receiver.

### 2.3 Variable / list reference binding

Statements and expressions reference variables and lists **by name**, and
procedures reference calls **by procedure name within the owning target**.
Consequences:

- a rename is a semantic change (a `SetVariable { var: "x" }` is not the same
  as `SetVariable { var: "y" }`);
- deleting a name that is still referenced breaks the program, so removal
  passes must be conservative across every target and scope;
- the `id` that ties a declaration in `project.json` to the `variables` map is
  representation only (bucket 3).

### 2.4 Numbers, strings, booleans

Scratch follows JavaScript/IEEE-754 `f64` arithmetic: `1.0 == 1`, `-0.0 == 0.0`,
division by zero yields `Infinity`/`NaN` (never a compile error), and there is
no integer type. Scratch booleans are a distinct value class: a comparison
reporter yields a boolean, which is not interchangeable with the numeric
literals `0`/`1`. A pass that folds `operator_equals`/`operator_gt` into a
numeric literal changes behavior and is forbidden (see
[`OPTIMIZATION.md §6`](../design/OPTIMIZATION.md)).

### 2.5 Scope model

- **Global**: visible to the stage and every sprite.
- **Sprite-local**: visible only within its owning sprite.
- Lists have the same two scopes.
- Temporary variables (compiler-generated) carry `VariableScope::Temporary`;
  for comparison purposes they are a distinct label only in ABI-lowered code.

Changing a variable/list between global and local scope (`ScopeChanged`) is a
semantic change: it alters which targets can read it.

## 3. Control flow and runtime model

- Statement order inside a body is semantic: `SetVariable x = 1; SetVariable
  x = 2` is not `SetVariable x = 2; SetVariable x = 1`.
- `If`, `Repeat`, `RepeatUntil`, `Forever`, and `Stop` define the structured
  control graph. Changing nesting, a loop's bound/condition, or adding/removing
  a `Stop` is `ControlFlowChanged`.
- Runtime-ABI statements (`EnterFrame`, `PopFrame`, `FrameSet`, `FrameGet`,
  `HeapAlloc`, `HeapLoad`, `HeapIndex`) and the expressions that read them
  (`FrameBase`, `FrameGet`, heap reads) form the *runtime frame/memory model*.
  A change confined to these — same control shape, different stack/heap write,
  same values — is `RuntimeChanged`. A change that also alters control shape is
  reported at the higher category.
- The SAIR leg of a roundtrip (`ScratchGraph → SAIR → ScratchGraph`) can only
  preserve what SAIR can express: **procedure bodies**. Event scripts and
  per-script/broadcast structure are outside the SAIR model today, so a SAIR
  roundtrip is only defined over a project's procedures and asserts equality
  over that subset (see `ROUNDTRIP_VALIDATION.md`).

## 4. Semantic equivalence contract

Given two projects A and B, write `N(A)` for the normalized form of A. A and B
are **semantically equivalent** iff `N(A) == N(B)` under the normalized
ordering and equality rules above. When they are not, the semantic diff
reports the smallest set of categorized differences explaining `N(A) → N(B)`.

The categories are:

| Category | Meaning |
|----------|---------|
| `Added` | an element exists in B but not A (sprite, variable, list, broadcast, procedure, script) |
| `Removed` | an element exists in A but not B |
| `Changed` | matched element with altered leaf content (literal, name reference, operator args) |
| `Moved` | an identical element present in both at a different position |
| `ScopeChanged` | variable/list changed global ↔ local |
| `ControlFlowChanged` | matched script/procedure differs in loop/conditional/stop structure |
| `RuntimeChanged` | matched code differs only in frame/heap ABI details |

## 5. Roundtrip contract

A roundtrip `X → Y → X'` is *semantically preserving* when `N(X) == N(X')`.
The framework verifies:

- `ScratchGraph → JsonExporter → project.json → parser → ScratchGraph`
  (JSON leg)
- `ScratchGraph → Sb3Writer → .sb3 → Sb3Reader → ScratchGraph` (SB3 leg)
- `ScratchGraph → Decompiler → SAIR → lower → ScratchGraph` (SAIR leg,
  procedure subset only)
- `ScratchGraph → transform pass → ScratchGraph` (transform preservation,
  per-pass allowed-change policies)

A roundtrip that changes only bucket-2/3/4 content is a **pass**. A roundtrip
that changes bucket-1 content is a **semantic regression** and must be fixed,
not papered over by loosening a comparison.

## 6. Known modeling limitations (current IR)

These are not bugs in the framework; they are gaps in what ScratchGraph models
and are deliberately *reported*, never hidden:

- Variable and list **initial values** are not represented in the IR
  (`Variable`/`List` carry name + scope only), so they are outside the
  equivalence contract until the IR grows a declaration-value field.
- Costume/sound content and order are not semantic in the IR (no block
  references them today); `next costume`-style blocks are not modeled.
- `wait`/`wait until`, motion/looks/sound blocks and clones are not modeled as
  distinct IR statements; such code can only roundtrip structurally.
- The SAIR leg is limited to procedure bodies and is value-lossy for
  non-integer numerics (SAIR is typed; Scratch is untyped `f64`).
