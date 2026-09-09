# Scratch Numeric Model

> Study of what the Scratch 3.0 number model can represent **exactly**, and what
> it cannot. This is the normative basis for every Scratch-backend decision in
> `scratcharch-scratchgraph/src/lower.rs`: a construct is lowered to Scratch
> only when this document classifies the lowering `EXACT` (unconditionally) or
> `SAFE_SUBSET` (exact under an explicit, stated value bound). Everything else
> is reported with a named diagnostic — never approximated.
> Companion: [`COMPATIBILITY_GAP_ANALYSIS.md`](./COMPATIBILITY_GAP_ANALYSIS.md).

## 1. The Scratch number model

Scratch 3.0 numbers are IEEE-754 double-precision floats (f64):

- Every integer in `[−2^53, +2^53]` is represented exactly.
- Arithmetic (`operator_add`, `operator_subtract`, `operator_multiply`,
  `operator_divide`, `operator_mod`) is f64 arithmetic.
- There is **no integer wrapping**: `operator_add(2000000000, 2000000000)` is
  `4000000000`, not the i32 two's-complement `−294967296`.
- There is **no modulo-2^n reduction** on overflow: `operator_mod(x, 2^32)` must
  be written explicitly.
- Booleans (`true`/`false`) exist; in any numeric context Scratch coerces them
  to `1`/`0`.
- The block palette has **no bitwise and no shift blocks** (`and`/`or`/`xor`,
  `<<`, `>>`, `>>>` do not exist as Scratch operators).

The ScratchGraph runtime adds one more representation:

- **Heap addresses** are indices into the `__scratcharch_heap` list. The runtime
  stores at 1-based list index `addr + 1` (Scratch lists are 1-indexed; SAIR
  pointers are 0-based byte offsets). A heap pointer is therefore an f64 offset
  into a list whose items are f64 numbers. For a heap whose live size stays well
  below 2^53 items, pointer arithmetic is exact f64 arithmetic.

## 2. Classification lattice

| Class | Meaning |
|---|---|
| `EXACT` | Correct for all inputs the construct accepts; no value bound required. |
| `SAFE_SUBSET` | Correct for programs whose executed values stay within a stated bound (usually `2^53`, or the width range without wrapping). The lowering itself is faithful; the *source language* semantics (e.g. i32 wrap) simply do not occur in the subset. |
| `INTERPRETATION_ONLY` | The construct has no VM/Scratch representation; only the SAIR interpreter implements it. |
| `UNREPRESENTABLE` | No exact Scratch representation exists at all; lowering reports `UnsupportedInstruction`. |

A construct is never silently lowered under a bound that the lowering cannot
enforce. The `SAFE_SUBSET` bound is a *semantic contract* documented here and
per-lowering; it is the reason Scratch's row in the benchmark measures *whole
programs that complete correctly on their data*, not a general equivalence
claim.

## 3. Per-representation analysis

### 3.1 Integers

