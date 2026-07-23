# ScratchArch ABI (Application Binary Interface)

> Specification version: **v0.1 (draft for freezing)**
> Status: normative for the `sa48` reference profile; parametric for all profiles.
> Companion documents: [`ISA.md`](./ISA.md), [`MEMORY.md`](./MEMORY.md).

---

## 1. Motivation

The ISA defines the instruction set and the type system. The ABI defines how
those instructions compose into **function calls**: how values cross the boundary
between caller and callee, how activations are created and destroyed, and how
the stack is managed.

The predecessor project, `llvm2scratch`, invented a calling convention for a
runtime (Scratch) that has no native procedure return values, no call-stack
introspection, and limited recursion depth. That convention — return-address
passing, a global return-value slot, save/restore of live values around calls —
was a prototype necessity. Its stable insights are:

1. **Value decomposition.** A logical integer `iN` that does not fit in a single
   cell must be split into multiple cells. The decomposition is a fundamental
   ABI property: it defines how arguments, return values, and memory operands
   are physically represented.

2. **Per-activation register state.** Recursion demands that each call has its
   own copy of virtual registers. The architecture must define what "own copy"
   means and what obligations the runtime has to preserve them.

3. **Argument transmission.** On a runtime with no native parameter passing
   (Scratch custom blocks are a workaround, not a primitive), the ABI must
   specify how caller values arrive at callee parameters.

4. **Return address dispatch.** When a function can be called from multiple
   call sites, the return must reach the correct continuation. The prototype
   encoded this as a numeric return-address parameter and a binary-search
   dispatch tree.

ScratchArch's ABI extracts these insights into a **runtime-independent calling
convention**. It does not assume Scratch procedures, project.json, or any
particular runtime mechanism. It assumes only the ISA's cell model and the
flat address space of [`MEMORY.md`](./MEMORY.md).

### What problem the ABI solves

| Problem from the prototype | ABI-level resolution |
|---|---|
| "Return value" was a global Scratch variable `!return value` | Return values are transmitted through a well-defined output slot of the activation; the storage mechanism is a runtime concern |
| "Save registers to local stack" was a Scratch-list workaround | The ABI defines caller-save semantics for all registers; the runtime decides whether to spill to a stack, allocate fresh storage, or use a persistent namespace |
| "Return address" was a hidden integer parameter to every non-leaf function | Return-address dispatch is formalized as a hidden parameter with defined semantics; not all calls need it (direct calls with a single return site may elide it) |
| "Function pointer dispatch" searched over a binary tree of known addresses | Function pointer calls pass a hidden `fnptr` parameter; the dispatch is a runtime mechanism, not an architectural leak |
| `InferredValue::Single` vs `Indexed` was a Rust enum | Cell decomposition is a **derived property** of a type under a profile, not a separate value kind |

---

## 2. Value representation

### 2.1 Cell decomposition

Every typed value is realized as an ordered sequence of one or more **cells**.
A cell is the fundamental scalar unit of computation on the ScratchArch
Abstract Machine (SAM). The number of cells needed for a value of type `T`
under profile `P = (W, Wptr)` is:

| Type `T` | `cells(T, P)` | Notes |
|---|---|---|
| `iN` (N ≤ W) | 1 | Single-cell value |
| `iN` (N > W) | `ceil(N / W)` | Multi-cell, little-endian order |
| `ptr` | `ceil(Wptr / W)` | Pointer is a `Wptr`-bit integer |
| `half`, `float`, `double`, `fp128` | 1 | Float types occupy one cell at double-or-better precision |
| `[M × E]` | `M × cells(E, P)` | Each element decomposed independently |
| `{T₁, …, Tₙ}` (aligned) | Σ `cells(Tₖ, P)` | Fields in declaration order; alignment padding does not contribute cells |
| `<{T₁, …, Tₙ}>` (packed) | Σ `cells(Tₖ, P)` | Same as aligned (padding is a memory concept, not a cell concept) |

