# VM Runtime-Gap Analysis (Runtime Intrinsic Completion) — closed

> Status: **gap analysis, closed** (2026-09-10). Written as the M-P1 working
> document for the milestone that gave the ISA VM a runtime resolver; the
> inventory and mechanism below were the plan, and §6 records what was
> implemented and measured. Benchmark gate: **v0.4**; spec revision: **v0.5**.
> Companion to
> [`docs/specification/LLVM_COMPATIBILITY.md`](../specification/LLVM_COMPATIBILITY.md)
> (normative matrix), [`docs/specification/RUNTIME.md`](../specification/RUNTIME.md)
> (SART spec §4/§6), [`docs/design/LLVM_TRANSLATION.md`](./LLVM_TRANSLATION.md)
> and [`LLVM_COMPATIBILITY_STATUS.md`](./LLVM_COMPATIBILITY_STATUS.md).

## 1. What "a runtime gap" is

The SAIR **interpreter** is the semantic reference for every LLVM construct.
When it executes a `Call` whose callee has no `define`d body in the module, it
resolves the name from one of three tiers (in order):

1. **`llvm.mem*`** — `llvm.memcpy/memmove/memset.<variant>` flat memory ops
   (`dispatch_memory_intrinsic`).
2. **`llvm.*` bit intrinsics** — `llvm.bswap/ctpop/ctlz/cttz.i<8|16|32|64>`
   pure value expansions (`dispatch_llvm_intrinsic`).
3. **SART builtins** — the 11 `__scratcharch_*` routines through
   `scratcharch_runtime::dispatch_intrinsic` over a `ByteMemory` view of the
   same flat memory.

The **ISA VM** is the frozen-word backend. It lowers a `Call` to a named ISA
`Call` and resolves *all* names at **load** time against the module's defined
functions. A bodyless runtime/intrinsic call therefore fails load with
`undefined function: <name>` — never a silent wrong result. That load-time
rejection is the entire VM runtime gap today, but it is one *surface*: each
name must be realized with the **same semantics** the interpreter applies,
otherwise the VM would become a second, drifting implementation.

This document inventories every bodyless name reachable from the committed
real-clang corpus and the runtime-pipeline test surface, ranks them by
frequency × lowering cost, and records the mechanism (the VM-native runtime
resolver) chosen for the milestone.

## 2. Inventory

> Read this section as the *plan*: it records which names were gaps and why, at
> the time of writing. Every item below was subsequently closed — see §6 for the
> implemented mechanism and the measured outcome.

### 2.1 SART builtins (`__scratcharch_*`) — resolved via `scratcharch-runtime`

The interpreter executes these through the shared
[`IntrinsicRegistry`](../../../crates/scratcharch-runtime/src/intrinsics.rs)
(registry `with_builtins()`, 11 names). Bodies are byte-wise over
`ByteMemory` with **wrapping** address arithmetic (each access bounds-checked
individually → `RuntimeError::MemoryOutOfBounds{addr}`).

| Name | Args | Returns | Corpus / surface today |
|------|------|---------|------------------------|
| `__scratcharch_memcpy(d,s,n)` | 3×`u32` | `dest` ptr | `memory` uses it but the translator already expands the *constant-length* call to byte load/stores → VM-exact; **runtime-length calls are interpreter-only** |
| `__scratcharch_memmove(d,s,n)` | 3×`u32` | `dest` ptr | overlap-safe; same expansion story |
| `__scratcharch_memset(d,c,n)` | 3×`u32` | `dest` ptr | same expansion story |
| `__scratcharch_memcmp(s1,s2,n)` | 3×`u32` | `i32` | runtime_pipeline only (interpreter) |
| `__scratcharch_strlen(s)` | 1×`u32` | `u32` | **`string` corpus fixture — VM blocker** (`undefined function: __scratcharch_strlen`) |
| `__scratcharch_strcmp(s1,s2)` | 2×`u32` | `i32` | runtime_pipeline only |
| `__scratcharch_strcpy(d,s)` | 2×`u32` | `dest` ptr | runtime_pipeline only |
| `__scratcharch_strncpy(d,s,n)` | 3×`u32` | `dest` ptr | runtime_pipeline only |
| `__scratcharch_abort()` | 0 | `void` | runtime_pipeline only |
| `__scratcharch_panic()` | 0 | `void` | runtime_pipeline only |
| `__scratcharch_trap()` | 0 | `void` | runtime_pipeline only |

`__scratcharch_strlen` is the single SART name the real-clang corpus actually
blocks on; the remaining 10 need corpus fixtures so the VM realisation is
*differential-tested*, not just claimed (M-P7).

### 2.2 `llvm.mem*` variants — flat memory ops

`llvm.memcpy/memmove/memset.<variant>`. The translator *expands* only canonical
constant-length (`len` is an SSA constant), non-volatile, `<= 4096` calls into
byte load/store chains (LLVM_TRANSLATION.md §VM-backend). Everything else stays
a bodyless `Call` and is interpreter-only:

