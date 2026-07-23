# ScratchArch Scratch Runtime ABI

> Specification version: **v0.3**
> Status: Scratch target only; does not affect the architecture-level ABI in
> [`ABI.md`](./ABI.md) or the SAIR calling convention.

This document defines the calling convention used when ScratchArch lowers SAIR
programs to Scratch via `scratcharch-scratchgraph`. It exists because Scratch has
no native procedure return values, no call stack, and no reentrant custom blocks.
The ABI therefore simulates an activation stack using Scratch's list and variable
primitives.

## Runtime storage

Two hidden stage-owned values form the runtime:

| Name | Kind | Purpose |
| ---- | ---- | ------- |
| `__scratcharch_stack` | List | Call stack; each activation is a contiguous region of cells |
| `__scratcharch_fp` | Variable | Frame pointer; 0-based index of the current frame's base cell |

Both are declared on the stage so that every sprite and the stage can access
them. They are compiler-generated temporaries and are not part of the source
program's observable state.

## Frame layout

For a function with `P` parameters and `L` non-parameter SSA values:

```text
frame_size = 2 + L
```

Each activation frame occupies `frame_size` consecutive cells in
`__scratcharch_stack`:

| Offset | Name | Purpose |
| ------ | ---- | ------- |
| 0 | saved_fp | The caller's `__scratcharch_fp` value |
| 1 | return_slot | Return value (reserved even for `void` functions) |
| 2 .. frame_size-1 | locals | Non-parameter SSA values, one slot per value |

Parameters are **not** stored in the frame. They are passed through Scratch
custom-block inputs and read with `ProcedureParam` expressions.

### SSA value to frame offset

A non-parameter SSA value `v` (where `v >= P`) maps to frame offset:

```text
offset(v) = 2 + (v - P)
```

Constants do not occupy frame slots; they are emitted as literal expressions.

## Frame lifecycle

### Entering a frame (`EnterFrame { slots }`)

```scratchblocks
add (__scratcharch_fp) to [__scratcharch_stack v]
repeat ((slots) - (1))
    add (0) to [__scratcharch_stack v]
end
set [__scratcharch_fp v] to ((length of [__scratcharch_stack v]) - (slots))
```

The first pushed cell becomes the saved caller FP at offset 0. The remaining
cells are zero-initialized. The return slot therefore starts as `0`, which is a
safe default for integer-like values.

### Leaving a frame as callee (`PopFrame { local_count }`)

On `ret`, the callee removes only its local slots, leaving the saved FP and the
return slot intact for the caller:

```scratchblocks
repeat (local_count)
    delete item (length of [__scratcharch_stack v]) of [__scratcharch_stack v]
end
```

where `local_count = frame_size - 2`.

### Returning a value

A non-void callee writes its return value to the per-frame return slot before
popping locals:

```scratchblocks
replace item ((__scratcharch_fp) + (2)) of [__scratcharch_stack v] with (return_value)
```

(The list index is `fp + 1 + 1` because Scratch lists are 1-indexed.)

### Reading a return value as caller

After the callee stops, the caller copies the return slot into its own result
frame slot, then restores the frame pointer and pops the callee's frame:

```scratchblocks
replace item ((__scratcharch_fp) + (result_offset + 1)) of [__scratcharch_stack v] with (item ((__scratcharch_fp) + (2)) of [__scratcharch_stack v])
set [__scratcharch_fp v] to (item ((__scratcharch_fp) + (1)) of [__scratcharch_stack v])
repeat (callee_frame_size)
    delete item (length of [__scratcharch_stack v]) of [__scratcharch_stack v]
end
```

## Calling convention

Caller responsibilities:

1. Push a frame for the callee with `EnterFrame { slots: callee_frame_size }`.
2. Call the Scratch custom block, passing arguments as custom-block inputs.
3. If the call is non-void, copy the return slot into the caller's result frame
   slot.
4. Restore `__scratcharch_fp` from the saved FP at offset 0.
5. Pop the callee's frame with `PopFrame { slots: callee_frame_size }`.

Callee responsibilities:

1. Read parameters from custom-block inputs (`ProcedureParam`).
2. Compute results into the caller-provided frame slots.
3. If non-void, write the return value to frame offset 1 before returning.
4. Pop local slots only (`PopFrame { local_count }`).
5. Stop this script.

## Entry point

The SAIR module's entry function is invoked by a generated green-flag script.
Because Scratch green-flag scripts cannot take arguments, entry parameters are
passed as zero literals. The entry script initializes the stack and frame pointer
before the call:

```scratchblocks
delete all of [__scratcharch_stack v]
set [__scratcharch_fp v] to (0)
EnterFrame { slots: entry_frame_size }
call entry(...)
set [__scratcharch_fp v] to (item ((__scratcharch_fp) + (1)) of [__scratcharch_stack v])
PopFrame { slots: entry_frame_size }
stop [this script v]
```

## Recursion example

For `factorial(n)`:

```text
factorial(3)
  frame [fp=0]: saved_fp=0, return_slot=?, n=3
    factorial(2)
      frame [fp=3]: saved_fp=0, return_slot=?, n=2
        factorial(1)
          frame [fp=6]: saved_fp=3, return_slot=?, n=1
            factorial(0)
              frame [fp=9]: saved_fp=6, return_slot=1, n=0
```

Each recursive invocation has its own `n` slot and return slot, so later writes
do not overwrite earlier ones. This makes recursion and nested calls correct.

## Relation to `docs/specification/ABI.md`

The architecture-level ABI in [`ABI.md`](./ABI.md) defines the abstract calling
convention: per-activation register namespaces, caller-save semantics, positional
argument transmission, and return-value output slots. The Scratch Runtime ABI is
one concrete realization of that abstraction for the Scratch target:

| Architecture ABI concept | Scratch realization |
| ------------------------ | ------------------- |
| Activation | Frame in `__scratcharch_stack` |
| Virtual register namespace | Frame-local SSA slots |
| Return-value output slot | Frame offset 1 |
| Caller-save / restore | Frames make saves implicit per activation |
| Argument transmission | Scratch custom-block inputs |

## Limitations

- **Loop-carried phi values.** The lowering copies phi operands on forward
  control-flow edges only. Back edges (loops) are not yet handled.
- **Single shared stack.** All Scratch scripts share `__scratcharch_stack` and
  `__scratcharch_fp`. Concurrent calls from multiple scripts can interleave
  unpredictably until a per-thread stack model is implemented.
- **Stack depth.** Frames are grown and shrunk one list cell at a time, which is
  slow for deep recursion and limited by Scratch's list size.
- **Single-cell values.** Multi-cell integers, aggregate arguments, alignment,
  and padding are out of scope for v0.3.
- **Indirect calls.** Calls to functions not defined in the SAIR module (runtime
  intrinsics, function pointers) are not lowered.
