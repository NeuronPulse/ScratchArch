# ScratchArch Development Roadmap

> Last updated: 2026-09-09 (LLVM Compatibility v0.2 completed)
> Status: living document

## Legend

- [x] Completed
- [ ] Planned

## Completed (v0.1 core)

### Specification

- [x] **ISA specification** (`docs/specification/ISA.md`): Instruction set, type
      system, cell model, control flow.
- [x] **ABI specification** (`docs/specification/ABI.md`): Calling convention,
      activation model, argument passing, return values.
- [x] **Memory specification** (`docs/specification/MEMORY.md`): Address space
      model, memory layout, type sizes, memory instructions.
- [x] **Execution model specification** (`docs/specification/EXECUTION_MODEL.md`):
      Value model, program execution, function call lifecycle, stack frames,
      memory model, undefined behavior. Prevents double-write and double-PC bugs
      by formalizing CALL/RETURN ownership rules.

### LLVM IR Translator (`scratcharch-llvm`)

- [x] **LLVM IR parser**: Hand-written lexer + recursive descent parser for
      textual `.ll` format. Parses function definitions, basic blocks,
      instructions, types, and SSA values.
- [x] **LLVM→SAIR translator**: Walks LLVM AST and builds SAIR `IrModule`
      via `IrBuilder`. Supports add/sub/mul/sdiv/udiv, icmp (eq/ne/slt/sgt/
      sle/sge/ult/ugt/ule/uge), alloca/load/store, call, br/cond_br, ret,
      getelementptr (array + struct).
- [x] **Type mapping**: i1/i8/i16/i32/ptr/void → SAIR types.
      Unsupported types rejected with clear error.
- [x] **Alloca count support**: `alloca T, i32 N` for array allocation.
- [x] **Array and struct types**: `[N x T]` and `{ T, T, ... }` type parsing.
- [x] **Negative constants, nsw/nuw/inbounds/align**: Parsed and handled.
- [x] **Entry point detection**: Uses `main` function as SAIR entry point.
- [x] **SAIR validation**: Translated modules pass `IrModule::validate()`.
- [x] **24 tests**: Return constant, arithmetic, function call, branch (true/false),
      memory, icmp ne, unconditional branch, void return, error rejection,
      C program tests (factorial, fib, array, struct, negative, division,
      icmp sgt/sle/sge).
- [x] No LLVM dependency — pure Rust parser.

### SAIR Crate (`scratcharch-ir`)

- [x] **SSA value model** with typed ValueId, SsaValue, Constant enum
- [x] **Instruction set**: Add, Sub, Mul, Div, Rem, Eq, Lt, Gt, Const,
      Alloca (with count), Load, Store, Call, Phi, Gep
- [x] **Basic blocks** with sequential instructions + exactly one terminator
- [x] **Terminators**: Branch, CondBranch, Return
- [x] **Validation**:
  - Entry block must be first
  - All referenced block labels must exist
  - Phi incoming labels must correspond to actual predecessor blocks
  - All blocks must be reachable from entry
  - Functions must have at least one block
- [x] **Builder API**: add_param, new_block, emit, arithmetic ops,
      const_i32/f64/i1/i8/i16, alloca/alloca_array, load/store, call, phi, gep,
      set_terminator/br/cond_br/ret
- [x] **Type system**: I1, I8, I16, I32, F64, Pointer, Void with
      size_in_bytes(), alignment(), to_core_type()
- [x] **GEP instruction** with dynamic and struct-field indices
- [x] **ISA lowerer**: Stack-based lowering for single-block functions
      with no phi or GEP (rejects complex cases with clear errors)

### SAIR Interpreter (`scratcharch-sair-interpreter`)

- [x] **All SAIR instructions** directly executable
- [x] **Multi-block control flow**: Branch, CondBranch with block dispatch
- [x] **Phi nodes**: Edge-selected semantics via prev_block tracking
- [x] **GEP**: Dynamic and struct field index computation
- [x] **Function calls**: Recursive call support with independent frames
- [x] **Memory model**: Flat byte-addressable, LE, stack-based alloc
- [x] **Type support**: All SAIR types (i1, i8, i16, i32, f64, ptr, void)
- [x] **Return value ownership**: Fixed double-write bug where the Return
      handler in execute_frame both wrote to the caller's value slot and
      incremented the caller's instr_idx, causing the Call instruction handler
      to write the return value to the wrong slot.

### Core / VM Enhancements

