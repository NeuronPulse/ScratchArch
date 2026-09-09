# ScratchArch Execution Model

> Specification version: **v0.1 (draft for freezing)**
> Status: normative for the `sa48` reference profile; parametric for all profiles.
> Companion documents: [`ISA.md`](./ISA.md), [`ABI.md`](./ABI.md), [`MEMORY.md`](./MEMORY.md).

---

## 1. Motivation

The ISA, ABI, and Memory Model define the ScratchArch Abstract Machine (SAM)
statically — what instructions exist, how values cross call boundaries, and how
memory is organized. The **Execution Model** defines how these pieces compose
into a running program: the dynamic semantics of instruction execution, control
flow, function calls, and state management.

This document formalizes the observable behavior of all ScratchArch
implementations. It is the reference for:

- The SAIR interpreter (`scratcharch-sair-interpreter`)
- The ISA VM (`scratcharch-vm`)
- Any future runtime implementation (TurboWarp, native VM, etc.)

The execution model is derived from the existing implementations and is
normative. No implementation may deviate from these semantics without changing
the architecture version.

---

## 2. Value Model

### 2.1 Integer Representation

Integers are stored as **unsigned residues** modulo `2^N` for type `iN`.
The value `v` of type `iN` is always in `[0, 2^N)`.

There is no signed storage format. Signedness is a property of instructions,
not of values. Every instruction that performs signed interpretation derives it
from the unsigned representation via two's-complement:

```
signed(v) = v                     if v < 2^(N-1)
signed(v) = v - 2^N              if v >= 2^(N-1)
```

### 2.2 Signedness Rules

