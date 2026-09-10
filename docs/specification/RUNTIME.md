# ScratchArch Runtime Library (SART) v0.2

> Specification version: **v0.2**
> Status: normative for the `sa48` reference profile; portable across profiles.
> Companion documents: [`ISA.md`](./ISA.md), [`ABI.md`](./ABI.md),
> [`MEMORY.md`](./MEMORY.md), [`EXECUTION_MODEL.md`](./EXECUTION_MODEL.md),
> [`TARGET_PROFILE.md`](./TARGET_PROFILE.md).
>
> **v0.2** adds the machine-readable `IntrinsicSignature` (§4), the shared
> bit-math leaf (§4.1), and the ISA-VM integration — load-time runtime
> resolution and the distinct VM failure categories (§6). No routine's semantics
> changed and no ISA instruction was added.

---

## 1. What the runtime is

The **ScratchArch Runtime Library (SART)** is a software runtime that executes
on top of ScratchArch. It is **not part of the ISA**. It provides portable
reference implementations of common low-level routines that compilers and
frontends need but that are not profitable to encode as individual machine
instructions.

SART is intentionally:

- **Architecture-independent**: It operates only through the abstract
  [`ByteMemory`] trait and uses only SAIR-visible types.
- **Backend-neutral**: It contains no Scratch-specific, LLVM-specific, or
  VM-specific code.
- **Language-agnostic**: The same routines can be called from C, Rust, Zig, or
  any other frontend that targets ScratchArch.

The runtime lives in the `scratcharch-runtime` crate and is consumed by the
SAIR interpreter (and, in the future, by the ISA lowerer) through an intrinsic
registry.

---

## 2. Relationship with other specifications

### 2.1 ISA

SART is **not** part of the ScratchArch ISA. The ISA defines the instruction
set, register/cell model, and execution semantics of the abstract machine. The
runtime is implemented *on top of* those primitives.

No ISA instruction is added, removed, or changed by this document.

### 2.2 ABI

SART follows the ScratchArch ABI when it is invoked from generated code:

- Arguments are passed according to the ABI calling convention.
- Pointers are 32-bit addresses in the ScratchArch address space.
- Return values use the same registers/cells as ordinary functions.

Because the runtime is currently invoked through the SAIR interpreter's
intrinsic dispatcher, the ABI details are handled by the interpreter's call
implementation. When the runtime is later lowered to ISA, it will use the same
convention as any other function.

### 2.3 MEMORY

All runtime memory routines operate through the [`ByteMemory`] trait. This trait
exposes byte-addressable load and store operations. Implementations perform
their own bounds checking and report [`RuntimeError::MemoryOutOfBounds`] for
invalid accesses.

The runtime is **little-endian neutral**: it manipulates individual bytes and
never assumes a particular multi-byte layout.

### 2.4 EXECUTION_MODEL

Runtime routines are ordinary computations in the ScratchArch execution model.
They do not introduce new control-flow primitives. `abort`, `panic`, and `trap`
currently return runtime errors; a backend may lower them to halting sequences,
hardware traps, or host exceptions without changing their portable semantics.

---

## 3. Provided routines

### 3.1 Memory routines

| Function | Signature (Rust) | Semantics |
| -------- | ---------------- | --------- |
| `memcpy` | `fn(&mut M, dest: u32, src: u32, n: usize) -> Result<u32, _>` | Copy `n` bytes from `src` to `dest`. Source and destination must not overlap. |
| `memmove` | `fn(&mut M, dest: u32, src: u32, n: usize) -> Result<u32, _>` | Copy `n` bytes from `src` to `dest`, correctly handling overlapping regions. |
| `memset` | `fn(&mut M, dest: u32, c: u8, n: usize) -> Result<u32, _>` | Set `n` bytes starting at `dest` to `c`. |
| `memcmp` | `fn(&M, s1: u32, s2: u32, n: usize) -> Result<i32, _>` | Compare the first `n` bytes of `s1` and `s2`; returns `<0`, `0`, or `>0`. |

### 3.2 String routines

All string routines operate on C-style null-terminated strings.

| Function | Semantics |
| -------- | --------- |
| `strlen` | Return the length of a string, excluding the null terminator. |
| `strcmp` | Compare two strings lexicographically as unsigned bytes. |
| `strcpy` | Copy a null-terminated string, including the terminator. |
| `strncpy` | Copy up to `n` bytes; pad the remainder with zeros. |

### 3.3 Panic / abort

| Function | Semantics |
| -------- | --------- |
| `abort` | Abruptly terminate execution. Signals `RuntimeError::Abort`. |
| `panic` | Report a fatal logic error. Signals `RuntimeError::Panic` with a message. |
| `trap` | Execute a debug/breakpoint trap. Signals `RuntimeError::Trap`. |

These functions are defined to return errors in the portable reference
implementation. A backend is free to translate them into non-returning control
flow.

---

## 4. Intrinsic registry

Runtime routines are exposed to SAIR as **intrinsics** with ScratchArch-specific
names. The naming convention is:

```text
__scratcharch_<name>
```

Examples:

- `__scratcharch_memcpy`
- `__scratcharch_memset`
- `__scratcharch_strlen`
- `__scratcharch_abort`

Frontends must use these names. The runtime does **not** hardcode LLVM builtin
names such as `llvm.memcpy` or `llvm.memset`; it is the frontend's
responsibility to map frontend/library names to ScratchArch intrinsic names.