- [x] **Pick instruction** in core ISA + VM (for frame-relative addressing)
- [x] **Store accepts Pointer values** (for frame pointer save/restore)

### Target Profile (`scratcharch-target`)

- [x] **TargetProfile structure**: name, cell_width, pointer_width, endianness,
      integer_model, memory_model, abi_version.
- [x] **SA48 profile**: 48-bit cell, 32-bit pointer, little-endian, modular
      wrapping, flat byte-addressable memory, ABI v0.1.
- [x] **Validation**: Rejects invalid/unsupported widths, pointer wider than
      cell, unsupported ABI/integer/memory models.
- [x] **Display**: Human-readable profile description.
- [x] **MemoryLayout**: Address space partitioning helper.
- [x] **AbiConvention**: Cell-level ABI calculation helpers.
- [x] **12 tests**: SA48 creation/validation/display/cells/max_addressable,
      invalid width rejection (unit + integration).

### C Compatibility

- [x] **C source programs**: hello.c, add.c, factorial.c, fib.c, array.c,
      struct.c under `tests/c_programs/`.
- [x] **Hand-written LLVM IR**: Matching .ll files for each C program.
- [x] **Pipeline integration tests**: All 6 C programs compile through
      the full LLVM→SAIR→interpreter pipeline with expected results.
- [x] **`run_c_tests.sh`**: Shell script to compile C→LLVM with clang (if
      available), translate, and verify results.

### Documentation

- [x] `LLVM_TRANSLATION.md` — LLVM IR → SAIR design, mapping, supported subset
- [x] `PHI_LOWERING.md` — phi lowering strategy and edge-copy approach
- [x] `SAIR_INTERPRETER.md` — interpreter architecture and usage
- [x] `EXECUTION_MODEL.md` — formal runtime semantics (archived at
      `docs/specification/`)
- [x] `TARGET_PROFILE.md` — architecture vs profile vs implementation,
      SA48 definition, future profiles (archived at `docs/specification/`)

### SAIR Optimization Framework (`scratcharch-opt`)

- [x] **Pass infrastructure**: `OptimizationPass` trait, `PassManager`, reusable
      `FunctionAnalysis`, shared id-compaction helper.
- [x] **Constant folding**: Folds `add`, `sub`, `mul`, `eq`, `lt`, `gt` with
      constant operands; iterative so chains collapse fully.
- [x] **Dead code elimination**: Removes side-effect-free instructions with no
      users while preserving SSA id consistency.
- [x] **CFG simplification**: Removes unreachable blocks and bypasses empty
      unconditional-branch blocks without breaking phi semantics.
- [x] **Documentation**: `docs/design/OPTIMIZATION.md`.

### ScratchArch Runtime Library (`scratcharch-runtime`)

- [x] **New crate** `crates/scratcharch-runtime` for portable runtime routines.
- [x] **Memory routines**: `memcpy`, `memmove`, `memset`, `memcmp` with correct
      overlapping behaviour and endian-neutral byte access.
- [x] **String routines**: `strlen`, `strcmp`, `strcpy`, `strncpy` for C-style
      null-terminated strings.
- [x] **Panic / abort**: `abort`, `panic`, `trap` returning runtime errors.
- [x] **Intrinsic registry**: ScratchArch-named intrinsics (`__scratcharch_*`)
      dispatched through `IntrinsicRegistry`; no LLVM-specific names.
- [x] **Abstract memory interface**: `ByteMemory` trait with implementations for
      `Vec<u8>` and `&mut [u8]`.
- [x] **Interpreter integration**: SAIR interpreter dispatches undefined callee
      calls to the runtime intrinsic registry.
- [x] **LLVM parser support**: `declare` statements parsed and skipped by the
      translator, enabling end-to-end pipeline tests.
- [x] **Documentation**: `docs/specification/RUNTIME.md` and
      `docs/design/RUNTIME_DESIGN.md`.
- [x] Architecture-independent, no Scratch-specific or backend-specific code.

### Toolchain Integration v0.1 (`scratcharch-driver`)

- [x] **New crate** `crates/scratcharch-driver`: unified compilation pipeline
      `LLVM IR → translator → SAIR → optimizer → interpreter`.
- [x] **Clean API**: `CompileDriver`, `CompileConfig`, `OptLevel`,
      `CompiledModule`, `DriverError`; no dependency cycles.
