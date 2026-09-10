# Floating-Point Feasibility Study

> Status: **feasibility study — no implementation yet** (Part 6 of the LLVM
> Compatibility v0.4 "Real-World Coverage Expansion" milestone). It analyses how
> LLVM `f32`/`f64` maps across SAIR, the interpreter, the frozen ISA/VM, and the
> Scratch numeric model — and assigns a *named* classification to each option.
> Companion matrix: [`docs/specification/LLVM_COMPATIBILITY.md`](../specification/LLVM_COMPATIBILITY.md).

## 1. What is at stake

Real `clang -O0` C is full of floating point, but LLVM IR types it *bit-exactly*:
every `fadd`, `fmul`, `fcmp`, and bit-level float→int cast has specified IEEE-754
behaviour (with per-`fast-math` relaxations a program may or may not use). A
compiler/VM stack therefore cannot "sort of" support floats: either each
operation matches IEEE-754 bit-for-bit (rounding, signed zero, NaN payload
propagation, subnormals, ±Inf) or it must say so explicitly and refuse the
program. ScratchArch's stated rule is **never approximate silently** — a float
that is rounded, flushed, or quieted without a named diagnostic would be a
semantic mismatch, not a feature.

The corpus records the gap as the `float` fixture (`float.c`, clang `-O0`,
`expected_stage: parser`, `expected_status: unsupported`).

## 2. The four language surfaces and their float state

### 2.1 LLVM (source semantics to honour)

`f32` and `f64` are IEEE-754 `binary32`/`binary64`. `-O0` clang emits, for the
fixture shapes ScratchArch will see: `fadd/fsub/fmul/fdiv/frem`, `fpext`
(`f32→f64`), `fptrunc` (`f64→f32`), `fptosi/fptoui/sitofp/uitofp`,
`fcmp` (`oeq/ogt/oge/olt/ole/one/ord/ueq/ugt/uge/ult/ule/une/uno`), `select` on
the result, `phi`, loads/stores, and `bitcast` between same-width float/int.
LLVM distinguishes signalling vs quiet NaN and **does not** fold NaN payloads
away at `-O0`; constants may be denormal. Honouring it means: correct rounding
to nearest-even, IEEE comparisons including the unordered predicate family,
signed zero as a distinct value where IEEE demands, ±Inf, NaN semantics, and
correct integer casts with out-of-range → `INT_MIN`/`INT_MAX` / zero-UB rules
and in-range truncation semantics.

### 2.2 SAIR + interpreter (what exists today)

| Capability | State |
|---|---|
| `IrType::F64`, `Constant::F64`, f64 locals/params | present |
| f64 loads/stores | present (byte memory) |
| `Eq` on f64 | present |
| `Add/Sub/Mul/Div/Rem` on f64 | **absent** — the interpreter's `int_binop` accepts integers only |
| `Lt/Gt`/ordered comparisons on f64 | **reject** F64 with a named error |
| casts f64↔int | absent |
| frontend f32/f64 types | parser rejects (`UnsupportedType`) before SAIR is built |

So SAIR can already *carry and move* f64 (a global double, an f64 round-trip
through memory, `icmp`-free programs), but no arithmetic or ordering executes.
This is the natural seam where a future implementation would slot in: SAIR's F64
is a real IEEE-754 f64 cell with no ISA width constraint, so an interpreter-only
implementation can be *exact* without touching the VM.

### 2.3 ISA / VM (the frozen engine)

`scratcharch-core` has `Value::F64`/const-f64 cells, but the ISA word-arithmetic
model (add/sub/bitwise/shift/mul/div on 32-bit words, software helpers realised
from word ops) has **no float opcodes**. The established pattern for a
"wide/decimal/non-word" operation on this ISA is the software helper
(`__sair_mul64`, `__sair_udivrem64`, …) — a program-level routine built from the
word ops and appended to the ISA program. An f64 `add` *could* in principle be
such a helper, but a full IEEE-754 software implementation of add/sub/mul/div/
rem/fcmp/casts with correct rounding and NaN handling is a large, high-risk body
of bit-exact code — the sort of thing that only earns its cost if the VM is a
product surface that must run float C. It is also **not expressible from the
word ops alone at `-O0` fidelity without a double-width intermediate**; the
existing helpers all lower a single high-level op onto the same-width words,
whereas IEEE rounding genuinely needs guard/round/sticky beyond 64 bits.

### 2.4 Scratch backend (the model target)

Scratch/JS numbers **are** IEEE-754 f64. A Scratch `f64` add is literally a
Scratch `+` block; comparisons, trig, `round`, etc. are native. This is the one
surface where f64 is *native and exact*. But Scratch's model has no f32, no NaN
construction, no signed-zero distinction in the block language, no
integer-conversion semantics distinct from JS `Math` coercions, and — for this
pipeline — floats arrive through the SAIR→ScratchGraph lowerer, which is
construction-only. A Scratch mapping of an *integer* program is already exact;
a Scratch mapping of a float program is only exact for the f64 arithmetic core.

## 3. Options and classification (the A/B/C/D decision)