> **Why separate cell count from byte size?** A cell is a register concept — it
> is the unit of computation. A byte is a memory concept — it is the unit of
> storage. For `sa48` (W=48), an `i32` occupies 1 cell but 4 bytes in memory.
> Defining `cells(T, P)` independently of `sizeof(T)` (§4 of MEMORY.md) keeps
> the ABI runtime-independent: a native VM uses one 64-bit register per cell;
> a float-substrate runtime uses one Scratch variable per cell.

> **Why cells instead of bytes?** On a float-substrate runtime the "cell" is a
> double-precision number stored in a Scratch variable. On a native VM the cell
> is a machine word. Both are "a scalar that can hold an integer up to `W` bits
> exactly." Defining the ABI in terms of cells — not bytes, not Scratch slots —
> keeps the convention runtime-independent. Only the memory model (MEMORY.md)
> needs to talk about bytes and addresses.

### 2.2 Cell ordering

Multi-cell values use **little-endian** ordering: cell 0 holds the least
significant `W` bits, cell 1 holds the next `W` bits, and so on. For a value
`v` of type `iN` decomposed into `k = ceil(N / W)` cells:

```
cell[i] = (⟦v⟧ >> (i * W)) mod 2^W    for i = 0, 1, ..., k-1
```

The most significant cell may have unused high bits (when `N` is not a multiple
of `W`). Those bits are **undefined** and may be anything; arithmetic results
depend only on the logical value.

> **Realization note (sa48).** The prototype stored multi-cell values in
> Scratch variables named `%varname:0`, `%varname:1`, etc., with `N ≤ W` in a
> single variable. This is one valid realization of cell decomposition. The ABI
> does not require any particular naming scheme; it requires that the runtime
> can transmit and store `cells(T, P)` independent storage units per value.

### 2.3 Floats and cells

Floating-point types (`half`, `float`, `double`, `fp128`) are defined by
IEEE-754 semantics but always occupy **one cell** in storage. A profile whose
cell holds double-precision (like `sa48`) stores all float types as doubles:
precision loss from `fptrunc`/`fpext` is observable only in rounding, not in
storage width.

> **Design decision.** Floats get one cell regardless of width because (a) on
> float-substrate runtimes all numbers are doubles anyway, and (b) splitting a
> float across multiple cells would require recomposing bits, which is exactly
> what IEEE-754 decomposition (§4.4 of ISA.md) handles for `bitcast`. Keeping
> floats single-cell simplifies arithmetic at the cost of a more complex
> `bitcast`. This is the right tradeoff for the reference profile.

### 2.4 Pointers and cells

A pointer is a `Wptr`-bit unsigned integer. It occupies `ceil(Wptr / W)` cells
— one cell in both `sa48` (32-bit pointer, 48-bit cell) and `sa64` (64-bit
pointer, 64-bit cell). Pointers are transmitted, compared, and arithmetic-ed as
integers; the memory model gives them meaning.

---

## 3. Function call semantics

### 3.1 The call instruction

`call` transfers control from the **caller** activation to a **callee**
activation. The ISA defines its shape:

```
%rv = call T @func(T1 %arg1, T2 %arg2, ...)
```

The ABI defines the observable sequence:

1. **Argument placement.** The caller ensures that each argument is available
   in a position the callee will read as its parameter (see §5).
2. **Control transfer.** Execution of the caller pauses; the callee's entry
   block begins executing.
3. **Return.** The callee executes `ret T %rv` (or `ret void`), which
   terminates the callee activation and resumes the caller.
4. **Result consumption.** The caller reads `%rv` from the position the callee
   left it (see §6).

### 3.2 The ret instruction

`ret` terminates the current activation. The sequence:

1. **Return value placement** (if non-void). The callee ensures the return
   value is in the agreed output position.
2. **Control transfer.** The caller activation resumes at the instruction
   immediately after the `call`.

### 3.3 No implicit callee-destroyed state

A `ret` does not implicitly deallocate stack memory, invalidate registers, or
perform cleanup beyond ending the activation. The caller is responsible for
managing the stack pointer if it uses `alloca` (see §8). This is the
architecture-level meaning of the prototype's stack-pointer restore on return.