- [x] **CLI example** `examples/sairc.rs`: compiles and runs an LLVM IR file
      with selectable optimization level.
- [x] **Driver tests**: end-to-end tests for all C compatibility programs under
      multiple optimization levels plus invalid-input error handling.
- [x] **SAIR serialization**: stable textual format in
      `crates/scratcharch-ir/src/text/`, specification in
      `docs/specification/SAIR_FORMAT.md`, round-trip tests including debug
      locations and call instructions.
- [x] **Debug metadata**: optional `DebugLoc` (file, line, column) stored as a
      side-table in `BasicBlock`; does not affect validation or execution.
- [x] **Expanded C compatibility**: `pointer`, `string`, `memory`, `recursion`
      tests alongside existing `hello`, `add`, `factorial`, `fib`, `array`,
      `struct`; all verified through `LLVM → SAIR → interpreter`.
- [x] **Documentation review**: added "Known limitations for v0.1" sections to
      `ISA.md` and `ABI.md` documenting gaps between the frozen architecture
      and the v0.1 implementation.

### Backend Foundation v0.1 (`scratcharch-ir` / `scratcharch-vm` / `scratcharch-driver`)

- [x] **Local-slot ISA instructions**: `local.get <slot>` / `local.set <slot>`
      in `scratcharch-core`, with VM frame support (`locals` vector) and
      function metadata (`param_cells`, `local_count`, `return_cells`).
- [x] **Multi-block SAIR lowering**: `IsaLowerer` lowers functions with
      multiple basic blocks, branch/conditional-branch terminators, and
      label resolution.
- [x] **SSA phi elimination**: edge-based copy insertion, critical-edge
      splitting into trampoline blocks, parallel-copy resolution with a temp
      slot for cyclic operands.
- [x] **Function call and return lowering**: argument/return value movement
      between operand stack and local slots, frame prologue/epilogue.
- [x] **GEP lowering**: dynamic index lowering to pointer arithmetic; struct
      fields deferred until type layout is available.
- [x] **Target-aware lowering**: `TargetProfile::cells_for_type` drives slot
      allocation; parameter and return value decomposition verified with an
      artificial `sa16` profile.
- [x] **End-to-end VM backend tests**: driver pipeline `LLVM IR → SAIR →
      optimization → ISA → VM` exercised for `hello`, `add`, `factorial`,
      `fib`, `recursion`, `array`, and `pointer` C compatibility programs.
- [x] **Backend documentation**: `docs/design/BACKEND_DESIGN.md`, updates to
      `PHI_LOWERING.md`, `SAIR_DESIGN.md`, `ROADMAP.md`, and
      `EXECUTION_MODEL.md`.

### Scratch Backend Foundation v0.1 (`scratcharch-scratchgraph`)

- [x] **New crate** `crates/scratcharch-scratchgraph`: ScratchGraph IR for
      semantic Scratch program representation.
- [x] **ScratchGraph data model**: `Project`, `Stage`, `Sprite`, `Script`,
      `Procedure`, `Variable`, `List`, `Broadcast`, `Stmt`, `Expr`, `Value`,
      and event `Hat`s.
- [x] **SAIR → ScratchGraph lowering**: `ScratchGraphLowerer` maps SAIR
      functions to Scratch custom blocks, SSA values to stage variables,
      control flow to Scratch control blocks, and calls to custom block calls.
- [x] **Exporter abstraction**: `ScratchExporter` trait isolates format-specific
      code. `JsonExporter` produces Scratch 3 `project.json`.
- [x] **Architecture separation**: `scratcharch-core`, `scratcharch-ir`, and
      `scratcharch-vm` remain Scratch-agnostic. Only `scratcharch-scratchgraph`
      and exporters know Scratch concepts.
- [x] **Tests**: SAIR → ScratchGraph → JSON exporter pipeline tests for
      arithmetic, variable assignment, conditional, procedure call, and loop
      exporter output.
- [x] **Documentation**: `docs/design/SCRATCH_BACKEND_DESIGN.md` and
      `docs/specification/SCRATCHGRAPH.md`.

### Scratch Backend Foundation v0.2 (`scratcharch-scratchgraph`)

- [x] **Semantic IR extensions**: event hats (`GreenFlag`, `KeyPressed`,
      `SpriteClicked`, `BroadcastReceived`, `CloneStart`), concurrent scripts per
      sprite/stage, scoped variables (`Global`, `SpriteLocal`, `Temporary`),
      scoped lists (`Global`, `SpriteLocal`), list operations, and a heap
      abstraction (`HeapAlloc`, `HeapLoad`, `HeapIndex`).
