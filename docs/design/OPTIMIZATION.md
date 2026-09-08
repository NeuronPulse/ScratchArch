# SAIR Optimization Framework

> Crate: `crates/scratcharch-opt`

This document describes the LLVM-like optimization pass infrastructure for
ScratchArch IR (SAIR). The goal of the framework is not to provide the most
aggressive optimizations possible, but to:

1. Provide a stable, extensible optimization API.
2. Prove that SAIR is suitable for standard compiler transformations.
3. Prepare the ground for future optimization work.

The framework deliberately does **not** redesign SAIR, change the ISA, or add
architecture-specific optimizations.

---

## 1. Why optimization happens at the SAIR level

SAIR is ScratchArch's SSA-based intermediate representation. It sits between
LLVM textual IR and the lower-level ISA/VM, making it the natural place for
machine-independent optimizations:

- **Stable semantics**: SAIR has a small, well-defined instruction set with
  explicit types and terminators. Unlike LLVM IR, we control exactly which
  constructs exist and what they mean.
- **Fast iteration**: SAIR modules are plain Rust structs, so passes can be
  written, tested, and debugged without involving the LLVM C++ API or external
  tooling.
- **Shared between paths**: Both the SAIR interpreter and the ISA lowerer consume
  the same `IrModule`. Optimizing at SAIR benefits both execution paths.
- **Validation**: `IrModule::validate()` enforces invariants (reachability,
  terminator structure, phi predecessor consistency) that optimization passes can
  rely on and must preserve.

Because SAIR is smaller than LLVM IR, writing and reasoning about passes is
simpler. Complex transformations such as inlining or `mem2reg` can be prototyped
here before any ISA-level support is added.

---

## 2. Relationship with LLVM optimization

`scratcharch-llvm` parses textual LLVM IR and lowers it to SAIR. It performs
*translation*, not optimization. Any LLVM-level optimizations must be applied by
an external tool such as `opt` before the IR reaches ScratchArch.

The SAIR optimization framework is intentionally independent of LLVM:

- It operates on `IrModule`, not LLVM IR.
- It makes no assumptions about LLVM passes that may or may not have run.
- It can clean up code that the translator generates conservatively (for example,
  unused constants, unreachable blocks, or obviously foldable arithmetic).

In a full pipeline, the expected order is:

```text
C / LLVM IR  →  external LLVM opt (optional)  →  scratcharch-llvm
                                                  ↓
                                              SAIR module
                                                  ↓
                                       scratcharch-opt passes
                                                  ↓
                                       interpreter / ISA lowerer
```

SAIR-level passes complement LLVM optimization rather than replacing it. They
are especially useful for target-specific cleanup that LLVM cannot perform
because it does not know ScratchArch's final execution model.

---

## 3. Pass architecture

### 3.1 `OptimizationPass` trait

Every pass implements the same simple trait:

```rust
pub trait OptimizationPass {
    fn name(&self) -> &str;
    fn run(&mut self, module: &mut IrModule);
}
```

A pass receives a mutable `IrModule` and modifies it in place. There is no
separate pass result object; passes communicate only through the IR.

### 3.2 `PassManager`

`PassManager` registers passes and executes them in order:

```rust
let mut pm = PassManager::new();
pm.add(ConstantFold);
pm.add(DeadCodeElimination);
pm.run(&mut module);
```

Features:

- `add<P: OptimizationPass + 'static>` registers any pass.
- `run` executes registered passes sequentially.
- `print_pipeline` prints the current pipeline for debugging.
- `passes()` exposes the registered passes for inspection.

### 3.3 Analysis utilities

`FunctionAnalysis` provides reusable per-function information that passes can
share instead of recomputing from scratch:

- **Use/def tracking**: `has_users`, `num_users`, `def_map`, `use_map`.
- **Constant tracking**: `is_constant_value`, `get_constant`.
- **CFG analysis**: `predecessors`, `successors`, `is_block_reachable`,
  `reachable_blocks`.

`util::compact_value_ids` is a shared helper that removes dead SSA result ids
and renumbers remaining ids so that operands stay consistent. This is necessary
because SAIR `ValueId`s are positional: deleting an instruction that produced an
id shifts all later ids unless a renaming pass is performed.