---

## 4. Activation model

### 4.1 Activation lifetime

Each `call` creates a fresh **activation** — the execution context for one
invocation of a function. An activation has:

- A **register namespace**: all virtual registers defined by the function are
  local to this activation. Two activations of the same function have
  independent register values.
- A **return continuation**: the location (call site) in the caller to resume
  after `ret`.
- An optional **stack frame**: a contiguous range of the address space
  allocated by `alloca` within this activation (see §8).

> **Design decision.** Per-activation registers follow from LLVM SSA semantics:
> a recursive call must not overwrite the caller's values. The prototype solved
> this by saving live values to `!local stack` before calls and restoring after.
> The abstraction — per-activation register namespace — is cleaner: it defines
> *what* must be true, not *how* the runtime achieves it.

### 4.2 Caller-save convention

**All virtual registers are caller-save.** A function call may freely overwrite
any storage backing virtual registers. If the caller needs a value after a
call, it must ensure the value is preserved by its own mechanism:

- On a runtime with unbounded storage (e.g., each activation has its own
  variable namespace), no explicit save/restore is needed — the activation
  boundary provides it.
- On a runtime with shared storage (e.g., all activations share a flat
  variable space), the caller must save live values to the stack before the
  call and restore after.

> **Why caller-save and not callee-save?** SSA values are defined once and may
> have many uses. The set of values live across a call is a property of the
> *caller's* code, which the caller knows statically. A callee cannot know
> which values the caller needs. The prototype's `compute_must_store` function
> computed exactly this set. Formalizing as "caller-save all registers" is both
> simpler and more general: a runtime with per-activation namespaces gets it
> for free; a runtime with flat namespaces has a clear obligation.

### 4.3 Recursion

Recursion is the general case of the activation model: each recursive
invocation creates an independent activation with its own register namespace.
The architecture does not impose a recursion limit; individual profiles may
define `max_activation_depth` based on runtime constraints.

> **Realization note (sa48).** On Scratch/TurboWarp-class runtimes, the
> activation depth is limited by the host's call-stack capacity (~2000 for
> TurboWarp, ~3,000,000 for Scratch 3.0). Deeper recursion requires an
> explicit stack management strategy (e.g., heap-allocated activation records).
> The profile definition documents this limit.

---

## 5. Argument passing

### 5.1 Positional cell transmission

Arguments are passed **positionally** as an ordered sequence of cells. For a
function with parameter types `(T₁, T₂, ..., Tₙ)`, the caller transmits

```
cells(T₁) ++ cells(T₂) ++ ... ++ cells(Tₙ)
```

cells in that order, where `++` is concatenation and `cells(T)` is the number
of cells defined in §2.1.

The callee receives the same sequence as its parameters: the first
`cells(T₁)` cells form parameter 1, the next `cells(T₂)` cells form parameter
2, and so on.

> **Example.** Under `sa48` (W=48), a function `void @f(i64, i8, ptr)` passes
> 2 + 1 + 1 = 4 cells. The first 2 cells carry the low and high parts of `i64`;
> cell 3 carries the `i8`; cell 4 carries the pointer.

### 5.2 Transmission mechanism

The ABI does not mandate *how* cells are transmitted. Conforming mechanisms
include:

- **Binding to named input slots** (e.g., the callee reads from well-known
  register names like `%param0`, `%param1`, ...).
- **Push on a stack** (caller writes cells to stack positions, callee reads
  them).
- **Host-language mechanism** (e.g., Scratch procedure parameters, WebAssembly
  locals, native register-passing conventions).

The only requirement is that the callee receives a faithful copy of each cell
value at the start of its execution.