- [x] **Procedure return values**: hidden stage variable convention
      (`__ret_<func>`) so SAIR functions with return types lower to Scratch
      custom blocks without modifying SAIR or core.
- [x] **Memory abstraction**: SAIR `alloca`/`load`/`store`/`gep` lower to a
      single stage-backed heap list (`__scratcharch_heap`) with 0-based pointers
      in ScratchGraph and 1-based indexing at the JSON boundary.
- [x] **Improved lowering**: support for multiple procedures, functions with
      return values, global variable declarations, basic loops, and nested
      conditionals inside loop bodies.
- [x] **Tests**: event hats, variable scopes, list operations, procedure return
      values, multi-script sprites, nested conditionals inside loops, and heap
      memory lowering.
- [x] **Documentation**: updated `docs/specification/SCRATCHGRAPH.md` and
      `docs/design/SCRATCH_BACKEND_DESIGN.md`; new
      `docs/design/SCRATCH_RUNTIME_MODEL.md` and
      `docs/specification/SCRATCH_MEMORY.md`.

### Scratch Backend Foundation v0.3 (`scratcharch-scratchgraph`)

- [x] **Scratch Runtime ABI**: `docs/specification/SCRATCH_ABI.md` defines the
      frame-based calling convention using `__scratcharch_stack` and
      `__scratcharch_fp`.
- [x] **Frame-based runtime model**: `Procedure::frame_size`,
      `Stmt::EnterFrame`, `Stmt::PopFrame`, `Stmt::FrameSet`, `Expr::FrameBase`,
      and `Expr::FrameGet` provide a reentrant call stack abstraction.
- [x] **Recursive procedure support**: SAIR `Call` lowers to push frame, call,
      copy return slot, restore frame pointer, pop frame; `Return` writes the
      per-frame return slot and pops locals.
- [x] **Scheduler design**: `docs/design/SCRATCH_SCHEDULER.md` outlines script
      lifecycle, event dispatch, cooperative single-threaded scheduling, and
      per-thread stack future work.
- [x] **Roundtrip design**: `docs/design/SCRATCH_ROUNDTRIP.md` explains the
      future `project.json → ScratchGraph → SAIR` path for decompilation.
- [x] **Tests**: recursive factorial, recursive fibonacci, nested function
      calls, frame-local variables, frame primitives JSON export, and existing
      v0.2 regression tests.
- [x] **Documentation updates**: `docs/specification/SCRATCHGRAPH.md` v0.3,
      `docs/design/SCRATCH_BACKEND_DESIGN.md`, `docs/design/SCRATCH_RUNTIME_MODEL.md`,
      and `docs/design/ROADMAP.md`.

### Scratch Backend Foundation v0.4 (`scratcharch-scratchgraph` / `scratcharch-analyzer`)

- [x] **Runtime abstraction**: `RuntimeState`, `SchedulerState`, `ThreadState`,
      `EventState`, and `ThreadContext` model Scratch VM execution at the IR level.
- [x] **Per-thread frame ABI**: `ThreadContext` owns a private stack and frame
      pointer so future concurrent scripts do not share global `__scratcharch_stack`.
- [x] **Event model**: `EventHat` and `ScriptEntry` provide a clear event-driven
      entry point for scripts.
- [x] **Basic roundtrip parser**: `ProjectParser` reads Scratch 3 `project.json`
      into ScratchGraph AST for variables, lists, broadcasts, procedures,
      arithmetic, and control blocks.
- [x] **Analyzer crate**: `scratcharch-analyzer` provides CFG construction,
      call-graph + recursion detection, unreachable-script detection, and
      variable/list usage analysis.
- [x] **Tests**: runtime model, event dispatch, roundtrip parser, and analyzer
      tests.
- [x] **Documentation**: `docs/design/SCRATCH_RUNTIME_IMPLEMENTATION.md` and
      `docs/design/SCRATCH_ANALYZER.md`.

### Roundtrip & Semantic Validation Framework (`scratcharch-validation`)

The formal framework that proves Scratch pipeline conversions preserve semantics
as far as the IR models behavior: `SB3 → ScratchGraph → transform → ScratchGraph
→ SB3` and `ScratchGraph → SAIR → ScratchGraph`. Verification is IR-level, never
byte-level. Spec contract: `docs/specification/SCRATCH_SEMANTICS.md` (§4
fidelity table, §5 roundtrip contract).