| Option | Meaning | Verdict |
|---|---|---|
| **A — exact on the VM** | IEEE-754 f64 (and f32 via correct f64 emulate) executed by the frozen VM | **deferred** — requires real architectural work (see §3.1) |
| **B — explicitly approximate** | flush-to-zero, qNaN-only, pre-rounded arithmetic, `fcmp` collapsed | **rejected** — violates the never-approximate rule; IEEE predicates/NaN/signed-zero are observable, so "close" is a semantic mismatch |
| **C — interpreter-only, exact f64** | SAIR F64 arithmetic implemented bit-exactly in the interpreter; f32 emulated as f64 with explicit `fptrunc`/`fpext`; VM/Scratch paths record a named `interpreter-only` gap | **near-term adoption** (see §3.2) |
| **D — rejected (parser-enforced)** | f32/f64 types remain a named `UnsupportedType` parse error | **enforced until C lands**, then retired for the exact core |

### 3.1 Why A is deferred, not dismissed

The VM could reach float C via the software-helper route (the ISA precedent is
real), and there is no *conceptual* blocker. The blockers are engineering:
bit-exact IEEE software arithmetic is thousands of lines of branch-heavy code
whose bugs only appear as wrong rounding on a denormal operand; the differential
test burden is proportionally heavy; and every existing VM integer guarantee
(memory byte-exactness, helper invocation by name) must hold around it. Until a
real corpus fixture *requires* VM float execution — i.e. float C that must run
on the ISA, not just the interpreter — the cost is not justified. A is the
honest long-term endpoint, and this study records the precondition that would
trigger it (a VM surface that must execute float programs end to end).

### 3.2 Why C is the near-term target

The interpreter already owns exact f64 *cells* (F64 type, constants, loads,
stores, `Eq`). Completing it means adding IEEE-correct f64 `add/sub/mul/div`,
`rem`, ordered + unordered `fcmp`, and the int↔float casts to the interpreter's
integer-only arithmetic paths — all on SAIR values, none on the ISA. Because
SAIR F64 is unconstrained by the 32-bit word model, this can be **bit-exact**
(round-to-nearest-even, NaN payload/sign handling per IEEE, ±0, subnormals,
±Inf). f32 support is then *correct by emulation*: parse `f32` as f64, widen
operands on entry, narrow results with an explicit exact `fptrunc` (rounding
once, at the truncation, never twice) — which is exactly LLVM's own model of
`f32` as f64-with-rounded-operations. The interpreter-only label is explicit in
the classification: `float` fixtures become `success` at the interpreter stage
and `VM_UNSUPPORTED`/Scratch-gap below it, never silently "supported" on the VM.

### 3.3 Why B is rejected outright

Every IEEE observable that B would flatten — the sign of zero, quiet vs
signalling NaN, the ordering of the unordered predicates, exact rounding — is a
value the C program can read. Flattening any of them silently produces a wrong
result that passes the runner's success gate if we are not careful, and fails it
if we are; either way it is the exact "semantic mismatch" the milestone forbids.
There is no program in the corpus that wants an approximate float. B has no
defensible use here.

### 3.4 D is the enforced posture until C lands

Today the parser rejects `float`/`double`/`half` with `UnsupportedType`, the
`float` fixture is pinned `parser/unsupported`, and the diagnostic names the
type. That is exactly option D and it is correct **as an interim**: it makes the
gap loud and parse-time-cheap rather than a silent wrong answer deeper in the
pipeline. The plan is therefore: D now (already in force), C as the targeted
slice that moves the interpreter row, then D retired only for the subset C
covers (f32/f64 arithmetic, casts, comparisons), with any float shape C does not
reach (e.g. `fp128`, vector float, `llvm.experimental.constrained.*`) keeping a
named D-style rejection. Nothing about D is a commitment that floats "should"
stay unsupported — it is the refusal to fake them.

## 4. Specific semantic traps any implementation must pass

These are the differential probes that separate "exact" from "looks exact":

1. **Signed zero**: `1.0 / -0.0 == -Inf`; `-0.0 < 0.0` is false yet they compare
   `oeq`; `1/(-0.0)` and `1/(0.0)` differ in sign of the result's zero/infinity.
2. **NaN payload & sign**: `fcmp uno/ord`, `fcmp oeq` false for NaN, `select`
   on an unordered compare; `x != x` must hold for NaN but not for ±0.
3. **Subnormals**: gradual underflow must not flush (a helper that flushes is B).
4. **Rounding**: round-to-nearest-even at every operation and at `fptrunc`;
   never double-round (f32 results computed in f64 must be truncated once).
5. **Cast boundaries**: `fptosi` out-of-range → LLVM poison/`INT_MIN`
   implementation behaviour is *specified* by the target; `fptosi` NaN → poison;
   in-range truncation semantics; `sitofp` of `i64` needs an exact path (cannot
   route through a 53-bit f64 blindly — needs the two-step or big-int method).
6. **`frem`**: IEEE remainder (exact, ties-to-even on the quotient), not C
   `fmod` truncation.
7. **`fpext` is free, `fptrunc` is not**: widening f32→f64 is exact and free;
   narrowing rounds and can overflow to ±Inf.

## 5. Recommendation

Adopt **option C** as the near-term slice (interpreter-exact f64 + f32-by-
emulation), keep **D** enforced for everything C does not cover, explicitly
**reject B**, and record **A** as the deferred VM endpoint with its trigger
condition. Ship C as its own milestone (*LLVM Compatibility v0.6: floating
point, interpreter-exact*) with the §4 traps as the differential test core and a
documented interpreter-only boundary below the interpreter row. Until then the
`float` fixture stays pinned `parser/unsupported` — loud, cheap, and honest.