The registry is represented by [`IntrinsicRegistry`] in
`crates/scratcharch-runtime/src/intrinsics.rs`. It maps intrinsic names to
functions of type:

```rust
fn(&mut dyn ByteMemory, &[u32]) -> Result<IntrinsicResult, RuntimeError>
```

All arguments (pointers, lengths, integers) are passed as `u32`. The intrinsic
interprets them according to its contract.

Every entry also carries an **`IntrinsicSignature`**: the operand-stack shape of
the call in 32-bit words.

```rust
struct IntrinsicSignature { arg_words: u8, result_words: u8 }
```

`arg_words` is how many operand-stack cells the call consumes and
`result_words` how many it leaves (`0` for a void routine). The signature is
declared at registration time (`register_with_signature`, or the
`word_args(n)` / `void()` shorthands) and read back with
`IntrinsicRegistry::signature(name)`. An interpreter that passes its arguments
as a fresh slice does not need it, but a **stack machine** does: the VM must
know how many cells to pop before the call and how many to push after, and it
must know that *before* running the routine. The signature is the single
machine-readable source of that arity, so the two engines cannot disagree about
a routine's cell shape.

The signature says nothing about the routine's *semantics* — that stays in the
function body, which both engines share.

### 4.1 Shared bit-math leaf

`crates/scratcharch-runtime/src/bitops.rs` holds one neutral, dependency-free
leaf the engines share for the `llvm.*` bit intrinsics:

```rust
fn bit_intrinsic_value(kind: BitIntrinsicKind, width: u32, value: u64) -> Option<u64>
```

It masks `value` to `width` bits, returns `width` for `ctlz`/`cttz` of `0`
(ScratchArch's no-poison policy — see LLVM_COMPATIBILITY.md §8.3) and reverses
the low `width/8` bytes for `bswap`; `width` must be one of `8/16/32/64`, and
any other width returns `None`. It is deliberately *not* a registered intrinsic
and carries no `llvm.*` name string: the interpreter's bit-intrinsic handler and
the VM's load-time runtime resolver both call it, so the two engines evaluate
these intrinsics bit-for-bit identically instead of each re-implementing them.

---

## 5. Integration with the SAIR interpreter

When the SAIR interpreter executes a `Call` instruction whose callee is not
defined in the current module, it dispatches the call to the runtime intrinsic
registry. This allows LLVM IR files that declare external runtime functions
(e.g. `declare void @__scratcharch_abort()`) to execute end-to-end without
requiring those functions to be translated into SAIR.

The interpreter wraps its internal byte memory in an implementation of
[`ByteMemory`] so that runtime routines can read and write the same address
space as ordinary SAIR load/store instructions.

---

## 6. Integration with the ISA VM

The ISA is frozen, and SART adds **no** ISA instruction. A program that calls a
bodyless runtime routine still carries an ordinary named ISA `Call`; the VM
resolves that name **once, at load time** (`scratcharch-vm/src/runtime.rs`) and
rewrites the call to an internal `Code::CallRuntime` entry pointing at its
runtime table. The execute loop then dispatches on the resolved *kind* — an
enum match — never on a name string, and never per execution.

Resolution order mirrors the interpreter's dispatch tiers exactly:

1. **SART builtins** — looked up in the same `IntrinsicRegistry` the
   interpreter consults; the entry's `IntrinsicSignature` supplies the
   operand-stack arity.
2. **`llvm.memcpy`/`memmove`/`memset` variants** the translator leaves as calls
   (runtime-length, volatile, oversized) — resolved to a flat
   copy/move/set op with the interpreter's contiguous-range,
   null-destination, and as-if-through-a-temporary rules.
3. **`llvm.bswap`/`ctpop`/`ctlz`/`cttz.iN`** — evaluated through the shared
   `bit_intrinsic_value` leaf (§4.1).

The VM implements [`ByteMemory`] over its linear memory for step 1, so the SART
routine body executed on the VM is the *same function* the interpreter runs. A
name that resolves to none of the three tiers stays a load-time error
(`undefined function: <name>`); the interpreter reports the same program as an
unknown intrinsic when the call is reached. Neither engine approximates.

**Failure categories.** `RuntimeError` maps onto distinct VM failure categories —
`Abort`, `Panic`, `Trap` (runtime), and the ISA's own `DivisionByZero` — so a
runtime abort is never reported as the ISA `unreachable` trap and vice versa.

**Compiler-synthesized helpers are not runtime routines.** The `__sair_*`
lowering helpers (EXECUTION_MODEL.md §5.7–§5.8) are appended to the program by
the ISA lowerer and never pass through the registry; the registry path serves
only calls the *source* program makes to a bodyless named function.

---

## 7. Future expansion

Future versions of SART may add:

- Heap allocator routines (`malloc`, `free`, `realloc`).
- I/O intrinsics for host communication.
- Math library helpers (`__scratcharch_mulodi4`, division helpers, etc.).
- Setjmp/longjmp or stack unwinding primitives.
- Profile-specific optimized variants selected by the backend.

All additions must remain architecture-independent and must not modify the ISA,
ABI, MEMORY, EXECUTION_MODEL, TARGET_PROFILE, or SAIR semantics.

A new runtime routine is added by registering it (with its `IntrinsicSignature`)
in `scratcharch-runtime`; **both** engines then pick it up, the interpreter
through its dispatch and the VM through its load-time resolver — no per-engine
implementation, and no change to the ISA.