| Instruction | Interprets operands as | Produces |
|---|---|---|
| `add`, `sub`, `mul` | Unsigned mod 2^N | Unsigned mod 2^N |
| `udiv`, `urem` | Unsigned | Unsigned |
| `sdiv`, `srem` | Signed (two's-complement) | Signed, re-encoded as unsigned |
| `icmp eq`, `icmp ne` | Unsigned (bit pattern equality) | i1 |
| `icmp ugt`, `icmp uge`, `icmp ult`, `icmp ule` | Unsigned | i1 |
| `icmp sgt`, `icmp sge`, `icmp slt`, `icmp sle` | Signed (two's-complement) | i1 |
| `zext` | Unsigned, zero-extend | Unsigned |
| `sext` | Signed, sign-extend | Unsigned |
| `trunc` | Unsigned, truncate modulo | Unsigned |

### 2.3 Wrapping Arithmetic

All integer arithmetic is **total and wrapping**. Results are taken modulo `2^N`.
There are no trap-on-overflow semantics.

```
add:  (a + b) mod 2^N
sub:  (a - b) mod⁺ 2^N   (non-negative remainder)
mul:  (a * b) mod 2^N
udiv: floor(a / b)        (b = 0 is undefined)
sdiv: trunc(signed(a) / signed(b)), re-encoded mod 2^N  (b = 0 or INT_MIN/-1 undefined)
urem: a - b * floor(a / b)
srem: signed(a) - signed(b) * trunc(signed(a) / signed(b)), re-encoded
```

LLVM `nsw`/`nuw`/`exact` flags are accepted as optimization hints only.
The architecture always wraps — the flags carry no architectural obligation.

### 2.4 Comparison Behavior

Comparisons take two values of the same type and produce an `i1` result:
`0` (false) or `1` (true).

Mnemonic | Semantics
---|---
`eq`  | a == b
`ne`  | a != b
`ugt` | a > b  (unsigned)
`uge` | a >= b (unsigned)
`ult` | a < b  (unsigned)
`ule` | a <= b (unsigned)
`sgt` | signed(a) > signed(b)
`sge` | signed(a) >= signed(b)
`slt` | signed(a) < signed(b)
`sle` | signed(a) <= signed(b)

### 2.5 Floating Point

Floating-point values follow IEEE-754 semantics for the **declared type**,
not the storage width. A profile whose cell width exceeds the declared type
may store at higher precision, but rounding and special-value behavior must
match the declared type.

| Operation | Semantics |
|---|---|
| `fadd`, `fsub`, `fmul`, `fdiv` | IEEE-754 for declared type |
| `frem` | `a - b * trunc(a/b)` with sign of dividend |
| `fneg` | IEEE-754 negation |
| `fcmp` | Ordered/unordered predicates per IEEE-754 |

### 2.6 Type Widths and Sizes

Type | Logical width | `sizeof(T)` (bytes) | Cell count (sa48, W=48)
---|---|---|---
`i1` | 1 bit | 1 | 1
`i8` | 8 bits | 1 | 1
`i16` | 16 bits | 2 | 1
`i32` | 32 bits | 4 | 1
`i64` | 64 bits | 8 | 2
`ptr` | Wptr bits | ceil(Wptr/8) | ceil(Wptr/W)
`float` | 32 bits | 4 | 1
`double` | 64 bits | 8 | 1

All integer values narrower than 32 bits are zero-extended to 32 bits in
register representation (SAIR interpreter and VM both store them in 32-bit
containers). Operations on narrower types mask the result to the type width.

---

## 3. Program Execution Model

### 3.1 Program Entry

A program begins execution at the **entry function**, designated by the
module's `entry` field. The entry function must have type `() → T` (no
parameters). The program's result is the value returned by the entry function.

```
Program = Module + EntryFunction
         ↓
    Call(EntryFunction)
         ↓
    Execute callee body
         ↓
    Return value → program result
```

### 3.2 Instruction Execution Order

Execution proceeds in **program order**: instructions execute one at a time,
each producing a state transition before the next begins.

```
┌────────────────────────────────────────┐
│  State = (Frame, Memory, CallStack)    │
│                                        │
│  loop:                                 │
│    instr = fetch(block, instr_idx)     │
│    instr_idx += 1                      │
│    execute(instr, state) → state'      │
│    if block has more instructions:     │
│      goto loop                         │
│    execute_terminator(terminator)      │
│      → may change block, call, return  │
│    goto loop                           │
└────────────────────────────────────────┘
```

Each instruction execution is atomic with respect to the current activation:
no other activation observes intermediate states of the current instruction.

### 3.3 Basic Block Execution

A **basic block** is a maximal straight-line sequence of non-terminator
instructions followed by exactly one **terminator**. There is no fall-through
between blocks.

```
Block:
  ┌──────┐
  │ Instr │ ← 0 or more non-terminator instructions
  │ Instr │
  │ ...   │
  └──────┘
       ↓
  ┌──────────────┐
  │ Terminator   │ ← exactly one: Branch, CondBranch, or Return
  └──────────────┘
```

### 3.4 Terminator Semantics

#### `Branch { target }`

Unconditional control flow. Sets the next block to `target` and resets
`instr_idx = 0`. Records the current block as `prev_block` for phi
resolution.

```
prev_block = current_block
current_block = target
instr_idx = 0
```

#### `CondBranch { condition, true_target, false_target }`

Conditional control flow. Reads `condition` as an `i1` value. If `1` (true),
transitions to `true_target`; if `0` (false), transitions to `false_target`.

```
cond = read(condition)
prev_block = current_block
current_block = true_target  if cond == 1
current_block = false_target if cond == 0
instr_idx = 0
```

#### `Return { value: Some(id) | None }`

Terminates the current activation. If a value is present, it is read and
propagated to the caller. Control does not continue in the current activation.

```
return_value = read(value)   if value is Some
activation.destroy()
caller.resume(return_value)  if caller exists
halt()                       if no caller (program end)
```

#### `unreachable`

Must never be executed. At the architecture level, if reached, behavior is undefined
(§8). The **reference implementations** realize this deterministically rather than leaving
it arbitrary:

- On the SAIR interpreter, executing `unreachable` stops the program in a **trap** — a
  well-defined terminal state, distinct from a normal return and from the machine-error
  classes (§7.1).
- On the ISA VM, `unreachable` lowers to the `trap` primitive ([`ISA.md`](./ISA.md)
  Appendix A, §A.5) whose execution model is defined in §5.6.

This is a *realization choice* the architecture permits (§8: an implementation may define
its behavior on UB); it does not make the architecture-level terminator defined.

---

## 4. Function Call Model

### 4.1 CALL Lifecycle

`call` creates a new activation and transfers control to the callee.

```
┌──────────┐         ┌──────────┐
│  Caller  │         │  Callee  │
│          │         │          │
│  1. Evaluate args  │          │
│  2. Save state     │          │
│  3. Transfer ─────→│  4. Init params │
│     control        │  5. Execute     │
│                    │     body        │
│                    │  6. ret ───────→│
│          ←─────────│     value       │
│  7. Read return    │          │
│  8. Continue       │          │
└──────────┘         └──────────┘
```

**Implementation contract (SAIR interpreter):**

1. Evaluate each argument by `read_current_value(arg_id)`.
2. Push a new **Frame** with:
   - `values`: pre-allocated `Vec<Option<RuntimeValue>>` of length
     `func.values.len()`. Parameter `i` is placed in `values[i]`.
   - `current_block`: the function's `entry_block`.
   - `instr_idx`: 0.
   - `prev_block`: None.
3. Execute the callee frame to completion.
4. On return, the callee frame is popped.

There is **exactly one write** of the return value into the caller's frame:
the `Call` instruction handler performs it. The `Return` terminator handler
in the callee's `execute_frame` does **not** write to the caller's frame
and does **not** advance the caller's `instr_idx`.

**Ownership rule:**

```
CALL consumes arguments by value.
CALL produces one return value.
RETURN produces the return value and destroys the callee activation.
Only the CALL instruction handler writes the return value into the caller's
value slot. The RETURN terminator must not touch caller state.
```

### 4.2 RETURN Lifecycle

`ret` terminates the current activation and resumes the caller.

```
1. Evaluate return value (if non-void):
     return_value = read_current_value(value_id)

2. Pop the current frame from the call stack:
     self.frames.pop()

3. If the call stack is now empty:
     Program terminates.
     Return the value (or None for void) as the program result.

4. If a caller frame exists:
     The CALL instruction handler (in execute_instruction) receives the
     return value from call_function() and writes it into the caller's
     value slot at the correct ValueId.

5. The CALL instruction handler then returns, and execute_frame's main
   loop increments the caller's instr_idx once.
```

**Critical invariant (prevents double-write bug):**

The `Return` terminator handler in `execute_frame` must **not**:
- Write the return value into the caller's value array.
- Increment the caller's `instr_idx`.

Both operations are the exclusive responsibility of the `Call` instruction
handler and the caller's `execute_frame` loop. Violating this invariant
causes the return value to be written to the wrong value slot (corrupting
the next instruction's result) or the PC to advance twice.

### 4.3 Recursion

Recursion is the general case of the call model. Each recursive invocation
creates an **independent activation** with:
- Its own register namespace (values array).
- Its own `current_block` and `instr_idx`.
- Its own `prev_block` for phi resolution.

The caller's register values are preserved by the activation boundary.
When the recursive call returns, the caller resumes with its register
state intact.

```
factorial(3):
  Frame for factorial(3):  %n = 3
    → calls factorial(2):
        Frame for factorial(2):  %n = 2
        → calls factorial(1):
            Frame for factorial(1):  %n = 1
            → ret 1
        → Frame for factorial(1) destroyed
        → caller continues: %n = 2, %rec = 1, %mul = 2
        → ret 2
    → Frame for factorial(2) destroyed
    → caller continues: %n = 3, %rec = 2, %mul = 6
    → ret 6
```

The architecture does not impose a recursion limit. Implementations may
define `max_activation_depth` based on resource constraints.

---

## 5. Stack Frame Model

### 5.1 Frame Contents

Each activation has an associated **stack frame** — a contiguous range of
the address space allocated by `alloca` instructions within that activation.

```
┌────────────────────────────┐
│   Caller's frame           │  ← higher addresses
│                            │
├────────────────────────────┤
│   Callee's frame           │
│   ┌──────────────────────┐ │
│   │  Local allocations   │ │  ← allocated by alloca
│   │  (variables, arrays) │ │
│   └──────────────────────┘ │
│                            │
│   Saved registers          │  ← caller-save convention: values
│   (runtime-dependent)      │     live across calls are preserved here
│                            │
└────────────────────────────┘  ← !sp (stack pointer)
         ↓ grows downward
```

### 5.2 Stack Pointer

The stack pointer (`!sp`) is a `Wptr`-bit unsigned integer that tracks the
boundary between allocated and free stack space.

Initial value: `memory_size` (one past the last valid address).
Grows downward toward `stack_limit`.

On `alloca`:
```
!sp = !sp - sizeof(T) * count
return !sp   (base address of allocation)
```

If `!sp` would fall below the profile-defined `stack_limit`, behavior is
undefined (stack overflow).

### 5.3 Frame Lifetime

1. **Entry**: The callee receives `!sp` at the caller's frame boundary.
2. **Allocation**: `alloca` decrements `!sp` and returns the new value.
3. **Return**: The callee restores `!sp` to its entry value (or the
   runtime re-establishes the caller's `!sp` from saved state).

The architecture mandates that the caller's frame is preserved across the
call. On a per-activation-register-namespace runtime, no explicit save/
restore is needed. On a flat-storage runtime, the caller must save register
values live across the call to its stack frame before `call` and reload
after `ret`.

### 5.4 Frame Deallocation

On `ret`, the callee's frame is deallocated by restoring the stack pointer
to the value it had at callee entry. The memory contents of the callee's
frame become undefined — they may be overwritten by the next `alloca` in
the caller or in a subsequent callee.

### 5.5 VM Realization: Local Slots

The ISA describes SSA virtual registers at the architecture level
([`ISA.md`](./ISA.md) §2.5). A stack-based VM implementation realizes those
registers with **local slots**: a per-activation vector indexed by non-negative
integers.

```text
Frame:
  ┌──────────────────────┐
  │ locals[0]            │  ← parameter cells start here
  │ ...                  │
  │ locals[param_cells]  │  ← first non-parameter SSA value
  │ ...                  │
  │ locals[local_count-1]│  ← includes temp slot for phi cycles
  └──────────────────────┘
```

- `local.get <slot>` pushes the value in `locals[slot]` onto the operand stack.
- `local.set <slot>` pops a value from the operand stack into `locals[slot]`.
- Slot lifetimes are tied to the activation: every slot is allocated when the
  frame is created and deallocated when the frame is destroyed.
- The set of slots and their cell counts are determined by the target profile
  during SAIR→ISA lowering; the architecture itself does not fix a slot count.

Local slots are a **VM realization detail**, not an architecture-level concept.
They make multi-block control flow and phi elimination practical because a
value can be stored in a slot at the end of one block and loaded from the same
slot at the start of another, regardless of the operand-stack shape at the
block boundary.

### 5.6 Byte-memory and trap primitives (`Load8`, `Store8`, `Trap`)

The VM's word `Load`/`Store` transfer a whole 32-bit cell (4 bytes) at an address.
The three primitives below give the realized machine byte-granular memory and a
program-declared terminal stop. They are additive (§A.6 of `ISA.md`): the word ops
are unchanged. `byte` below means the low 8 bits of a 32-bit cell value.

#### `Load8` — byte load, zero-extended

Stack: `( addr → byte )`.

1. Pop `addr`.
2. Read the single byte at `⟦addr⟧` (`memory.read_byte`).
3. Push the byte **zero-extended** to a 32-bit cell (`0x00 ..= 0xFF`).

`Load8` reads exactly one byte and never sign-extends. Addressing and bounds follow
the memory model exactly as word `Load` (address `0` and out-of-range are UB, §6).

#### `Store8` — byte store

Stack: `( addr byte → )`.

1. Pop the byte value; take `byte = value & 0xFF`.
2. Pop `addr`.
3. Write exactly one byte: `memory.write_byte(⟦addr⟧, byte)`.

`Store8` leaves `⟦addr⟧ ± 1` untouched — the property word `Store` lacks. Addressing
and bounds follow the memory model exactly as word `Store`.

#### Composition (how typed memory is realized)

A typed load/store of a value narrower than a cell is composed from `Load8`/`Store8`
in little-endian byte order by the lowering layer:

| Type `T` | `load T` (via `Load8`) | `store T` (via `Store8`) |
|---|---|---|
| `i8` / `i1` | one `Load8` (mask bit 0 for `i1`) | one `Store8` |
| `i16` | two `Load8`, combine `lo | hi<<8` | two `Store8`, low byte first |
| `i32` | four `Load8`, assemble LE | four `Store8` |

`i64` (two cells, 8 bytes) is assembled/disassembled as two `i32`-shaped halves over
the same byte primitives. Nothing in the primitives is endian-sensitive; only their
composition is.

#### `Trap` — terminal, program-declared stop

Stack: `( → )` — the operand stack is left unchanged.

Executing `Trap` **terminates the program in a well-defined trap state** (`VmError::Trap`).
The contract:

- The program did **not** complete: there is no return value, and control does not
  resume at any caller.
- The state is distinguishable from every *machine-error* class (stack underflow, type
  mismatch, division by zero, invalid address, call-stack exhaustion, undefined
  function/label). Those report a defect in the *program's use of the machine*; a trap
  is a stop the *program itself declared*.
- It is **not** a host panic/unwind and **not** an arbitrary continuation.
- The trap state records the location (function / block / program counter) that executed
  it, so the host can report where the program's assumptions were violated.

`Trap` is the VM realization of the `unreachable` terminator (§3.4).

### 5.7 Bitwise and shift semantics (`and` / `or` / `xor` / `shl` / `lshr` / `ashr`)

SAIR bitwise and shift instructions are width-preserving integer ops. This section
fixes their realized semantics so the interpreter and the VM agree bit-for-bit.
All arithmetic is on the stored two's-complement bits; nothing here is
Scratch/`f64`-shaped.

**Width carrier.** As with the arithmetic ops (§2.1), a sub-32-bit value is carried
in a 32-bit word whose low `w` bits are the value; results are masked to `w` on
write. A 64-bit value is carried as two 32-bit limbs (low limb at the lower
slot / byte offset). `w = 1|8|16|32` occupy one limb; `w = 64` occupies two.

**`and` / `or` / `xor`.** Applied per limb over the width bit pattern: single-limb
values use the 32-bit word op on the masked cell; `i64` applies the word op to each
limb independently. There is no cross-limb interaction.

**Shift amount and the poison region.** LLVM treats a shift amount `≥ w` as poison.
This stack chooses a deterministic definition so both engines can match exactly:
the **effective amount is `amount mod w`** — `amount & (w - 1)` for single-limb
values, `amount & 63` for `i64`. Amounts inside `[0, w)` therefore shift exactly;
amounts at or above `w` are folded by the same mask instead of being UB or
host-dependent.

- `shl` zero-fills the vacated low bits; bits shifted out of the `w`-bit pattern are
  discarded (the result is remasked to `w`).
- `lshr` zero-fills the vacated high bits.
- `ashr` replicates the sign bit (bit `w - 1` of the `w`-bit pattern) into the vacated
  high bits.

**VM realization.** The word ISA only exposes a 32-bit logical `Shr` (no shift of a
masked sub-32 sign, no `ashr`, no 64-bit crossing). The lowerer therefore realises
every shift by a small program-level software helper (`__sair_shl64` /
`__sair_lshr64` / `__sair_ashr64`) that steps one bit per iteration over a
`(lo, hi)` pair:

- A single-limb value is widened into the pair: `shl`/`lshr` zero-extend (`hi = 0`),
  while `ashr` **sign-extends across both limbs** — bits `w..31` of the low limb as
  well as `hi` become `0` or `0xFFFF_FFFF` from bit `w - 1`. Sign-extending only
  `hi` while leaving bits `w..31` of the low limb zero would not be the true
  `w`-bit pattern and would shift the wrong negative value.
- An `i64` value is passed as its two limbs directly.
- After the helper returns, a single-limb result is remasked to `w` (an `i1` is
  reduced to a flag), so the cell stays canonical (§2.1).

The interpreter computes the same semantics directly on masked `w`-bit values
(`scratcharch-sair-interpreter` `int_bitop`); the two engines are differentially
tested over the boundary and poison-region cases (`vm_differential_tests.rs`). The
interpreter/VM agreement is what makes these ops part of the executable
LLVM→SAIR→ISA loop rather than an interpreter-only feature.

---

## 6. Memory Model

### 6.1 Global Memory

Global variables occupy a contiguous region starting at address 1, one per
declaration in module order. Each global occupies `sizeof(T)` bytes. The
global region is initialized before the entry function executes.

### 6.2 Heap

The heap region (`[G, S)`) exists in the address space layout but is **not
managed by the architecture**. Heap allocation is a runtime library concern.
The architecture provides no heap operations.

### 6.3 Stack

The stack region (`[S, M)`) grows downward from memory size toward the
stack base. Stack allocation is performed exclusively by `alloca`, which
decrements `!sp` and returns the new base address.

```
┌──────────────────────┐  M (memory_size)
│  Free stack space    │
│                      │
├──────────────────────┤  ← !sp (moves downward on alloca)
│  Allocated frames    │
│                      │
├──────────────────────┤  S (stack base / limit)
│  Heap region         │
├──────────────────────┤  G (heap base)
│  Global variables    │
├──────────────────────┤  1
│  Null (no access)    │
└──────────────────────┘  0
```

### 6.4 Pointer Behavior

- Pointers are `Wptr`-bit unsigned integers.
- Address arithmetic wraps modulo `2^Wptr`.
- Address `0` is the null pointer. Load/store from/to address `0` is
  undefined behavior.
- Function pointers are virtual addresses in the reserved region `[M, M+F)`.
  They cannot be loaded from or stored to memory.

### 6.5 Load/Store Semantics

#### `load T, ptr %addr`

```
bytes[i] = memory[addr + i]   for i = 0, 1, ..., sizeof(T)-1
result   = decode_little_endian(T, bytes)
```

The loaded value is constructed from `sizeof(T)` consecutive bytes in
little-endian order. For types wider than a cell (`sizeof(T) * 8 > W`),
the value decomposes into multiple cells in register representation
(see ABI.md §2).

#### `store T %val, ptr %addr`

```
bytes = encode_little_endian(T, %val)
memory[addr + i] = bytes[i]    for i = 0, 1, ..., sizeof(T)-1
```

The source value's register representation (possibly multi-cell) is
serialized to `sizeof(T)` little-endian bytes and written to consecutive
addresses.

#### Alignment

If `addr` is not a multiple of `alignof(T)`, behavior is undefined unless
the instruction is annotated with `align 1` (packed access). This matches
LLVM's alignment semantics.

### 6.6 Uninitialized Memory

Memory allocated by `alloca` has **undefined initial contents**. Reading
from uninitialized memory without a prior `store` is undefined behavior.

---

## 7. Instruction Dispatch

### 7.1 SAIR Interpreter Dispatch

The SAIR interpreter uses an **instr_id_map** that maps each
`(block_label, instruction_index)` pair to the global `ValueId` of the
value produced by that instruction. This map is built before execution and
enables `O(1)` write-back of instruction results.

```
InstrIdMap = Vec<(block_label, instr_idx, value_id)>

On instruction execute:
  compute result
  lookup (current_block, instr_idx) → value_id
  frames.top().values[value_id] = Some(result)
  instr_idx += 1
```

### 7.2 VM Dispatch

The ISA VM uses a flat `Vec<Code>` per function with a program counter.
Each instruction is a `Code` enum variant. The VM dispatches on the opcode
and manipulates an operand stack. Stack-based instructions consume operands
from and push results onto this stack; `local.get` and `local.set` transfer
values between the operand stack and the frame-local slots described in
§5.5, allowing SSA values to survive across block boundaries.

### 7.3 Dispatch Equivalence

Both dispatch models produce the same observable state transitions:

- SAIR instructions → SAIR interpreter: SSA-style, values stored by ValueId.
- ISA instructions → VM: stack-based, values stored on operand stack.
- Lowering (SAIR → ISA) is correct if every SAIR instruction sequence
  produces the same memory state and return value as the interpreter.

---

## 8. Undefined Behavior

The following operations have undefined behavior in the execution model:

| Operation | Condition |
|---|---|
| Division | Divisor is 0 (`udiv`, `sdiv`, `urem`, `srem`) |
| Signed division overflow | `sdiv iN INT_MIN, -1` |
| Null pointer access | `load`/`store` to address 0 |
| Unaligned access | `load`/`store` to misaligned address without `align 1` |
| Uninitialized read | `load` from memory not previously stored to |
| Stack overflow | `alloca` pushes `!sp` below the stack limit |
| Activation overflow | `call` exceeds `max_activation_depth` |
| `unreachable` | Terminator is reached |

An implementation may trap, produce a distinguished result, or behave
arbitrarily on undefined behavior. The architecture does not prescribe
the behavior.

> **Realized behavior of `unreachable`.** Although the row above is UB at the
> architecture level, neither reference implementation leaves it arbitrary: the
> SAIR interpreter and the ISA VM both realize it as a **trap** (§3.4, §5.6).
> A host that reaches `unreachable` therefore observes a well-defined terminal
> state, never a silent wrong value.

---

## 9. Frozen Decisions (v0.1)

1. All integer arithmetic wraps modulo 2^N; there are no trap-on-overflow
   semantics (§2.3).
2. Signedness is an operation property, not a value property (§2.1, §2.2).
3. Floating-point follows IEEE-754 for the declared type (§2.5).
4. Execution is sequential and in program order (§3.2).
5. Basic blocks have no fall-through; every block ends in a terminator (§3.3).
6. `Call` owns return-value placement; `Return` must not write to caller
   state (§4.1, §4.2).
7. Each recursive call creates an independent activation with its own
   register namespace (§4.3).
8. `alloca` decrements `!sp` and returns the new `!sp` as the base address
   (§5.2).
9. Uninitialized memory has undefined contents (§6.6).
10. The execution models of SAIR interpreter and ISA VM are observationally
    equivalent under correct lowering (§7.3).
11. Local slots are a VM realization detail for stable cross-block SSA value
    storage; they do not alter the architecture-level SSA model (§5.5).
12. `Load8`/`Store8`/`Trap` are the realized stack machine's byte-memory and
    terminal primitives (§5.6, `ISA.md` Appendix A): `Load8` zero-extends a
    byte, `Store8` writes the low byte only, `Trap` terminates in the
    distinguished `VmError::Trap` state. They are additive — the word
    `Load`/`Store` are unchanged — and target-independent.