> **Why not mandate a mechanism?** The prototype passed arguments through
> Scratch custom block parameters — a host-specific mechanism that does not
> exist outside Scratch. Defining a fixed mechanism (e.g., "parameters are
> passed in a reserved area of the address space") would add unnecessary
> overhead for runtimes with native parameter passing. The ABI defines the
> *logical* transmission and leaves the *physical* mechanism to the runtime.

### 5.3 Hidden parameters

Some calls require additional parameters beyond the declared arguments:

| Parameter | When used | Semantics |
|---|---|---|
| `!return.address` | The caller has multiple call sites to the same callee, and the callee needs to return to the correct continuation | A `Wptr`-bit cell uniquely identifying the call site; the callee dispatches `ret` through this value |
| `!fnptr` | Indirect call (`call %ptr(...)`) | The function pointer value being called; the runtime uses it to select the target |
| `!vararg.ptr` | Variadic function call | A `Wptr`-bit address of the variadic argument list on the stack |

Hidden parameters are appended *after* the explicit parameters in cell order.

> **Design decision.** Hidden parameters are not always present. A direct call
> to a function that has only one caller (or whose call sites can be fused)
> may elide `!return.address`. An indirect call where the function pointer
> resolves to exactly one target at compile time may elide `!fnptr`. This
> follows the prototype's optimization: both direct and indirect calls were
> simplified when the target was statically known.

### 5.4 Aggregate arguments

Struct and array arguments are passed **cell-by-cell in memory order** (struct
fields in declaration order, array elements in index order). Each element is
decomposed according to its type.

> **Realization note.** For struct types, the decomposition follows the
> in-memory layout defined in MEMORY.md §4, including alignment padding between
> fields. This ensures that the cell representation of an aggregate argument
> matches its representation in memory, so `load`/`store` on the callee's
> parameter area works correctly.

---

## 6. Return values

### 6.1 Return slot

A function returning type `T` places its return value as `cells(T)` consecutive
cells in a well-defined **return slot**. The mechanism parallels argument
passing:

- The callee writes the return value to the return slot before `ret`.
- The caller reads the return slot after the `call` resolves.

### 6.2 Void functions

`ret void` writes nothing to the return slot. There is no return value.

### 6.3 Multi-cell return values

A multi-cell return value (e.g., `i64` on `sa48`) occupies multiple cells in
the return slot, in little-endian order. The caller reads all `cells(T)` cells.

> **Realization note (sa48).** The prototype stored the return value in a
> global variable `!return value` (with `:0`, `:1` suffixes for multi-cell).
> This is one valid realization of a return slot. An alternative is a
> dedicated register or a callee-allocated buffer. The ABI does not prescribe
> which.

---

## 7. Stack frame convention

### 7.1 Frame as address-space range

When a function uses `alloca`, its stack frame is a contiguous range of the
address space. The frame is logically part of the activation and is reclaimed
on `ret`.

The stack pointer (`!sp`) points to the boundary between allocated and free
stack space. Details are in [`MEMORY.md`](./MEMORY.md) §5.1; this section
defines the ABI contract.

### 7.2 Frame lifetime

1. Before executing the first `alloca`, the function may decrement `!sp` to
   reserve its frame.
2. `alloca` instructions decrement `!sp` further and return the new address.
3. On `ret`, the function restores `!sp` to its value at entry.

> **Why restore `!sp` on ret?** The caller's frame lives above the callee's on
> the stack. If the callee does not restore `!sp`, the caller's subsequent
> `alloca` will reuse the callee's freed space, causing corruption. The
> prototype explicitly restored the stack pointer on return.

### 7.3 Frame pointer (optional)

The architecture does not mandate a dedicated frame pointer register. A
profile may define one, or the runtime may track the frame boundary by saving
`!sp` at function entry.

---

## 8. Function pointers

### 8.1 Representation

A function pointer is a `Wptr`-bit integer representing the **address** of a
function in the function-pointer address region (see [`MEMORY.md`](./MEMORY.md)
§3.5). These are virtual addresses that never appear in memory; they exist only
as register values and are used solely for dispatch.

> **Why virtual addresses?** The prototype assigned each function a unique
> address starting at `memory_size + 1`. These addresses are not in the memory
> array and cannot be loaded from or stored to. This avoids allocating storage
> for code and makes function pointers cheap (they are just integers). The
> architecture formalizes this as a reserved address region.

### 8.2 Dispatch

An indirect call `call T %ptr(...)` proceeds as follows:

1. The caller places `%ptr` as the hidden `!fnptr` parameter (§5.3).
2. The runtime matches `%ptr` against the known function addresses.
3. Control transfers to the matching function.

> **Design decision.** The dispatch mechanism is runtime-defined. A simple
> runtime may use a binary search over a sorted table (prototype). A native VM
> may use a jump table or a `match` expression. The architecture requires only
> that the correct function is called; it does not specify how.

### 8.3 Signature matching

A function pointer call is well-defined only if the target function's signature
matches the call site's signature. If no function with the given address
exists, or if the signature does not match, behavior is **undefined** (matching
LLVM's undefined behavior for indirect calls with mismatched types).

---

## 9. Variadic functions

### 9.1 Calling sequence

A call to a variadic function passes:

1. The named parameters in positional cell order (§5.1).
2. The hidden `!vararg.ptr` parameter (§5.3) — an address in the stack region
   pointing to the variadic argument list.
3. The variadic arguments stored sequentially in the stack region at and after
   the address in `!vararg.ptr`.

### 9.2 Callee view

The callee accesses variadic arguments by reading cells from the address in
`!vararg.ptr`, advancing by `sizeof(T)` (in addressable units) per argument.
The mechanism mirrors LLVM's `va_arg`: the callee advances a pointer through
the argument area.

> **Design decision.** Storing variadic arguments on the stack is a natural
> extension of the flat address space. The prototype used the same approach
> (a hidden `vararg ptr` parameter pointing to stack data). This is
> straightforward for any runtime that implements the memory model.

---

## 10. Examples

### 10.1 Simple direct call (single-cell)

```
define i32 @add(i32 %a, i32 %b) {
  %s = add i32 %a, %b
  ret i32 %s
}

define i32 @main() {
  %r = call i32 @add(i32 1, i32 2)
  ret i32 %r
}
```

**Under sa48:** Each `i32` is one cell. The call passes 2 cells (1 and 2) as
positional arguments. The callee returns 1 cell in the return slot. No hidden
parameters needed — `@add` has one call site, so `!return.address` is elided.

### 10.2 Multi-cell argument

```
define i64 @double_it(i64 %x) {
  %d = mul i64 %x, 2
  ret i64 %d
}
```

**Under sa48:** `i64` decomposes into 2 cells (low 48 bits, high 16 bits). The
call passes 2 cells. The return slot has 2 cells. The callee performs 48-bit
multiplication on the low cell and a 16-bit wide multiplication on the high
cell with carry from the low cell.

### 10.3 Indirect call

```
define void @call_twice(ptr %fn, i32 %x) {
  call void %fn(i32 %x)
  call void %fn(i32 %x)
  ret void
}
```

**Under sa48:** Each call passes 1 cell (`i32`) as the explicit argument, plus
a hidden `!fnptr` cell carrying `%fn`. The runtime dispatches to the function
whose address matches `%fn`. Each call also needs `!return.address` (the two
calls are distinct call sites, so they need distinct return addresses) —
2 hidden cells per call: `!fnptr` and `!return.address`.

### 10.4 Recursive activation

```
define i32 @fact(i32 %n) {
  %c = icmp eq i32 %n, 0
  br i1 %c, label %base, label %recur
base:
  ret i32 1
recur:
  %n1 = sub i32 %n, 1
  %r = call i32 @fact(i32 %n1)
  %p = mul i32 %n, %r
  ret i32 %p
}
```

**Under sa48:** Each recursive call to `@fact` creates a fresh activation with
its own `%n`, `%c`, `%n1`, `%r`, `%p` registers. The runtime must preserve
the outer activation's registers across the inner call (per §4.2,
caller-save). On a flat-variable-space runtime, `%n` must be saved to the
stack before the recursive call and restored after.

---

## 11. Relationship with LLVM

| LLVM concept | ScratchArch ABI |
|---|---|
| Function parameters as `Argument` values | Positional cell sequence (§5.1) |
| `call` instruction arguments | Same logical transmission, decomposed into cells |
| `ret` instruction | Same: callee writes return slot, transfers control |
| `va_arg` | `!vararg.ptr` + pointer advance (§9) |
| Indirect call via `@llvm.call.preallocated` | `!fnptr` hidden parameter (§8.2) |
| Tail call | Recognized but not optimized in v0.1 (prototype rejected tail calls); reserved for future ABI extension |
| `inalloca` | Not supported in v0.1; future ABI may define an equivalent |
| `sret` (struct return) | Not needed: aggregates are decomposed into cells (§5.4) |
| `byval` | Not needed: arguments are passed by value as cells |
| `nest` | Not supported in v0.1 |
| `swiftcc`/`preserve_mostcc`/... | Not applicable; ScratchArch has a single calling convention for v0.1 |

**Backend obligations.** An LLVM→ScratchArch backend must:
- Decompose each argument type into cells according to the target profile.
- Emit the transmission sequence (cell-by-cell) and reassembly on the callee side.
- Insert hidden parameters where needed (indirect calls, multi-site returns, varargs).
- Generate save/restore code for values live across calls on profiles with flat storage.

---

## 12. Known limitations for v0.1

The v0.1 reference implementation realizes the calling convention described
above with the following restrictions:

- **Hidden parameters** (`!return.address`, `!fnptr`, `!vararg.ptr`) are
  specified in §5.3 and §8.2 but are not yet emitted or consumed by the v0.1
  toolchain. Direct calls, single-return-site calls, and non-variadic functions
  work without them; indirect calls, multi-site return-address dispatch, and
  varargs are reserved for v0.2.
- **Function pointers** are specified as virtual addresses in §8 but are not
  yet supported by the LLVM translator or SAIR interpreter.
- **Variadic functions** (§9) are not yet supported.
- **Aggregate arguments** are passed as opaque pointer values in the current
  test programs; full cell-by-cell decomposition per §5.4 is not yet enforced
  by the translator.

These limitations describe the current implementation surface; they do not
change the ABI semantics.

## 13. Future extensions

- **Tail calls.** Formal tail-call optimization: a `ret` immediately following
  a `call` to the same function (or a function with an equivalent frame) may
  recycle the current activation instead of creating a new one. The prototype
  detected but rejected tail calls; v0.2 may define when they are safe.
- **Struct return (`sret`).** For large aggregates where cell decomposition
  would be expensive, a caller-allocated buffer pointer could be passed as a
  hidden parameter. This is the LLVM `sret` convention and is reserved for
  v0.2.
- **Multiple return values.** LLVM's `call` returns one value. A future
  extension could allow returning multiple values through multiple output
  slots (similar to `llvm.mul.with.overflow`'s two-output intrinsic pattern).
- **Custom calling conventions.** Profiles may define additional convention
  keywords (e.g., `scratcharch_fastcc`) for specific runtime needs.
- **Unwinding / exceptions.** `setjmp`/`longjmp` and `landingpad` would
  require saving and restoring the full activation state including register
  values and stack frames. The prototype's `setjmp` saved the local stack
  snapshot; a future ABI extension would formalize activation serialization.

---

## 13. Frozen decisions (v0.1)

1. All virtual registers are caller-save (§4.2).
2. Parameters are passed as a positional concatenation of cells (§5.1).
3. Return values occupy a return slot of `cells(T)` cells (§6.1).
4. Hidden parameters (`!return.address`, `!fnptr`, `!vararg.ptr`) are appended
   after explicit parameters and are optional (§5.3).
5. Cell decomposition is little-endian (§2.2).
6. Floats occupy one cell regardless of declared width (§2.3).
7. Pointers are `Wptr`-bit unsigned integers occupying `ceil(Wptr/W)` cells
   (§2.4).
8. Each call creates a fresh activation with independent register namespace
   (§4.1).
9. The stack frame is reclaimed by restoring the stack pointer on `ret` (§7).
10. Function pointer dispatch is runtime-defined; the ABI requires only
    correct target selection (§8.2).
