# Function-Pointer & Indirect-Call Feasibility Study

> Status: **feasibility study — no implementation yet** (Part 5 of the LLVM
> Compatibility v0.4 "Real-World Coverage Expansion" milestone). Read this
> before writing any indirect-call code. It answers *how* function pointers
> could be represented and dispatched through ScratchArch's frozen SAIR/ISA
> model, and *what* it would cost — not a commitment to the design.
> Companion matrix: [`docs/specification/LLVM_COMPATIBILITY.md`](../specification/LLVM_COMPATIBILITY.md).

## 1. The question

Real `-O0` C programs call functions through pointers all the time: callbacks,
dispatch tables, vtables, `qsort` comparators. Today every one of those is
rejected — some before it is even parsed. The compatibility corpus records the
gap as the `indirect-call` fixture (`indirect-call.c`, clang `-O0`,
`expected_stage: parser`, `expected_status: unsupported`).

This study determines:

1. what forms real C function pointers take at `clang -O0`;
2. how each is stopped by the current pipeline (exact diagnostics);
3. a representation and dispatch design that does **not** alter the frozen ISA
   cell/word model or the data-layout rules;
4. the ABI interaction, recursion interaction, and the required change list.

**Executive conclusion:** feasible within the frozen 32-bit-word ISA, by
encoding a function's address as a small integer *function id* and expanding
each indirect-call site into a per-site dispatch chain (`eq`/branch over the id
that ends in the existing **static** `call` + `Trap` on an unknown id). This is
the same "program-level realisation from primitives, never a silent
approximation" discipline used for `__sair_mul64` and the memory-intrinsic slice.
It needs no widening ISA and no new cell kind. What it *does* need is a
frontend change (accept `@f` as a value and parse `call … %fp(…)`), a new SAIR
indirect-call instruction (or an equivalently explicit lowering contract), and a
per-site dispatch expansion in the ISA lowerer. It is a self-contained slice —
recommended as its own milestone, not a silent addition to this one.

---

## 2. What real `clang -O0` C actually does

A C function pointer is just an address. Clang at `-O0` realises it in a small
number of shapes:

| Shape | IR the parser/translator sees today | Current fate |
|---|---|---|
| Direct call | `call i32 @f(...)` | works (static named callee) |
| Call through a local | `%fp = …; call i32 %fp(...)` | **parser reject** (`indirect call`) |
| Address in a global | `@tbl = global [2 x ptr] [ptr @add, ptr @sub]` | **translator reject** (function name in a value/initializer position) |
| Address as an SSA value | `%fp = ptrtoint ptr @f to i64` … | **translator reject** (`cannot use function '@f' as an SSA value`) |
| Callback passed down | `qsort(a, n, w, @cmp)` → `call i32 %fp(…)` inside qsort | parser reject at the call site |
| Compare/store of fps | `%b = icmp eq ptr %fp, @f` | parser/translator reject (`@f` in operand position) |

Because the callee in `call … %fp(…)` is an SSA *value*, not a global name, the
parser rejects it up front (Part 3 below quotes the diagnostic). Programs that
merely *take* a function address without calling through it — e.g. a table that
is only inspected — are stopped by the translator when the address of `@f` is
required as a value.

## 3. Where the pipeline stops today (exact diagnostics)

All three rejection points are deliberate, named, and never a silent fallback.

### 3.1 Parser: indirect call

`crates/scratcharch-llvm/src/parser.rs` — `call` parses the callee as a
`@global`; a `%local` callee (the indirect form) is rejected:

```text
indirect call through '…' is unsupported: the SAIR/ISA call ABI requires a
statically-named callee (no function-pointer ABI)
```

`indirect-call` is recorded `parser` / `unsupported` with exactly this message.

### 3.2 Translator: a function used as a value

`crates/scratcharch-llvm/src/translator.rs`, `ModuleSyms::global_addr_const`:

```text
cannot use function '@f' as an SSA value: taking a function address requires a
function-pointer ABI (indirect calls are not supported)
```

Data-global addresses are ordinary `i32` constants; a function name is *not* in
the data-global map, so it must be handled explicitly — today that means
rejection rather than a wrong `i32`.

### 3.3 Scratch backend

`ScratchGraphLowerer` lowers every SAIR `Call` to a Scratch custom-block call
(`Stmt::Call { proc: callee, … }`). Scratch 3.0 has **no first-class function
value** and custom blocks cannot be stored, compared, or dispatched at runtime;
a Scratch program cannot express "call whatever `fp` points to". The correct
terminal behavior for the Scratch surface is therefore a named rejection, the
same as the existing "cannot be lowered to Scratch numbers" family. The Scratch
surface is construction-only for LLVM input (§6.2 of the spec), so this is a
backend *model* limit, recorded, not a correctness gap.