- [x] **Semantic model spec** (Part 1): `docs/specification/SCRATCH_SEMANTICS.md`
      defines observable behavior, fidelity buckets, and the equivalence
      contract the framework verifies.
- [x] **SemanticNormalizer** (Part 2): `scratcharch-scratchgraph::semantic`
      reduces a `Project` to `NormalizedProject`, erasing block IDs,
      declaration ordering, sprite order, broadcast declaration sites, and
      value noise (`-0.0` = `0.0`) so equality means semantic equality
      (7 unit tests).
- [x] **Categorized semantic diff** (Part 3): `scratcharch-analyzer::diff`
      (`semantic_diff` / `semantic_diff_normalized`, `DiffResult`/`DiffEntry`/
      `DiffFormat`) classifies differences as `Added`, `Removed`, `Changed`,
      `Moved`, `ScopeChanged`, `ControlFlowChanged`, or `RuntimeChanged`, with
      LCS body alignment and severity aggregation (12 unit tests).
- [x] **Roundtrip validation crate** (Part 2/6): `scratcharch-validation`
      composes graph validation (`validate_graph`), the SB3 byte roundtrip
      (`roundtrip_sb3`), semantic preservation, and per-pass soundness verdicts
      into `verify_project`.
- [x] **Roundtrip test matrix** (Part 4): 31 tests under
      `tests/roundtrip/` (basic, procedures, recursion, events, lists, memory)
      prove native programs round-trip semantically and that documented lossy
      code (ABI frame/heap) is *detected* as lossy, never silently blessed.
- [x] **Transform preservation verdicts** (Part 5): 17 tests under
      `tests/preservation.rs` run each pass and forbid its disallowed changes —
      deleting a live broadcast receiver / reachable procedure (DCE), folding a
      boolean context (constant folding), dropping a still-referenced variable
      (variable analysis), removing a non-empty control block (empty-block
      removal).
- [x] **`scratcharch verify` subcommand** (Part 6): parse → graph validation →
      SB3 roundtrip → semantic preservation → transform preservation, text and
      JSON report (verify engine 4 unit tests, CLI 5 end-to-end tests).
- [x] **Differential corpus** (Part 7): `tests/corpus/scratch/` commits 6
      representative loadable `project.json` fixtures + a `manifest.json`
      catalog covering basic, events, procedures, recursion, lists, memory.
      The harness loads each through the CLI `.json` path, validates it, runs
      the full verify gate, and checks it against its canonical IR builder;
      regeneration is env-gated and deterministic (3 tests + 1 ignored).
- [x] **Semantic diff documentation** (Part 8): `docs/design/SEMANTIC_DIFF.md`
      rewritten to the categorized API and real `scratcharch diff` CLI.
- [x] **Roundtrip validation documentation** (Part 8):
      `docs/design/ROUNDTRIP_VALIDATION.md` documents the checks, soundness
      contracts, test layers, corpus, and the deliberately-reported boundaries.

### LLVM Compatibility v0.1 (`scratcharch-llvm` / `scratcharch-ir` / `scratcharch-sair-interpreter`)

Milestone goal: push *real* clang/LLVM IR through the toolchain as far as
possible, not to implement the widest possible opcode surface. Compatibility
matrix and status: `docs/specification/LLVM_COMPATIBILITY.md`.

- [x] **i64 type support** (Part 2): `IrType::I64` end-to-end on the SAIR
      interpreter — typed constants, wrapping arithmetic, conversions,
      `alloca`/GEP over `[N x i64]`. Floating-point LLVM types stay explicitly
      unsupported/reserved with a clear parse-time diagnostic.
- [x] **Integer conversions** (Part 3): `zext`/`sext`/`trunc`/`bitcast` map to
      SAIR `CastOp`s at any supported width.
- [x] **Control flow** (Part 4): `select` → SAIR `Select`, `switch` → per-case
      `eq` + `CondBranch` chain to a shared default, `unreachable` → SAIR
      `Unreachable`. Implemented on SAIR terms only — no dependency on
      Scratch-specific behavior.
- [x] **Pointer ↔ integer conversions** (Part 5): `ptrtoint`/`inttoptr` per
      MEMORY/ABI (an address is an integer).
- [x] **DataLayout unification** (Part 6): byte-address GEP model — every GEP is
      a single dynamic byte offset over `i8` elements, offsets folded via
      `scratcharch_target::layout`; duplicated type-size code removed.