- runtime (non-constant) `len`;
- `volatile` immarg `true`;
- oversized (`> MAX_INLINE_MEMOP = 4096`) constant `len`.

The interpreter's semantics for these differ from the SART builtins: it
**prechecks the whole contiguous range**, treats a zero **destination** as
`NullPointer`, copies through a temporary (so overlapping `memmove` is
well-defined), and returns a discarded `I32(0)`. A VM realisation must mirror
*that* semantics — not the SART builtin byte-loop semantics — for the same name
to agree with the interpreter.

No committed corpus fixture hits a runtime-length `llvm.mem*` today (M-P7 will
add one).

### 2.3 `llvm.*` bit intrinsics — pure value expansions

`llvm.bswap/ctpop/ctlz/cttz.i<8|16|32|64>`. Interpreter semantics
(`dispatch_llvm_intrinsic`):

- the width `N` is carried in the name; only 8/16/32/64 are supported;
- the first operand is the value, masked to `N` bits;
- `ctlz`/`cttz` take a trailing `i1` `is_zero_undef` immarg that clang always
  emits `false`; it is read and ignored (SAIR has no poison);
- no-poison policy: `ctlz(0,N) = cttz(0,N) = N`;
- `bswap(x,N)` reverses the low `N/8` bytes only (i8 → identity);
- result is the width-`N` value (i64 → two limbs on the VM).

Corpus: the **`intrinsics` fixture is a VM blocker**
(`undefined function: llvm.bswap.i16`); it exercises bswap/ctpop/ctlz/cttz at
16/32/64.

### 2.4 Other `llvm.*` names

`llvm.trap`, `llvm.debugtrap`, and any other unknown `llvm.*` are *not* runtime
functions the interpreter supports: the interpreter reports
`unsupported llvm intrinsic` / `malformed llvm intrinsic`. On the VM these stay
`undefined function: <name>` at load — an explicit rejection, never a silent
approximation. Out of scope for this milestone (M-P6 unifies `abort`/`panic`/
`trap`/`unreachable`/div0 *categories*; it does not add `llvm.trap` support).

### 2.5 Non-LLVM bodyless names

Any other undeclared bodyless callee (e.g. `printf` without a shim) is
interpreter-`UndefinedFunction` / VM-`undefined function`. Not a runtime-gap
item.

## 3. Ranking

**P0 — corpus blockers + foundational (this milestone, M-P3…M-P6):**

- `__scratcharch_strlen` (blocks `string`).
- the four bit-intrinsic families at 8/16/32/64 (block `intrinsics`).
- the SART memory/string builtins and runtime-length `llvm.mem*`
  (foundational; make the corpus differential-tested in M-P7).
- distinct `abort`/`panic`/`trap` categories (M-P6).

**P1 — same machinery, no corpus blocker yet:**

- `__scratcharch_memcmp/strcmp/strcpy/strncpy` corpus fixtures (M-P7).

**P2 — specialized / rare:**

- oversized non-constant memory ops with adversarial lengths; `memmove`
  worst-case overlap; `strncpy` zero-pad edge at buffer boundaries. All are
  covered by the shared SART bodies + differential tests rather than special
  VM code.

## 4. Selected mechanism (decided 2026-09-10)

The milestone brief M-P2 asked for a unified `RuntimeFunction` /
`RuntimeRegistry` / `RuntimeCall` mechanism and explicitly rejected
(a) hardcoding libc behaviour into the ISA and (b) synthesising a second,
handwritten ISA implementation of every helper. The chosen mechanism is the
**VM-native runtime resolver**:

```
LLVM/SAIR call (bodyless callee)
        │  ISA `Call(name)`  [ISA unchanged]
        ▼
load-time runtime-symbol resolution          (vm.rs load_program)
        │
        ▼
RuntimeRegistry ── name ──▶ RuntimeFunction / FnId
        │                          │  { kind, arg_words, result_words }
        ▼                          ▼
Code::CallRuntime(FnId)      (VM-internal Code variant — the ISA Instruction
        │                     enum stays frozen; only the VM's private `Code`
        ▼                     grows)
execute.rs: native runtime implementation
```

Key properties, matching the decision record:

- **ISA unchanged.** `scratcharch-core` `Instruction::Call(name)` semantics and
  the SA48 word ISA are untouched. Only `crates/scratcharch-vm/src/vm.rs`
  `Code` gains `CallRuntime(u32)`; the `Vm` gains a runtime-function table.
- **Names resolved at load time.** `load_program` resolves each bodyless
  callee once to an `FnId` in the VM's runtime table (deduplicated by name).
  `execute.rs` dispatches on the `FnId`'s kind — an enum match, **no string
  switch on every execution**.