### 3.4 Current passes

|Pass|File|Purpose|
|---|---|---|
|`ConstantFold`|`constant_fold.rs`|Fold `add`, `sub`, `mul`, `eq`, `lt`, `gt` when all operands are constants. Comparisons are evaluated on `i32` values. Folding runs iteratively so chains such as `(10+10)+5` collapse fully.|
|`DeadCodeElimination`|`dce.rs`|Remove instructions whose results have no users and are side-effect free. Safe to remove: `add`, `sub`, `mul`, `eq`, `lt`, `gt`, `Const`. Never removed: `store`, `call`, branch terminators, `return`, memory operations, `phi`, `gep`.|
|`CfgSimplify`|`cfg_simplify.rs`|Remove unreachable blocks and bypass empty unconditional-branch blocks. Preserves the entry block and skips merges that would invalidate phi nodes.|

---

## 4. Correctness constraints

Optimization passes must preserve SAIR invariants:

- Every block ends with exactly one terminator.
- The first block is the entry block.
- All referenced block labels exist.
- Phi incoming labels match actual predecessor blocks.
- All blocks are reachable from the entry block.

Passes use `IrModule::validate()` after translation and rely on the analysis
utilities to avoid breaking these invariants. `compact_value_ids` ensures that
 deleting instructions or blocks does not leave dangling `ValueId`s.

---

## 5. Future passes

The framework is designed to make adding new passes straightforward. Planned and
possible future work includes:

- **Function inlining**: Replace small call sites with the callee body, then run
  cleanup passes.
- **mem2reg / promote memory to registers**: Convert alloca/load/store patterns
  into SSA values where possible.
- **Loop optimizations**: Loop-invariant code motion, strength reduction inside
  loops.
- **Strength reduction**: Replace expensive operations with cheaper equivalents
  (for example, `x * 2` → `x + x` or shifts where appropriate).
- **SSA construction improvements**: Simplify phi placement and critical-edge
  handling to make later passes more effective.
- **Common subexpression elimination**: Reuse previously computed results when
  operands are identical.
- **Phi elimination / DemoteRegToStack**: Lower phi nodes to stack slots for
  targets that do not support phi directly.

These passes extend the existing `OptimizationPass` trait and reuse the analysis
helpers, keeping the architecture uniform.

---

## 6. ScratchGraph optimization framework

> Crate: `crates/scratcharch-transform`

In addition to SAIR-level optimization, ScratchArch provides a separate
optimization and transformation framework that operates directly on
**ScratchGraph IR** (`scratcharch_scratchgraph::ir::Project`). This layer is
responsible for cleaning up Scratch-specific representations after lowering
from SAIR or after importing a Scratch project.

The two frameworks are distinct and should not be confused:

| Framework           | Crate                  | IR operated on                     |
|---------------------|------------------------|------------------------------------|
| SAIR optimization   | `scratcharch-opt`      | SAIR `IrModule` (SSA, basic blocks) |
| ScratchGraph optimization | `scratcharch-transform` | ScratchGraph `Project` (sprites, scripts) |

Sections 1–5 of this document describe the former; this section describes the
latter.

### 6.1 Why optimize at the ScratchGraph level

ScratchGraph is a semantic representation of Scratch programs: sprites,
scripts, procedures, variables, lists, and control blocks. Some optimizations
are natural to express here rather than at SAIR:

- **Dead script elimination**: Scratch has event hats and custom blocks; an
  unreachable broadcast script or uncalled procedure can only be identified
  once the project structure is known. Broadcast messages are project-global
  (a sprite's `broadcast` can trigger another target's `when I receive`), while
  custom-block (procedure) definitions are per-target — DCE accounts for both.
- **Variable analysis**: Scratch variables are global or sprite-local and are
  referenced by name; a variable that no script or procedure references can be
  dropped, but removing a shared (cross-target) variable name is only attempted
  when no other target uses it.
- **Empty block removal**: Control blocks such as `if` with empty branches can
  be simplified in ScratchGraph without touching SAIR semantics.

Importantly, optimization must happen at the ScratchGraph or SAIR layer. The
`.sb3` crate only serializes and deserializes archives; the CLI only wires
crates together; and exporters preserve semantics.

