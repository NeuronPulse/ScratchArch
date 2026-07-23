# ScratchArch Runtime Library Design

> Crate: `crates/scratcharch-runtime`
> Companion documents: [`RUNTIME.md`](../specification/RUNTIME.md),
> [`SAIR_INTERPRETER.md`](./SAIR_INTERPRETER.md),
> [`LLVM_TRANSLATION.md`](./LLVM_TRANSLATION.md).

---

## 1. Why the runtime exists

Compilers targeting ScratchArch need more than raw instructions. They need
portable, reusable implementations of routines that every language expects:

- Memory movement and comparison (`memcpy`, `memmove`, `memcmp`).
- String handling (`strlen`, `strcmp`).
- Language-neutral panic and abort paths.
- Compiler helper intrinsics (e.g. widening multiplication, division helpers).

Rather than forcing each frontend to emit inline expansions or forcing the ISA
to grow an unbounded set of special-purpose instructions, ScratchArch provides
a **software runtime library**. Frontends call runtime routines as ordinary
functions; the routines themselves are written once and shared.

---

## 2. Why these functions are not ISA instructions

The ISA is kept small and stable for three reasons:

1. **Implementation cost**: Every ISA instruction must be implemented in every
   backend (interpreter, VM, future LLVM target, future TurboWarp generator).
   Adding a `memcpy` instruction would not reduce this cost; it would just move
   the complexity into the backend.

2. **Generality**: A `memcpy` instruction still needs source, destination,
   length, and overlap semantics. Those details are easier to express as a
   function with a clear contract than as a monolithic instruction.

3. **Portability**: ISA instructions are architecture-defined. Runtime routines
   are software-defined. A routine implemented in portable Rust can be reused on
   every profile and backend without changing the ISA specification.

The runtime therefore occupies the layer *above* the ISA: it is a library, not a
hardware feature.

---

## 3. Design principles

### 3.1 Abstract memory

All routines operate through the [`ByteMemory`] trait:

```rust
pub trait ByteMemory {
    fn load_u8(&self, addr: u32) -> Result<u8, RuntimeError>;
    fn store_u8(&mut self, addr: u32, value: u8) -> Result<(), RuntimeError>;
}
```

This single abstraction makes the runtime independent of:

- The interpreter's `Vec<u8>` backing store.
- The VM's physical memory layout.
- Any future backend's memory representation.

Implementations provide bounds checking, so the runtime routines never perform
unsafe indexing.

### 3.2 Endian neutrality

The runtime works one byte at a time. Multi-byte values are never assembled or
disassembled inside the runtime; that responsibility belongs to the caller
(SAIR instructions or frontend-generated code). This makes the runtime naturally
little-endian neutral and compatible with any byte-addressable memory model.

### 3.3 ScratchArch intrinsic names

Frontends often have their own names for runtime helpers. LLVM has
`llvm.memcpy.p0.p0.i32`; C has `memcpy`; Rust has `core::intrinsics::copy`.
Rather than embedding all of those naming conventions in the runtime, SART
exposes a single, stable set of ScratchArch names:

```text
__scratcharch_memcpy
__scratcharch_memmove
__scratcharch_memset
__scratcharch_memcmp
__scratcharch_strlen
__scratcharch_strcmp
__scratcharch_strcpy
__scratcharch_strncpy
__scratcharch_abort
__scratcharch_panic
__scratcharch_trap
```

A frontend translator (e.g. `scratcharch-llvm`) maps frontend/library names to
these intrinsic names at translation time. The runtime remains frontend-agnostic.

### 3.4 Error model

Every routine returns `Result<T, RuntimeError>`. Errors include:

- Out-of-bounds memory accesses.
- Unterminated strings.
- `abort`, `panic`, and `trap` signals.
- Unknown or misapplied intrinsics.

This design lets the SAIR interpreter turn runtime errors into `InterpError`
gracefully and lets backends decide how to handle fatal conditions.

---

## 4. How future languages reuse the runtime

A new frontend targeting ScratchArch needs only three things to use SART:

1. **Emit calls using ScratchArch intrinsic names** when translating to SAIR, or
   declare external functions in LLVM IR (e.g.
   `declare void @__scratcharch_abort()`).

2. **Provide a [`ByteMemory`] implementation** for its execution environment
   (the interpreter and VM already do this).

3. **Register any language-specific helpers** in [`IntrinsicRegistry`] if the
   default builtins are not sufficient.

Because SART is a plain Rust crate with no LLVM or Scratch dependency, it can be
linked into any tool chain that needs reference expansions or direct execution.

---

## 5. Integration with the interpreter

The SAIR interpreter treats undefined callees as runtime intrinsic calls. When
it encounters a `Call` to a function that is not in the module's function index,
it:

1. Converts each argument `RuntimeValue` to `u32`.
2. Wraps the interpreter memory in a [`ByteMemory`] adapter.
3. Calls `scratcharch_runtime::dispatch_intrinsic(name, ...)`.
4. Converts the [`IntrinsicResult`] back into a `RuntimeValue`.

This keeps the interpreter decoupled from the runtime: the interpreter knows
*that* an external call should be dispatched, but it does not know which
routines exist. That knowledge lives in the runtime registry.

---

## 6. Future work

- **ISA lowering**: When `lower.rs` supports multi-block functions, the runtime
  routines can be lowered to SAIR/ISA call sequences instead of being
  interpreted.
- **Profile-specific variants**: The registry could select optimized routines
  based on `TargetProfile` (e.g. word-sized copies on profiles with wide cells).
- **Heap and I/O**: A future v0.2 runtime may add allocation and host I/O
  primitives behind the same trait/registry design.