| Width | Exact range on Scratch | Classification | Why |
|---|---|---|---|
| `i1` | 0/1 (or boolean `true`/`false`, coerced) | `EXACT` | Booleans and 0/1 numbers are exactly 0/1 under Scratch coercion. |
| `i8` / `i16` | value in `[0, 2^n)` as stored; casts reduce mod `2^n` | `SAFE_SUBSET` | In-range byte/word loads are exact. i8/i16 *arithmetic* does not wrap in Scratch (`200+100 = 300`, not 44), so an i8 value is only guaranteed in range if the program keeps it there. All *casts* from i8/i16 are reduced mod `2^n` first, so casts stay exact for any carried value. |
| `i32` | values that never overflow i32 range | `SAFE_SUBSET` | f64 holds every i32 exactly, and `operator_add`/`sub`/`mul` on such values is exact f64. i32 *wrapping* is not modeled (no mod-2^32), so a program whose arithmetic crosses the i32 boundary is outside the subset. Signed vs unsigned: wherever an i32 participates in arithmetic or comparison it carries the *mathematical* signed value (`-5`, never `0xFFFFFFFB`); SAIR constants arrive as raw patterns and are normalized by casts and by width-aware memory loads (see §3.3). Signed predicates and signed comparisons are therefore naturally exact. |
| `i64` | `[−2^53, +2^53]` | `SAFE_SUBSET` | Values beyond 2^53 lose integer exactness, and i64 wrap (mod 2^64) is unrepresentable. Within the bound, add/sub/mul and signed compare are exact f64. |
| `i64 → i32` trunc | input `[−2^53, +2^53]` | `SAFE_SUBSET` | Lowered as `m = x mod 2^32; if m ≥ 2^31 then m − 2^32 else m` (two's-complement reinterpret). Exact whenever the i64 value is exact. |

### 3.2 Width casts (the exact set)

All of the following are **exact** on any value the source width can carry
(they reduce mod the source width first, so even an out-of-subset carried value
casts correctly):

| SAIR cast | Scratch lowering |
|---|---|
| `zext i1 → iN` | `(x ? 1 : 0)` — boolean/0-1 coerced to number |
| `zext i8 → iN` | `x mod 256` |
| `zext i16 → iN` | `x mod 65536` |
| `zext i32 → i64` | `if x < 0 then x + 2^32 else x` |
| `sext i8 → iN` | `m = x mod 256; if m ≥ 128 then m − 256 else m` |
| `sext i16 → iN` | `m = x mod 65536; if m ≥ 32768 then m − 65536 else m` |
| `sext i32 → i64` | identity (Scratch already holds the mathematical signed value) |
| `trunc i64 → i32` | `m = x mod 2^32; if m ≥ 2^31 then m − 2^32 else m` |
| `trunc iN → i1/i8/i16` | `x mod 2^N` (the low N bits as a non-negative value) |
| `ptrtoint i64` | identity (pointers *are* f64 offsets) |
| `inttoptr i64` | identity (offset number used as a heap address) |

Classification: **`EXACT`** for i1; `SAFE_SUBSET` only where the *source value*
is not guaranteed to be in its width range (i8/i16 arithmetic). In practice the
corpus fixtures keep byte values in range, so every fixture exercised here is
bit-for-bit exact. Note the mod-first form means a cast is never *wrong* — an
out-of-range carried value still yields the correct low bits.

### 3.3 Pointers and addresses

- Pointers are heap offsets (0-based byte offsets into `__scratcharch_heap`,
  stored 1-indexed). Classification: **`EXACT`** for any heap size ≪ 2^53.
- `ptrtoint`/`inttoptr`: identity — an address *is* a number. **`EXACT`**.
- GEP: the LLVM translator folds every GEP into a single `Dynamic` byte offset
  (with struct padding applied at translation time), so Scratch only ever does
  `HeapIndex(base, offset)` with offset = `base + 1 + offset`. **`EXACT`**.
- The 1-indexing shift is a fixed ABI convention applied at Store/Load, not a
  per-value adjustment; it cannot silently lose data.

### 3.4 Byte-exact memory (loads and stores)

The ScratchGraph heap is **byte-addressable** (`docs/specification/SCRATCH_MEMORY.md`):
one list item per byte, little-endian, width-exact (`i1`/`i8` = 1 byte, `i16` =
2, `i32` = 4, `i64` = 8, `ptr` = 4). The value convention is the *mathematical
signed* integer, reconciled with SAIR raw-bit semantics by two rules:

- **Store** reduces the value mod `2^(8·N)` first, then writes N little-endian
  bytes. Because `operator_mod` is floor-mod and the raw pattern and the
  mathematical value are congruent mod `2^(8·N)`, a store writes the correct
  bytes whether the carried f64 was `0xFFFF_FFFB` or `-5`. Neighboring bytes
  are untouched.
- **Load** recomposes the raw pattern `m` then reinterprets the high bit for
  `i8`/`i16`/`i32`: `m − 2^(8·N)` when `m ≥ 2^(8·N−1)`, else `m`. `i64` and
  `ptr` loads return the raw non-negative pattern.

Classification: **`EXACT`** for the memory operations themselves within the
documented bounds; the bounds are `SAFE_SUBSET`:

- `i64` in memory: `[0, 2^53)` — every stored/loaded `i64` must be an exact
  f64 (negative or ≥ 2^53 `i64` is unrepresentable).
- Raw-bit consumption of a loaded value at or above `2^(N−1)` (unsigned
  comparisons against large constants, e.g. `icmp uge i8 %l, 200`) is outside
  the subset — loads normalize to the signed value.
- Arithmetic on loaded values must not depend on mod-2^N wrapping.

The corpus keeps every executed value inside these bounds; a fixture that
violates one is classified by the construct that leaves the subset.

### 3.5 Booleans and predicates

Scratch comparisons (`operator_lt`, `operator_gt`, `operator_equals`) return
`true`/`false`, which coerce to 1/0 in arithmetic. Signed predicates on
mathematical (signed) i32/i64 values are therefore exact. Classification:
**`EXACT`**.

### 3.6 Bitwise and shift

`and`/`or`/`xor`/`shl`/`lshr`/`ashr` have **no Scratch operator**. An f64
arithmetic emulation of i32 bitwise is theoretically possible (i32 fits f64
exactly; XOR/AND/OR can be written as mod-2^32 arithmetic) but is large, slow,
and fragile, and yields no benefit for the target user (block programming).

Classification: **`UNREPRESENTABLE`** on the Scratch backend. The lowerer emits
`UnsupportedInstruction("<op> cannot be lowered to Scratch numbers")`. These
constructs remain interpreter-only on the Scratch path. (The VM executes the
full i32/i64 bitwise/shift family exactly — see
`docs/specification/LLVM_COMPATIBILITY.md`.)

### 3.7 Beyond integers

| Construct | Scratch classification |
|---|---|
| `float` / `double` types | `UNREPRESENTABLE` (parser rejects; f64 literals would silently round) |
| vector types | `UNREPRESENTABLE` (parser rejects) |
| `atomicrmw` / atomics | `UNREPRESENTABLE` (parser rejects; no concurrency model) |
| `llvm.*` runtime intrinsics (bswap/ctpop/mem* …) | `INTERPRETATION_ONLY` where not folded; the SAIR interpreter resolves them, VM/Scratch report "undefined function" |
| indirect calls | `INTERPRETATION_ONLY`; see `FUNCTION_POINTERS.md` |

## 4. What this means for the benchmark

- The Scratch row measures **whole programs correct on their data**, under the
  `SAFE_SUBSET` contract. A fixture that keeps its executed values inside the
  f64-exact range is fully exact; one that relies on i64 width or i32 wrapping
  beyond the bound is classified by the construct it hits.
- Raising a fixture to Scratch success requires: (a) every construct it uses
  classified `EXACT` or `SAFE_SUBSET` here, and (b) a passing differential
  (native ↔ interpreter ↔ VM ↔ Scratch runtime).
- No fixture is ever made "pass" by a lowering that relies on floating-point
  rounding. The mod-first cast forms and the explicit `SAFE_SUBSET` bounds are
  the enforcement mechanism.

## 5. Validation boundary (what "passes" means)

Construction success is **not** full semantic execution. The Scratch backend
lowers SAIR to a `Project` and verifies the lowering by formula and structure;
it has no standalone executor. The three levels of evidence are distinct and
must not be conflated:

1. **Construction** — every construct lowers to a `Project` or is rejected with
   a named diagnostic (`Ok(project)` from `ScratchGraphLowerer::lower`). A
   fixture's benchmark `scratch.success` records construction only.
2. **Formula/structural verification** — the tests in
   `crates/scratcharch-scratchgraph/tests/` assert the emitted expressions and
   project structure: cast formulas at every width, byte split/recombine
   exactness, little-endian order, width preservation, signed-reinterpretation
   at the width boundary, heap layout (`item = addr + 1`), and static-data
   seeding. This is the authoritative evidence that the lowering is faithful to
   SAIR semantics *under the `SAFE_SUBSET` bounds*.
3. **Execution** — the interpreter and VM execute; the Scratch backend does
   not. A future standalone ScratchGraph reference executor (a separate
   milestone) will evaluate whole `Project`s and close the gap between formula
   evidence and execution.

The classification lattice (§2) is a *lowering contract*: it states what the
emitted Scratch *would* compute given the bounds. Whole-program correctness on
real Scratch remains unclaimed until the executor milestone exists.