### 6.2 `TransformPass` trait

Each ScratchGraph pass implements:

```rust
pub trait TransformPass {
    fn name(&self) -> &str;
    fn run(&mut self, project: &mut Project) -> PassReport;
}
```

A pass receives a mutable `Project` and returns a `PassReport` describing how
many nodes were modified, deleted, and how many variables were saved. Reports
are aggregated by the pass manager into an `OptimizationReport`.

### 6.3 `PassManager`

`PassManager` registers passes and executes them in order:

```rust
let mut pm = PassManager::new();
pm.add(ConstantFolding::new());
pm.add(DeadScriptElimination::new());
let report = pm.run(&mut project);
```

Pass names can also be parsed from strings such as `"all"`, `"dce"`, or
`"dce,constfold"` via `parse_pass_list` and `PassManager::from_pass_names`.

### 6.4 Current ScratchGraph passes

| Pass                    | File              | Purpose                                                            |
|-------------------------|-------------------|--------------------------------------------------------------------|
| `DeadScriptElimination` | `dce.rs`          | Remove unreachable broadcast scripts and uncalled procedures.      |
| `ConstantFolding`       | `const_fold.rs`   | Fold constant numeric arithmetic (`+`, `-`, `*`, `/`).            |
| `EmptyBlockRemoval`     | `empty_block.rs`  | Remove empty control blocks (`if`, `repeat`, etc.).                |
| `VariableAnalysis`      | `variable.rs`     | Remove variables never referenced by any script or procedure.      |

Notes on the current pass semantics:

- `DeadScriptElimination` always keeps green-flag, key, sprite-clicked and
  clone hats. `BroadcastReceived` scripts are kept when the broadcast name is
  known to be sent — from any target, including inside a procedure body — or
  when the project sends a broadcast dynamically (non-literal argument), since
  the receiver set cannot be proven empty. Procedures are kept only if some
  remaining script or procedure calls them by name (per-target call set).
- `ConstantFolding` folds only operators whose inputs are all numeric literals
  (`operator_add/subtract/multiply/divide`). Comparisons and `operator_not`
  are left unfolded: Scratch booleans are not interchangeable with the numeric
  literals `0`/`1`, and the IR values that carry them are distinct. Division by
  zero is left unfolded because Scratch/JS semantics yield `Infinity`/`NaN`,
  which a literal fold would change.
- `EmptyBlockRemoval` removes an empty `if` (both branches empty) or `repeat` in
  all cases. Empty `repeat until` / `forever` loops are removed only because
  ScratchGraph conditions and bodies are pure (no side effects, timing, or
  concurrency in the IR), so an empty idle loop is unobservable.
- `VariableAnalysis` is deliberately conservative across targets: it only
  removes a variable name when it is not referenced anywhere in the project, so
  it can over-retain but never wrongly delete a shared variable.

### 6.5 Optimization report

`OptimizationReport` collects per-pass `PassReport` entries:

- `modifications`: number of changed nodes/expressions.
- `deleted_nodes`: number of removed scripts, procedures, or blocks.
- `saved_variables`: number of variables removed.

The report can be rendered as text or JSON and is emitted by the CLI
`optimize` command.

### 6.6 CLI integration

The `scratcharch optimize` command implements the pipeline:

```text
.sb3 / .json
    ↓
ScratchGraph Project
    ↓
PassManager (selected passes)
    ↓
ScratchGraph Project
    ↓
.sb3 / .json
```

Example usage:

```bash
scratcharch optimize input.sb3 -o output.sb3 --passes all
scratcharch optimize input.json -o output.json --passes dce,constfold
scratcharch optimize input.sb3 --format json --report report.json
```

### 6.7 Future ScratchGraph passes

Possible extensions include:

- **Procedure inlining**: Inline small custom blocks when reentrancy is not
  required.
- **Constant propagation**: Propagate folded values across sequential
  statements.
- **Dead store elimination**: Remove variable assignments that are overwritten
  before being read.
- **Loop unrolling/peeling**: For small constant repeat counts, unroll the body
  into straight-line ScratchGraph statements.

These passes reuse the same `TransformPass` trait and reporting infrastructure.