## 4. The current architecture (what a design must respect)

### 4.1 SAIR `Call` is statically named

`IrBuilder::call(ret_ty, callee_name, args)` — an `IrModule` `Call` carries the
callee as a `String`. The SAIR interpreter dispatches on that string
(`func_index` for defined functions, otherwise the runtime-intrinsic registry;
`scratcharch-sair-interpreter/src/lib.rs`). There is no "call this value".

### 4.2 ISA `Call` is statically named

ISA lowering emits `IsaInstr::Call(callee_name)` (see the software helpers:
`__sair_mul64` etc. are appended to the ISA program and reached by a plain
named `Call`). The frozen ISA is a set of word ops + branches + `Load8`/`Store8`
+ `Trap`; **there is no computed/indirect call primitive.** Adding one would be
an ISA change and is out of scope under the "no silent ISA modification" rule.

### 4.3 The ABI already anticipated this — and drifted

`docs/specification/ABI.md` §5 still documents a hidden-parameter indirect-call
mechanism:

```text
!fnptr      Indirect call (call %ptr(...))     The function pointer value being
                                               called; the runtime uses it to
                                               select the target
```

alongside a `!return.address` hidden parameter for multi-site returns. The
*implemented* model — SAIR/ISA static-named `Call`, return-address-free — has
superseded that document (the ROADMAP v0.1 entry records the static-call ABI,
and `call_function`/`func_index` dispatch by name). This is a **spec drift**: a
feasibility pass that resurrects `!fnptr` would be an ABI change, not a doc
refresh. The design below therefore chooses the ISA-neutral per-site-dispatch
route and recommends updating ABI.md's §5 to match whichever model is adopted —
never leaving both alive silently.

## 5. Design: function ids + per-site dispatch expansion

### 5.1 Representation — no data-model change

Within one module every reachable function is statically known. Assign each
module function a small, stable **function id** `gid(f)` drawn from a distinct
range that can never collide with a data-global address (e.g. negative or a
high tag bit; the SAIR pointer is an `i32`, so an id is just an `i32` constant).

- Taking `@f` as a value ⇒ `const_i32(gid(f))`. The current translator
  rejection (`global_addr_const`) becomes: *if the name is a function, return its
  id* instead of rejecting.
- A function pointer stored in a global initializer / struct field / passed as
  an argument / compared with `icmp eq` is therefore just an `i32` — no new
  pointer kind, no layout change (pointer-typed leaves already reserve the
  source x86-64 stride; the stored value is the 4-byte SAIR word, exactly the
  id). `bitcast ptr @f to ptr` stays a no-op.
- `null` function pointers stay `0`, which is never a valid id → any call
  through them traps at dispatch (see §5.2). Calling through null is UB in C, so
  trapping is the faithful, non-silent behaviour.

### 5.2 Dispatch — no new ISA primitive

An indirect call `call i32 %fp(a, b)` becomes, at each site, an expansion that
reads `%fp` and dispatches on its id:

```text
   switch (%fp) {
     case gid(f1): result = call i32 @f1(a, b)
     case gid(f2): result = call i32 @f2(a, b)
     default:      trap        // unknown/0 id — faithful, never approximate
   }
```

Because the frozen ISA already has `eq`, conditional branch, and `Trap`, and
because `switch` itself is already expanded to an `eq`/`CondBranch` chain by the
translator, this expansion is expressible as *ordinary SAIR control flow* the
moment indirect calls are parsed. Two placement options:

- **(A) Translator emits a per-site dispatch block** (a synthetic switch over
  the id). Clean, but it multiplies call-site code before lowering and must
  remap phi predecessors exactly as `translate_switch` already does.
- **(B) ISA lowerer expands a dedicated `CallIndirect` SAIR instruction** into
  the dispatch chain at `lower.rs`. One instruction in SAIR (interpreter: direct
  dispatch through its own function table; ISA: chain → named `Call`). Keeps SAIR
  small and puts the ISA-specific fan-out where the ISA lives.

**(B) is the recommendation**: a single `CallIndirect { callee_ty, target, args }`
SAIR instruction is validated and executed directly by the interpreter (which
already has `func_index`, so dispatch is a table lookup — exact and fast), while
the ISA lowerer expands it into the per-site `eq`/branch/`Call`/`Trap` chain.
The interpreter path needs no per-site blocks; the VM path never needs more than
the primitives it already has.

