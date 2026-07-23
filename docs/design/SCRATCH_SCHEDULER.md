# ScratchArch Scratch Scheduler Design

> Design version: **v0.3**
> Status: conceptual; no implementation in this milestone.

This document describes how a Scratch scheduler would fit into the ScratchArch
stack. The scheduler is the boundary between the compiler's static control flow
and Scratch's event-driven, cooperative, multi-script execution model.

## Why a scheduler matters

Scratch programs are not single-entry procedures. A project contains multiple
scripts, each with its own event hat, and the Scratch VM runs them concurrently
(within a single-threaded cooperative loop). The compiler currently lowers SAIR
into a single green-flag entry script plus helper procedures, but future
frontends will want to compile event-driven programs with multiple entry points.

A scheduler design ensures that:

- Event hats map to script lifecycle states.
- Concurrent scripts do not corrupt each other's runtime state.
- Yielding and waiting have defined semantics.
- The runtime stack model from [`SCRATCH_ABI.md`](../specification/SCRATCH_ABI.md)
  composes correctly with multi-script execution.

## Script lifecycle

Each `Script` in ScratchGraph follows a simple state machine:

```text
  Idle
   |
   | event triggered
   v
 Running ----> Waiting (yield/wait block)
   ^                |
   | resume         | condition satisfied
   +----------------+
   |
   | stop / script ends
   v
 Stopped
```

| State | Meaning |
| ----- | ------- |
| `Idle` | The script exists but is not executing. It is attached to an event hat. |
| `Running` | The script is actively executing blocks. |
| `Waiting` | The script has yielded and will resume later. |
| `Stopped` | The script has terminated. It can be triggered again from `Idle`. |

A script transitions from `Idle` to `Running` when its event hat fires. It
returns to `Idle` or `Stopped` when it reaches a `Stop` statement or the end of
its body.

## Event dispatch

ScratchGraph already models event hats:

| Hat | Trigger |
| --- | ------- |
| `GreenFlag` | Green flag clicked |
| `KeyPressed(key)` | Specific key pressed |
| `SpriteClicked` | Sprite or stage clicked |
| `BroadcastReceived(name)` | Named broadcast received |
| `CloneStart` | Clone created |

A future scheduler would maintain a dispatch table from hat to script entry
point. When an event occurs, the scheduler creates or resumes a thread for that
script.

### Dispatch rules

- Multiple scripts can respond to the same event; each gets its own thread.
- A `BroadcastReceived` hat runs once per broadcast event per matching script.
- `GreenFlag` stops all other scripts and restarts the green-flag scripts (this
  is Scratch VM behavior; the scheduler would coordinate it).

## Scheduling model

### Cooperative single-threaded loop

The Scratch VM runs one block per thread per frame, switching between threads.
A scheduler for ScratchGraph would follow the same model:

1. Maintain a queue of runnable threads.
2. Pick the next thread.
3. Execute a bounded amount of work (e.g., one Scratch block or one small
   control-flow region).
4. If the thread yields or waits, move it to a waiting set.
5. Repeat.

This model is cooperative: scripts must explicitly yield. Long-running loops in
generated code must insert yield points to avoid freezing the VM.

### Thread identity

Each thread needs its own:

- Program counter (current block / script position).
- Stack pointer / frame pointer.
- Local frame slots.

In v0.3, the runtime uses a single global `__scratcharch_stack` and
`__scratcharch_fp`. A future scheduler would either:

- Allocate a separate stack list per thread, or
- Tag each frame with a thread id and manage a single shared stack carefully.

The compiler can keep the same `EnterFrame` / `PopFrame` primitives; only the
backing storage changes.

## Cooperative yielding

A yielded thread records its resume point and gives control back to the
scheduler. Common yield reasons:

| Scratch block | Scheduler action |
| ------------- | ---------------- |
| `wait N seconds` | Move thread to waiting set; resume after timer expires. |
| `wait until <cond>` | Move thread to waiting set; check condition each frame. |
| `broadcast ... and wait` | Send broadcast; yield until all responders finish. |
| Long-running loop | Insert periodic yield statements during lowering. |

The generated code can express yields as a scheduler primitive, for example a
`YieldAndResume { resume_label }` statement that the exporter maps to a
broadcast/wait pair or a custom scheduler block.

## Relationship to `__scratcharch_stack`

The stack list belongs to the runtime ABI, not the scheduler. The scheduler:

- Does not interpret frame offsets.
- Does not create or destroy frames.
- Ensures that when a script resumes, `__scratcharch_fp` still points to that
  script's current frame.

If each thread has its own stack list, the scheduler switches `__scratcharch_stack`
(and `__scratcharch_fp`) on context switch. If threads share a stack list, the
scheduler must guarantee that scripts do not interleave in the middle of a call
sequence, because `__scratcharch_fp` is global.

## Future implementation path

1. **Per-thread stacks.** Replace the single `__scratcharch_stack` with a stack
   list per script thread. `__scratcharch_fp` stays global but always refers to
   the currently running thread's stack.
2. **Yield primitive.** Add a `Yield` statement to ScratchGraph and a
   corresponding exporter mapping.
3. **Event dispatch table.** Lower multiple event hats to scripts and register
   them with a small scheduler runtime.
4. **Green-flag reset.** On green flag, reset all thread states and stacks.

## Limitations of v0.3

- No scheduler is implemented.
- `__scratcharch_stack` is global and shared across all scripts.
- Generated code does not yield inside long-running loops.
- Only a single green-flag entry script is produced by the SAIR lowerer.
