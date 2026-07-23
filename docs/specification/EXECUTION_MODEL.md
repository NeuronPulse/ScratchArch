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

Must never be executed. If reached, behavior is undefined.

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
and manipulates an operand stack.

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