- [x] **Signed division/remainder exactness**: `sdiv`/`srem` lower exactly
      (trunc-toward-zero, dividend sign) via a magnitude expansion
      (`translate_signed_divrem`); `udiv`/`urem` map one-to-one to SAIR
      `Div`/`Rem` at any width incl. i64. Fixed two silent-wrong-result bugs:
      `srem` previously lowering to a quotient, and both ops ignoring signs.
- [x] **Bit intrinsics** (Part 7): `llvm.bswap`/`llvm.ctpop`/`llvm.ctlz`/
      `llvm.cttz` at widths 8/16/32/64 resolved by the SAIR interpreter as pure
      reference expansions over runtime values (no new ISA). `ctlz`/`cttz`
      ignore the `i1 is_zero_undef` immarg (no poison). Unknown families and
      widths raise an explicit `UnsupportedInstruction`.
- [x] **Real-world clang corpus** (Parts 8–9): committed `intrinsics.{c,ll}` and
      `signed.{c,ll}` fixtures under `tests/c_programs/` (real clang 19 output)
      plus fresh-clang recompile harness entries.
- [x] **Explicit diagnostics**: unsupported instructions/types/intrinsics/widths
      error out — never silently dropped. 47 translator tests, 12 pipeline, 12
      corpus, 10 runtime (`scratcharch-llvm`).
- [x] **Documentation**: `docs/specification/LLVM_COMPATIBILITY.md` matrix and
      updated `docs/design/LLVM_TRANSLATION.md`.

**Deferred / known gaps at v0.1** (all addressed in v0.2 below): signed
`icmp slt/sgt/sle/sge` translated to the unsigned bit-pattern compare (exact
only for non-negative operands); the single-cell VM could not execute `i64` (two
cells) or `select`; bit intrinsics were interpreter expansions, not yet ISA
sequences.

### LLVM Compatibility v0.2 — Semantic Correctness & Target Completeness (`scratcharch-llvm` / `scratcharch-ir` / `scratcharch-sair-interpreter` / `scratcharch-driver`)

Milestone goal: make the supported subset *semantically exact* and complete
interpreter **and** VM coverage, with every unsupported corner reported
explicitly — never a silent approximation. Status matrix:
`docs/specification/LLVM_COMPATIBILITY.md`; current bottlenecks:
`docs/design/LLVM_COMPATIBILITY_STATUS.md`.

- [x] **Six-level compatibility classification** (Part 1): every matrix row is
      `Supported` (exact on interpreter and VM), **P** (interpreter-exact), **VU**
      (VM-unrepresentable), **IU** (interpreter-only), or unsupported — `Supported`
      is claimed only where *both* execution surfaces are exact.
- [x] **Exact signed comparisons** (Part 2): `icmp slt/sgt/sle/sge` expand via
      the sign-bit identity (`slt(a,b) = sign(a)!=sign(b) ? sign(a) : a <u b`),
      exact for negatives and mixed signs at `i8/i16/i32/i64`; real-clang
      `signedcmp.{c,ll}` fixture proves it (checksum 59).
- [x] **Multi-cell i64 on the VM** (Part 3): `i64` is two 32-bit limbs for
      add/sub/compare/cast/load/store/select/phi; limb count comes from
      `TargetProfile::cells_for_type` (never hardcoded); profile-driven
      rejection when the split is not two words. `i64` mul/div/rem stay rejected
      with an explicit diagnostic.
- [x] **Memory intrinsics** (Part 4): `llvm.memcpy`/`memmove`/`memset` handled by
      the SAIR interpreter's memory-intrinsic family; the VM rejects modules that
      need them with a named diagnostic rather than running a bodyless call.
- [x] **Phi as first-class SAIR Phi** (Part 5): LLVM `phi` parses and translates
      one-to-one; predecessor labels are remapped to SAIR block names; runs on
      the interpreter and lowers through VM edge copies.
- [x] **Module-level globals** (Part 6): data globals lower to a `StaticData`
      image with `@name` → absolute-address constants; word-granular segments are
      VM-exact, byte-granular segments run exactly on the interpreter and are
      rejected by the VM with a "sub-word or byte" diagnostic.
- [x] **Indirect calls rejected** (Part 7): calls through a function-pointer value
      produce an explicit `UnsupportedInstruction` naming the missing ABI — never
      a wrong dispatch.
