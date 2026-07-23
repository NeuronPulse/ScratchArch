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
