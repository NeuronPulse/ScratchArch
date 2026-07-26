# ScratchArch Scratch Runtime Implementation

> Design version: **v0.4**
> Status: IR-level abstraction; not a full Scratch VM interpreter.

This document describes the runtime abstraction added to ScratchGraph in the
Scratch Backend Foundation v0.4 milestone. The goal is to model Scratch VM
execution semantics at the IR level so that future milestones can implement a
real scheduler, per-thread stacks, and event dispatch without redesigning the
underlying data structures.

## Runtime model overview

```text
RuntimeState
├── SchedulerState
│   ├── threads: Vec<ThreadState>
│   ├── runnable: Vec<ThreadId>
│   ├── waiting: Vec<ThreadId>
│   └── current: Option<ThreadId>
├── EventState
│   ├── green_flag: Vec<ThreadId>
│   ├── key_pressed: HashMap<String, Vec<ThreadId>>
│   ├── sprite_clicked: Vec<ThreadId>
│   ├── broadcast_received: HashMap<String, Vec<ThreadId>>
│   └── clone_start: Vec<ThreadId>
└── GlobalState
    ├── variables: HashMap<String, Value>
    ├── lists: HashMap<String, Vec<f64>>
    └── broadcasts: Vec<String>
```

Every component is a plain Rust value. No actual Scratch execution happens in
v0.4; the structures capture what a scheduler would need to know.

## Thread context and frame ABI

v0.3 stored the call stack in a single global list `__scratcharch_stack` and a
global variable `__scratcharch_fp`. This is correct for a single green-flag
script but breaks when multiple scripts run concurrently.

v0.4 introduces `ThreadContext`:

```rust
pub struct ThreadContext {
    pub stack: Vec<f64>,
    pub frame_pointer: usize,
}
```

Each `ThreadState` owns its own `ThreadContext`. The generated Scratch project
still emits `__scratcharch_stack` and `__scratcharch_fp` at runtime; a future
scheduler will swap these values (or use per-sprite lists) on context switch.
The compiler itself does not need to change frame layout or generated code.

### Frame operations

`ThreadContext` provides the same operations that ScratchGraph statements
express:

- `enter_frame(slots)`: push saved FP + zero-initialized local slots.
- `pop_frame(slots)`: remove trailing stack cells.
- `frame_get(offset)`: read `stack[frame_pointer + offset]`.
- `frame_set(offset, value)`: write `stack[frame_pointer + offset]`.

These are used by tests and future interpreter code to verify that the frame
layout matches the generated Scratch blocks.

## Thread lifecycle

```text
Idle -> Running -> Stopped
         |
         v
      Waiting(Yield/Timer/Until/Broadcast)
         |
         v
      Running
```

A thread is created in `Idle` by `RuntimeState::register_project`. An event
dispatch moves matching threads to `Runnable`. The scheduler picks one thread,
sets it to `Running`, and executes a bounded amount of work. Yielding or waiting
moves the thread to `Waiting`.

## Event dispatch

`EventState` is a dispatch table from event hats to thread IDs. When a green
flag is clicked, `RuntimeState::dispatch_green_flag`:

1. Resets all threads to `Idle` and clears their stacks.
2. Clears the runnable and waiting queues.
3. Enqueues every thread registered under `EventHat::GreenFlag`.

Other events (`KeyPressed`, `BroadcastReceived`, etc.) use
`RuntimeState::dispatch_event`, which enqueues only the matching threads without
resetting the whole runtime.

## Relationship to generated Scratch code

The runtime abstraction does not change the exporter. `JsonExporter` still emits:

- `__scratcharch_stack` as a stage list.
- `__scratcharch_fp` as a stage variable.
- `EnterFrame` / `PopFrame` / `FrameSet` / `FrameGet` as list operations.

A future scheduler runtime would be implemented as additional Scratch scripts or
an external VM that manipulates these globals between context switches.

## Future work

- Implement a real Scratch VM interpreter using `RuntimeState`.
- Add `Yield` and `Wait` statements to ScratchGraph.
- Per-thread stacks instead of a single shared list.
- Green-flag reset semantics that stop all other threads.
- Broadcast-and-wait synchronization.