- [x] **Real LLVM corpus** (Part 8): committed real-clang fixtures
      `tests/c_programs/*.{c,ll}` cover signed comparisons, `i64` arithmetic,
      structs, arrays, globals, bit/memory intrinsics, and calls. A per-fixture
      five-surface record (`scratcharch-pipeline/tests/llvm_corpus_surfaces.rs`)
      pins parser / SAIR / interpreter / VM / Scratch-lower verdicts.
      `phi` loops are not part of clang `-O0` output; loop-carried `phi` is
      exercised by the hand-written VM corpus and focused translator tests,
      recorded honestly.
- [x] **Differential correctness harness** (Part 9): the corpus is also run
      through a real C compiler (native reference); native exit code, SAIR
      interpreter, and ISA VM must all agree per fixture — and where the VM is
      unsupported the expected outcome is the pinned diagnostic, never a silent
      fallback.
- [x] **Documentation** (Part 10): `LLVM_COMPATIBILITY.md` refreshed for v0.2,
      `LLVM_TRANSLATION.md` de-staled, `ROADMAP.md` updated, and a new
      `LLVM_COMPATIBILITY_STATUS.md` records current bottlenecks.

**Deferred / known gaps at v0.2**: bitwise ops (`and`/`or`/`xor`/shifts) have no
SAIR form and are rejected; floating-point ops/types are reserved; indirect
calls have no function-pointer ABI; `switch` stays a linear `eq` chain;
VM `i64` mul/div/rem, byte-granular globals, and `llvm.*`/runtime intrinsics are
interpreter-only and rejected on the VM with named diagnostics. Each is
documented in the matrix — nothing is silently approximated.

### Testing

- [x] All tests pass with 0 warnings and 0 clippy errors
- [x] v0.2 gate (2026-09-09): `cargo test --workspace` = 497 passed, 0 failed
      (1 ignored); `cargo clippy --workspace --all-targets` = 0 warnings;
      `./scripts/run_c_tests.sh` = 1 passed, 0 failed.

## In Progress

(none — v0.2 delivered; see the commit proposals in the milestone report)

## Future (v0.3+)

### Short-term

- [x] **Bitwise operations** (`and`, `or`, `xor`, `shl`, `lshr`, `ashr`) —
      landed in the v0.3 milestone (bitwise + trap slice): SAIR/ISA bitwise ops
      and `Trap`, LLVM translator + parser wiring, sub-32 `ashr` sign-fill fix,
      software-helper shifts on the VM, and the `unreachable` trap model. Proven
      by the `bitwise` corpus fixture and 17 interpreter/VM differential tests
      (`vm_differential_tests.rs`); see EXECUTION_MODEL.md §5.7
- [ ] **Floating point**: `fadd`/`fsub`/`fmul`/`fdiv` (SAIR has `f64`; the LLVM
      frontend still rejects float types)
- [ ] **VM i64 mul/div/rem and widening** — two-limb add/sub/cmp/select/phi are
      done; a widening multiply/divide ISA would lift the remaining `i64` gap
- [ ] **VM runtime intrinsic linking** — lower or link `llvm.memcpy`/`memmove`/
      `memset` and `__scratcharch_*` calls to run on the VM instead of the
      interpreter-only boundary
- [ ] **Function-pointer ABI / indirect calls** (currently rejected at parse time)

### Medium-term

- [ ] **Advanced optimization passes**
  - Function inlining
  - mem2reg / promote memory to registers
  - Loop optimization
  - Strength reduction
  - Common subexpression elimination

- [ ] **Intrinsic lowering to ISA/VM**
  - Reference expansion of `bswap`/`ctpop`/`ctlz`/`cttz` is done on the SAIR
    interpreter; lowering to concrete ISA sequences for the single-cell VM
    remains
  - Saturating arithmetic
  - Overflow-checked arithmetic

- [ ] **Function pointers and indirect calls**
  - Hidden `!fnptr` parameter
  - Virtual address dispatch

### Long-term

- [ ] **LLVM backend for SAIR** (full target machine registration)
- [ ] **Multi-profile support**
  - sa64 (native 64-bit) profile
  - Parametric profile configuration

- [ ] **Runtime library extensions**
  - Heap allocator (`malloc`, `free`, `realloc`)
  - I/O intrinsics
  - Math library helpers (widening multiply, division helpers, etc.)

- [ ] **TurboWarp backend**
