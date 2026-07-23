# ScratchArch Development Roadmap

> Last updated: 2026-07-23 (Scratch Backend Foundation v0.3 completed)
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

### Testing

- [x] All tests pass with 0 warnings and 0 clippy errors
- [x] Test breakdown: see final verification output for the current total.

## In Progress

## Future (v0.3+)

### Short-term

- [ ] **Extended LLVM IR support**
  - Signed comparison predicates with sign-aware lowering
  - Division/remainder lowering
  - Bitwise operations (`and`, `or`, `xor`, `shl`, `lshr`, `ashr`)
  - Phi node parsing and translation
  - Global variable support
  - Indirect function calls
- [ ] **Multi-cell arithmetic execution**
  - Expand ISA lowering for `i64` and wider types on `sa48`
  - Multi-cell add/sub/mul/div and load/store
  - Low/high part extraction and recombination
- [ ] **VM backend runtime linking**
  - Lower or link calls to `__scratcharch_*` runtime intrinsics
  - Reference expansions for `memcpy`, `memmove`, `memset`

### Medium-term

- [ ] **Advanced optimization passes**
  - Function inlining
  - mem2reg / promote memory to registers
  - Loop optimization
  - Strength reduction
  - Common subexpression elimination

- [ ] **Intrinsic lowering**
  - ctpop, ctlz, cttz
  - bswap
  - saturating arithmetic
  - overflow-checked arithmetic

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