- **Shared registry, same names.** `__scratcharch_*` resolves through the
  `scratcharch-runtime` registry the interpreter already uses, extended with a
  signature (`arg_words` / result shape) so the VM can drive the ISA operand
  stack. The SART bodies run over a `ByteMemory` view of the VM's memory that
  mirrors the interpreter's flat-byte view, so interpreter and VM run the
  *same* `IntrinsicFn` — parity by construction.
- **Engine-specific where it must be.** `llvm.mem*` and the bit intrinsics
  touch memory / SSA cell shapes differently per engine; their *pure value
  math* (bit intrinsics) is shared as a neutral `scratcharch-runtime` leaf so
  neither engine re-derives it, and their memory ranges mirror the
  interpreter's exact `dispatch_memory_intrinsic` rules. Interpreter/VM
  agreement is then enforced by differential tests (M-P7), never assumed.
- **Distinct failure categories.** The registry maps onto distinct `VmError`
  variants: `Abort`, `Panic(msg)`, runtime `Trap`, plus the existing
  `DivisionByZero` and the ISA `Trap{function,pc}` from `unreachable`. A
  runtime `abort`/`panic`/`trap` stays distinguishable from an ISA
  `unreachable` trap.
- **Dependency rule.** `scratcharch-vm` gains a dependency on
  `scratcharch-runtime` (acyclic: SART has zero dependencies). If a cycle ever
  appeared, the escape hatch is a dependency-neutral runtime-API crate.

Separation kept: the existing compiler-generated lowering helpers
(`__sair_shl64`/`__sair_lshr64`/`__sair_ashr64`, `__sair_mul64`,
`__sair_udivrem64`) stay exactly where they are — they are *lowering* helpers,
not runtime-library functions, and are **not** migrated into the runtime
registry.

## 5. Semantics each VM realisation must mirror

| Resolved name | Mirror of interpreter | Failure on VM |
|---------------|------------------------|---------------|
| SART builtin (11) | shared `IntrinsicFn` body over `ByteMemory` adapter | `RuntimeError` → `VmError::Abort` / `Panic(msg)` / runtime `Trap` / `RuntimeError(msg)` |
| `llvm.mem*` | contiguous-range precheck; zero-dst → null; temp copy (memmove-safe); discarded `I32(0)` | null dst / OOB → `VmError::RuntimeError` |
| `llvm.{bswap,ctpop,ctlz,cttz}.iN` | shared bit-math leaf; `N∈{8,16,32,64}`; no-poison `0→N`; width masked | unsupported width rejected at load (`undefined function`) |
| `__scratcharch_strlen` | shared body | unterminated → `RuntimeError::UnterminatedString` → `VmError::RuntimeError` |

## 6. What was implemented and measured

The mechanism of §4 was built as planned: `scratcharch-vm/src/runtime.rs` holds
the resolver and the VM runtime table, `vm.rs` gains `Code::CallRuntime(u32)` and
resolves bodyless names in `load_program`, and `execute.rs` dispatches on the
resolved kind. `scratcharch-runtime` gained `IntrinsicSignature`
(`arg_words`/`result_words`) and the shared `bit_intrinsic_value` leaf, and
`scratcharch-vm` depends on it (acyclic — SART has zero dependencies).

**Every ranked item closed.** The 11 SART builtins resolve through the registry;
the four bit-intrinsic families resolve per width and evaluate through the shared
leaf; runtime-length/volatile/oversized `llvm.mem*` resolve to a flat memory op
with the interpreter's rules; `abort`/`panic`/`trap` map onto distinct VM failure
categories. `__sair_*` lowering helpers were left untouched, as §4 required.

**Measured** (benchmark v0.4, 34 fixtures):

| | v0.3 | v0.4 |
|---|---|---|
| VM stage | 79% (26/33) | **85% (29/34)** |
| `vm-failure` class | 2 (`string`, `intrinsics`) | **0** |
| Corpus | 33 | 34 (`runtime_mem`) |
| Differential tests | 31 | 40 |

Both former blockers (`string` → 5, `intrinsics` → 2018928754) are now exact on
the ISA VM, bit-for-bit against the interpreter. The new `runtime_mem` fixture —
runtime-length memory ops plus every SART string builtin in one program — is
exact on the interpreter, the VM, *and* a native compiler (255).

Interpreter/VM agreement for all of it is enforced by
`vm_differential_tests.rs` (bit intrinsics across families and widths,
runtime-length `llvm.mem*`, the SART string/memory builtins, the distinct failure
categories, and an unknown bodyless callee rejected by both engines), not
assumed.

**Not closed by this milestone** (out of scope, and still explicit rejections):

- other `llvm.*` names the interpreter does not support (`llvm.trap`,
  `llvm.debugtrap`, …) — an `unsupported llvm intrinsic` diagnostic on the
  interpreter, an `undefined function` at VM load;
- any other undeclared bodyless callee — same explicit rejection on both engines.

Neither is a silent-approximation path: both refuse, and both say which name they
refused.
