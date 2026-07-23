# ScratchArch Scratch Runtime Model

This document describes the runtime conventions used when ScratchArch lowers
SAIR to Scratch via the `scratcharch-scratchgraph` backend. These conventions
are intentionally confined to the Scratch backend; SAIR itself remains
language-independent and has no notion of Scratch-specific execution semantics.

## Custom block return values

Scratch 3 custom blocks (procedures) do not return values. A custom block can
report its result only by writing to a variable that the caller can read after
the block finishes.

ScratchGraph v0.3 models SAIR functions that return a value with a **per-call
frame slot** in the runtime call stack:

```text
__scratcharch_stack[fp + 1]
```

Every activation has its own frame in the hidden stage list
`__scratcharch_stack`, indexed by the frame pointer `__scratcharch_fp`.

### Caller/callee contract

- The caller pushes a frame for the callee with `EnterFrame { slots }`.
- The callee writes its return value to frame offset 1 before popping its locals
  and stopping.
- The caller copies frame offset 1 into its own result SSA slot, restores the
  frame pointer from the saved FP at offset 0, and pops the callee's frame.

Example lowering:

```text
SAIR:
  %r = call @add(%a, %b)

ScratchGraph caller:
  EnterFrame { slots: add_frame_size }
  call add(a, b)
  FrameSet { offset: offset(%r), value: FrameGet { offset: 1 } }
  set __scratcharch_fp = FrameGet { offset: 0 }
  PopFrame { slots: add_frame_size }

ScratchGraph callee `add`:
  FrameSet { offset: 1, value: a + b }
  PopFrame { slots: local_count }
  stop this script
```

This convention is implemented entirely inside
`crates/scratcharch-scratchgraph/src/lower.rs` and does not affect SAIR, the
interpreter, or the VM backend. See
[`docs/specification/SCRATCH_ABI.md`](../specification/SCRATCH_ABI.md) for the
full frame layout.

### Re-entrancy

The frame-based convention is **re-entrant**. Each recursive or nested call
gets its own saved FP, return slot, and local slots, so later activations do
not overwrite earlier ones. Concurrent calls from multiple scripts still share
the single global stack list in v0.3; a future scheduler would provide
per-thread stacks (see [`SCRATCH_SCHEDULER.md`](./SCRATCH_SCHEDULER.md)).

## Event-driven execution

Scratch programs are event-driven, not single-entry. A Scratch project contains
a stage and zero or more sprites; each target may contain multiple independent
scripts, each triggered by its own event hat.

ScratchGraph models this directly:

```text
Project
└── Stage
|   ├── Script { hat: GreenFlag, ... }
|   └── Script { hat: BroadcastReceived("go"), ... }
└── Sprite
    ├── Script { hat: KeyPressed("space"), ... }
    ├── Script { hat: SpriteClicked, ... }
    └── Script { hat: CloneStart, ... }
```

### Event hats

| ScratchGraph `Hat`          | Scratch trigger                         |
| --------------------------- | --------------------------------------- |
| `GreenFlag`                 | When the green flag is clicked          |
| `KeyPressed(key)`           | When the specified key is pressed       |
| `SpriteClicked`             | When this sprite is clicked             |
| `BroadcastReceived(name)`   | When the named broadcast is received    |
| `CloneStart`                | When a clone of this sprite starts      |
| `Procedure { name }`        | Custom block definition (not an event)  |

### Entry point convention

SAIR modules have a single named entry function. The ScratchGraph lowerer
creates one green-flag script on the stage that calls the entry procedure. This
is a pragmatic mapping: Scratch has no concept of command-line arguments, so the
green-flag script passes zero literals for any entry-function parameters.

Future frontends that target event-driven programs can build `Project` values
directly and attach additional event hats to sprites or the stage.

## Concurrent scripts

Because each script has its own hat, multiple scripts can run at the same time.
ScratchGraph preserves this concurrency model:

- Each `Script` is an independent stack of statements.
- Scripts share variables and lists according to their scope (`Global`,
  `SpriteLocal`, or `Temporary`).
- Stopping one script (`Stop { ThisScript }`) stops only that script.

The return-value convention is the main place where concurrency interacts with
the compiler: concurrent calls to the same function race on the shared
`__ret_<name>` variable. Programmers and future lowering passes must be aware
of this when emitting calls from multiple scripts.

## Scope of runtime conventions

All conventions described here live in `scratcharch-scratchgraph` and its
exporters. No crate below it (`scratcharch-core`, `scratcharch-ir`,
`scratcharch-vm`, `scratcharch-llvm`) knows about event hats, hidden return
variables, or Scratch concurrency. This keeps the architecture layered and
allows the same SAIR module to be executed by the SAIR interpreter, lowered to
the core ISA VM, or exported to Scratch without semantic coupling.
