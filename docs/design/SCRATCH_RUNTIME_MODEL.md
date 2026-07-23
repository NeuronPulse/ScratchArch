# ScratchArch Scratch Runtime Model

This document describes the runtime conventions used when ScratchArch lowers
SAIR to Scratch via the `scratcharch-scratchgraph` backend. These conventions
are intentionally confined to the Scratch backend; SAIR itself remains
language-independent and has no notion of Scratch-specific execution semantics.

## Custom block return values

Scratch 3 custom blocks (procedures) do not return values. A custom block can
report its result only by writing to a variable that the caller can read after
the block finishes.

ScratchGraph models SAIR functions that return a value with a **hidden stage
variable** named after the callee:

```text
__ret_<function_name>
```

### Caller/callee contract

- The callee writes its return value to `__ret_<function_name>` immediately
  before it stops.
- The caller invokes the custom block and then copies `__ret_<function_name>`
  into its own SSA result variable.

Example lowering:

```text
SAIR:
  %r = call @add(%a, %b)

ScratchGraph caller:
  call add(a, b)
  set %r = __ret_add

ScratchGraph callee `add`:
  set __ret_add = a + b
  stop this script
```

This convention is implemented entirely inside
`crates/scratcharch-scratchgraph/src/lower.rs` and does not affect SAIR, the
interpreter, or the VM backend.

### Limitations

The hidden-variable convention is **not re-entrant**. If a function calls
itself recursively, or if multiple scripts call the same function concurrently,
they all share the same `__ret_<name>` slot. Later writes overwrite earlier
ones before the caller can copy them. Re-entrant return values are left to
future work (for example, per-call stack slots or a call frame list).

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