### 5.3 Module-level table

The set of `(gid → function name)` pairs is a static property of the module.
Both engines already enumerate functions (`IrModule::functions`,
`IsaProgram`), so a function-id map is a derived, ordered numbering, not new
state. A call whose runtime id names no function is *not* a silent fallback: the
`default:` arm `Trap`s, giving both engines a named terminal state identical in
spirit to `unreachable`.

### 5.4 Recursion interaction

Recursion is unaffected: ids are assigned at module level before execution and
are independent of the call stack. Direct recursion already works; **indirect
recursion** (a cycle of calls through pointers) works because the dispatch
table is complete and static — a `gid` always resolves. The interpreter's
`max_frames` guard continues to bound runaway recursion; the VM's frame
discipline is unchanged because every actual transfer is still a named `Call`.

### 5.5 Argument/return ABI

`CallIndirect` reuses the existing static-call argument conventions unchanged —
the target function's prototype (its SAIR parameter list) determines cell
placement. The only new requirement is that the *call site* know the target's
prototype: LLVM types indirect calls with the full function type
(`call i32 (i32,i32) %fp …`), so the translator has it. A mismatched id can only
arise from a C-level type error, which is UB; per §5.2 the `default:` arm traps
rather than misreading the frame. The scratcharch-side ABI (§5 of ABI.md) is
otherwise untouched.

## 6. Required change list (nothing done silently)

Adopting this design touches, in order:

1. **Parser** (`parser.rs`): accept `call <ty> (…params…) %fp(args)` and a
   function-typed operand (`@f` used as a value). Remove the §3.1 rejection;
   keep a rejection only for shapes that remain outside scope (e.g. `invoke`,
   musttail).
2. **Translator** (`translator.rs`): map `@f`-in-value-position and
   `@f`-in-initializer-position to `const_i32(gid(f))`; lower indirect calls to
   the new `CallIndirect` instruction. Update `global_addr_const` and
   `write_global_scalar`'s pointer-leaf handling to consult the function table.
3. **SAIR** (`scratcharch-ir`): new `CallIndirect` instruction + validation +
   builder + (already covered) phi/edge rules; per-site-dispatch lowering to ISA
   in `lower.rs`.
4. **Interpreter** (`scratcharch-sair-interpreter`): execute `CallIndirect` via
   `func_index`.
5. **ISA/VM**: no ISA change (dispatch chain uses existing primitives). New
   differential tests assert interpreter-vs-VM agreement for the dispatch chain.
6. **Scratch backend** (`scratchgraph::lower`): reject `CallIndirect` with a
   named diagnostic (Scratch has no function value; §3.3).
7. **Docs**: refresh ABI.md §5 (`!fnptr` drift), the spec matrix, ROADMAP, and
   the `indirect-call` fixture's expectation (moves off `parser` only once the
   whole frontend slice lands).

### What stays out of scope even after this lands

- **Cross-module / dynamic-library addresses**: an id space is module-local.
  Code that obtains a function address from outside the module (a `dlsym`-style
  import) has no static target; it stays an explicit unsupported diagnostic.
  (No `dlsym` exists in this runtime anyway.)
- **First-class Scratch function values**: impossible in the Scratch model;
  recorded as a Scratch-backend gap, not approximated.
- **Closures / fat pointers**: not part of C or LLVM's data model; out of scope.

## 7. Cost and risk

- **IR growth**: each indirect call site becomes a dispatch chain proportional
  to the number of *possible* targets in the module (in practice the functions
  whose address is ever taken). Real callbacks keep this small.
- **Frontend surface**: the parser must handle function *types* in call
  position and operands, which is the bulk of the work and the main risk of
  subtle misparsing — hence the recommendation that it ship as its own tested
  milestone.
- **Correctness**: the interpreter executes `CallIndirect` by table lookup, so
  its result is exact by construction; the VM's dispatch chain is exercised by
  interpreter/VM differential tests (the same harness that guards
  `__sair_mul64`). The `default: Trap` arm means an out-of-range id is a named
  failure on both engines — never a wrong dispatch.

## 8. Recommendation

**Feasible; adopt §5 with option (B).** Do **not** ship it inside this v0.4
milestone — it is a self-contained frontend+IR slice with its own parser risk,
better landed as *LLVM Compatibility v0.5: indirect calls*. Until then the
`indirect-call` fixture stays pinned `parser/unsupported` with its explicit
diagnostic, which is the honest, non-approximating status.
