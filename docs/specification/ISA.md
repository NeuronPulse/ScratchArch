# ScratchArch ISA (Instruction Set Architecture)

> Specification version: **v0.1 (draft for freezing)**
> Status: normative for the `sa48` reference profile; parametric for all profiles.
> Companion documents: [`ABI.md`](./ABI.md), [`MEMORY.md`](./MEMORY.md), [`README.md`](./README.md).

---

## 1. Motivation

ScratchArch is a **standalone virtual architecture**. It is not a Scratch compiler and it
does not describe Scratch. It describes an abstract machine — the *ScratchArch Abstract
Machine* (SAM) — that is designed to be:

1. a practical **LLVM compilation target**, and
2. **implementable on runtimes that lack native integers, memory, or bitwise operators**
   (a floating-point substrate such as Scratch/TurboWarp being the motivating example, but
   not the only one).

The predecessor project, `llvm2scratch`, proved that LLVM IR can be lowered onto a
double-precision-float substrate. The archaeology of that project (see
`archaeology-python.md`, `archaeology-rust.md`) is the **design evidence** behind this
document, but it is not a requirement. Where `llvm2scratch` baked Scratch details into the
architecture (procedures-per-block, lookup tables, list-based memory, a 48-bit hard limit),
ScratchArch extracts the *stable* idea and discards the *prototype* detail.

The single most important stable idea is this:

> **A ScratchArch machine computes on scalar *cells* whose exact-integer capacity may be
> narrower than the integer widths a program uses.**

Everything else in the ISA follows from taking that constraint seriously while remaining a
faithful LLVM target.

### What problem the ISA solves

| Problem from the prototype | ISA-level resolution |
|---|---|
| The 48-bit limit was hardcoded and Scratch-flavored | Cell width `W` is a **profile parameter**; `sa48` is the reference, `sa64` a native profile |
| Bitwise ops were "lookup tables" (a Scratch trick) | Bitwise ops are **primitive instructions** with exact semantics; table lowering is a runtime concern |
| Control flow was "one Scratch procedure per basic block" | Control flow is **basic blocks + terminators**, like LLVM; dispatch strategy is a runtime concern |
| Signed math was a tangle of 4-case trees | Instructions have clean **mathematical semantics**; the 4-case lowering is documented as a *limited-precision realization note*, not as the definition |
| Everything was untyped floats at the Scratch level | The ISA is **typed** (`iN`, floats, `ptr`, aggregates) exactly like LLVM |

---

## 2. Architecture definition

### 2.1 The machine

A SAM consists of:

- **Code**: a set of *functions*. Each function is a list of *basic blocks*. Each basic
  block is a list of *instructions* ending in exactly one *terminator*.
- **Virtual registers**: an unbounded supply of typed, single-assignment (SSA) values.
  There is no fixed register file and no register allocation at the architecture level —
  a register is simply a named typed value. (Rationale in §2.5.)
- **Memory**: a single flat linear address space of *addressable units*. Defined normatively
  in [`MEMORY.md`](./MEMORY.md).
- **A call mechanism**: functions call functions; activations are independent. Defined
  normatively in [`ABI.md`](./ABI.md).

The SAM is deliberately close to LLVM IR in shape. The novelty is not the shape; it is the
**cell model** that constrains how values are realized.

### 2.2 The cell model

A **cell** is the fundamental scalar unit of computation and storage. A cell holds exactly
one of:

- an **integer** in the range `[0, 2^W)`, represented exactly, or
- a **real number** (an IEEE-754-double-compatible value), for floating-point values.

`W` (the *cell width*, in bits) is a **profile parameter**:

| Profile | `W` (cell width) | `Wptr` (pointer width) | Intermediate exact-int precision `P` | Intended runtime class |
|---|---|---|---|---|
| **`sa48`** (reference) | 48 | 32 | 53 | Float substrate (Scratch/TurboWarp-class) |
| `sa64` (native) | 64 | 64 | 64+ | Native VM / Wasm / real hardware |

`W`, `Wptr`, `P`, endianness, and the memory addressing unit together form a **profile**.
The ISA *semantics* below are defined identically for every profile; the profile only
changes how wide a single cell is and therefore how logical values decompose into cells (see
§2.4 and [`ABI.md`](./ABI.md) §2).

> **Why parameterize instead of fixing 48?** The 48-bit number came from JavaScript's
> 53-bit double mantissa minus headroom — a property of *one* runtime. Freezing it into the
> architecture would make ScratchArch secretly a Scratch spec. Parameterizing it keeps a
> single normative semantics while letting a native VM use full-width cells. The alternative
> — fixing `W = 64` like an ordinary machine — was rejected because it throws away the entire
> reason ScratchArch exists: faithful lowering onto narrow-precision runtimes.

### 2.3 Integer representation

- Integers are stored **unsigned**: a value of type `iN` is the non-negative residue in
  `[0, 2^N)`. **Signedness is a property of operations, not of values** (exactly as in LLVM:
  `sdiv` vs `udiv`, `icmp slt` vs `icmp ult`).
- The two's-complement interpretation of an `iN` value `v` is
  `v` if `v < 2^(N-1)`, else `v − 2^N`. Signed instructions are *defined* by this
  interpretation; they do not require a signed storage format.
- All integer arithmetic is **total and wrapping**: results are taken `mod 2^N`. There are no
  trap-on-overflow semantics at the ISA level. LLVM `nsw`/`nuw` poison flags are accepted as
  **optimization hints** and carry no architectural obligation (see §7).

### 2.4 Value width and cell decomposition (summary)

A logical value of type `iN` occupies `ceil(N / W)` cells, ordered **little-endian** (least
significant cell first). Values with `N ≤ W` are *single-cell*; wider values are
*multi-cell*. Floats occupy one cell. Pointers are `Wptr`-bit integers and occupy
`ceil(Wptr / W)` cells (one cell in both reference and native profiles).

The ISA is written in terms of **logical typed values**. The concrete cell tuple for a value
is defined by the ABI ([`ABI.md`](./ABI.md) §2), so that the ISA semantics stay independent
of `W`. This is the clean version of the prototype's `InferredValue::Single | Indexed`
distinction: single/multi-cell is a *derived* property of a type under a profile, not a
separate kind of value the programmer or frontend must track.

### 2.5 Why SSA virtual registers (no register file)

LLVM produces SSA. The prototype mapped every SSA value to its own storage slot and never
did register allocation, because on the target runtime "variables" are unlimited and free.
ScratchArch keeps that: virtual registers are SSA, typed, and unbounded.

- **Alternative considered — a fixed register file** (e.g. 16 general registers): rejected.
  It would force a register-allocation pass with spills, which is pure overhead for runtimes
  whose storage is unbounded, and it would make the architecture *less* faithful to LLVM.
- Registers are **function-local and per-activation** (see [`ABI.md`](./ABI.md) §4): recursion
  gives each activation its own register values. This is the architecture-level meaning of
  the prototype's "save registers to the local stack around recursive calls."

---

## 3. Type system

The type grammar is intentionally LLVM-shaped so that lowering is a near-identity map.

```
Type ::=
    | "void"
    | "i" N                         ; arbitrary-width integer, N ≥ 1
    | "half" | "float" | "double" | "fp128"
    | "ptr" [ addrspace ]           ; opaque pointer, width = Wptr
    | "[" M "x" Type "]"            ; array of M elements
    | "{" Type,* "}"               ; struct (default aligned)
    | "<{" Type,* "}>"             ; packed struct
    | "func" "(" Type,* [",..."] ")" "->" Type
    | "<" M "x" Type ">"           ; vector (reserved; see §9)
```

Notes:

- **Opaque pointers.** Pointers do not carry pointee type (matching modern LLVM). Element
  type information lives in the `getelementptr`, `load`, and `store` instructions.
- **`i1` (boolean)** is an integer type whose values are `0` and `1`. Comparisons produce
  `i1`. There is no separate boolean value class.
- **Float widths** (`half`/`float`/`double`/`fp128`) are semantically distinct (rounding,
  range) but every profile is permitted to *store* them in a single cell at
  double-or-better precision. `fptrunc`/`fpext` between representable formats may be
  identity on a profile whose cell already holds the wider format losslessly; the ISA only
  requires that observable float semantics match IEEE-754 for the *declared* type.
- Type sizes and alignment are defined in [`MEMORY.md`](./MEMORY.md) §4.

---

## 4. Instruction set

Notation: `%d = op T %a, %b` means "define register `%d` of type `T` as the result of `op`".
For an `iN` operand, `⟦%a⟧` is its unsigned value and `sⁿ(%a)` its two's-complement signed
value. `mod⁺` denotes the non-negative modulus (`a mod⁺ m ∈ [0, m)`).

Every instruction's **semantics** below are runtime-neutral. Where a limited-precision
runtime (like `sa48`) cannot evaluate the naive formula without exceeding its intermediate
precision `P`, a boxed **Realization note** records the technique proven by the archaeology.
Those notes are informative, not normative: any evaluation producing the specified result is
conforming.

### 4.1 Integer arithmetic

| Instr | Result type | Semantics |
|---|---|---|
| `add`  | `iN` | `(⟦a⟧ + ⟦b⟧) mod 2^N` |
| `sub`  | `iN` | `(⟦a⟧ − ⟦b⟧) mod⁺ 2^N` |
| `mul`  | `iN` | `(⟦a⟧ · ⟦b⟧) mod 2^N` |
| `udiv` | `iN` | `floor(⟦a⟧ / ⟦b⟧)`; `⟦b⟧ = 0` is undefined |
| `sdiv` | `iN` | `trunc(sⁿ(a) / sⁿ(b))` (round toward zero), re-encoded mod 2^N; `b = 0` or `INT_MIN/−1` undefined |
| `urem` | `iN` | `⟦a⟧ − ⟦b⟧·floor(⟦a⟧/⟦b⟧)` |
| `srem` | `iN` | `sⁿ(a) − sⁿ(b)·trunc(sⁿ(a)/sⁿ(b))` (sign of dividend), re-encoded mod 2^N |

> **Realization note (`sa48`).** For `add`/`sub`/`mul`, a runtime whose exact-integer range
> is `2^P` must keep intermediates below `2^P`. `mul` uses half-splitting
> `a·b = a₀b₀ + 2^w(a₀b₁ + a₁b₀) + 2^{2w}a₁b₁` with `w = ceil(N/2)` and a `mod` step on the
> middle term when `N` is large enough to overflow `P`. `sdiv`/`srem` are realized by the
> four-case sign tree on `sⁿ`. These are the archaeology's `multiplyWrap`, `sdiv`, and
> `srem` algorithms. They are *how* `sa48` computes the values above; they are not the
> definition.

### 4.2 Bitwise and shift

| Instr | Result type | Semantics |
|---|---|---|
| `and` | `iN` | bitwise AND of the `N`-bit patterns |
| `or`  | `iN` | bitwise OR |
| `xor` | `iN` | bitwise XOR (`xor a, -1` is bitwise NOT) |
| `shl`  | `iN` | `(⟦a⟧ · 2^⟦b⟧) mod 2^N`; `⟦b⟧ ≥ N` undefined |
| `lshr` | `iN` | `floor(⟦a⟧ / 2^⟦b⟧)`; `⟦b⟧ ≥ N` undefined |
| `ashr` | `iN` | `floor(sⁿ(a) / 2^⟦b⟧)` re-encoded mod 2^N (sign-replicating) |

> **Realization note (`sa48`).** Bitwise ops are **primitive** in ScratchArch. A native
> runtime uses native AND/OR/XOR. A float-substrate runtime realizes them with precomputed
> byte-chunk lookup tables (`table[a·256 + b]`) and the identity formulas
> `a|b = (a & ¬b) + b`, `a^b = a + b − 2(a & b)`, splitting operands wider than a chunk. That
> the *architecture* exposes bitwise ops as primitives — rather than as "lookup tables" — is
> the key correction over the prototype, where the table was mistaken for the concept.
> Shifts are realized by multiply/divide against a power-of-two table; multi-cell shifts
> carry bits across the cell boundary: `(lo, hi) << b = (lo<<b, (hi<<b) | (lo >> (W−b)))`.

### 4.3 Comparison

`icmp <pred> iN %a, %b → i1` with predicates
`eq, ne, ugt, uge, ult, ule, sgt, sge, slt, sle`. Unsigned predicates compare `⟦·⟧`; signed
predicates compare `sⁿ(·)`.

`fcmp <pred> FT %a, %b → i1` with the LLVM ordered/unordered predicate set
(`oeq, ogt, …, ueq, …, ord, uno`). NaN handling follows IEEE-754: *ordered* predicates are
false if either operand is NaN; *unordered* predicates are true if either is NaN.

> **Realization note (`sa48`).** Signed `icmp` reverses two's complement before comparing;
> `fcmp` guards NaN and ±∞ explicitly. (Prototype `intCompare`, `fcmp` handling.)

### 4.4 Conversions

| Instr | Semantics |
|---|---|
| `trunc iN %a to iM` (`M<N`) | `⟦a⟧ mod 2^M` |
| `zext iN %a to iM` (`M>N`)  | value-preserving (identity on `⟦a⟧`) |
| `sext iN %a to iM` (`M>N`)  | sign-replicating: `sⁿ(a) mod⁺ 2^M` |
| `fptoui`, `fptosi` | truncate toward zero to integer, then encode mod 2^N |
| `uitofp`, `sitofp` | integer → nearest float |
| `fptrunc`, `fpext` | change float format per IEEE-754 |
| `ptrtoint`, `inttoptr` | reinterpret between `ptr` and integer (mod `2^Wptr`) |
| `bitcast T %a to U` | reinterpret the bit pattern; `sizeof(T) = sizeof(U)` required |

> **Realization note (`sa48`).** `zext`/`uitofp` are identity (float cells have unbounded
> integer-free range). `sext` adds a sign-dependent offset. `bitcast` between float and
> integer requires IEEE-754 decomposition (logarithmic exponent estimate + mantissa
> extraction) because the cell does not expose its bits directly. (Prototype §5 "IEEE 754
> Bitcast".)

### 4.5 Floating-point arithmetic

`fadd, fsub, fmul, fdiv, frem, fneg` with IEEE-754 semantics for the declared float type.
NaN, ±∞, and ±0 propagate per IEEE-754. `frem` follows LLVM (result has sign of the
dividend and magnitude `|a| − |b|·trunc(|a|/|b|)`).

### 4.6 Aggregate and selection

| Instr | Semantics |
|---|---|
| `select i1 %c, T %t, T %f` | `%t` if `%c = 1` else `%f` |
| `extractvalue`, `insertvalue` | read/write a field of a struct/array *value* by constant index |
| `freeze T %a` | returns `%a`, or an arbitrary fixed value of `T` if `%a` is poison/undef |
| `phi T [%v1,%bb1], [%v2,%bb2], …` | value selected by the predecessor edge taken (see §5.3) |

### 4.7 Memory instructions (defined in MEMORY.md)

`alloca`, `load`, `store`, `getelementptr` have their full semantics in
[`MEMORY.md`](./MEMORY.md). They appear in the ISA only as the interface between computation
and the address space.

### 4.8 Calls (defined in ABI.md)

`call` and `ret` semantics — argument/return transmission, activation lifetime — are defined
in [`ABI.md`](./ABI.md). The ISA fixes only their *shape*: `call` transfers control to a
function and yields its return value as a register; `ret` terminates the current activation.

---

## 5. Control flow

### 5.1 Basic blocks and terminators

A function is a set of basic blocks; the first is the *entry*. Each block is a straight-line
sequence of non-terminator instructions followed by exactly one **terminator**:

| Terminator | Meaning |
|---|---|
| `br label %dst` | unconditional branch |
| `br i1 %c, label %t, label %f` | conditional branch |
| `switch iN %v, label %default [ %k1, %l1 … ]` | multi-way branch on constant keys |
| `ret [T %v]` | return from function |
| `unreachable` | control must not reach here (undefined if it does) |

There is **no fall-through**: every block ends in a terminator. This is identical to LLVM and
independent of how a runtime dispatches between blocks.

> **Why not "one procedure per block"?** The prototype compiled each block to a Scratch
> custom procedure and each branch to a procedure call — and offered a "branch jump table"
> alternative for TurboWarp. Both are **dispatch strategies of a runtime**, not properties of
> the architecture. ScratchArch specifies only the CFG and its semantics. A VM may realize
> branches as host calls, as a `switch`-in-a-loop dispatcher, as threaded code, or as native
> jumps. Removing this Scratch-ism is the biggest single de-leak in the ISA.

### 5.2 `switch`

`switch` selects the block whose key equals `⟦%v⟧`, else `%default`. Keys are distinct
compile-time constants. The ISA does not mandate a search structure; a limited runtime may
lower it to a binary-search tree of comparisons (prototype), a jump table, or a hash — all
observationally equal.

### 5.3 `phi` and edge semantics

`phi` values are logically selected **on the control-flow edge** that entered the block: on
taking edge `bbₖ → bb`, every `phi` in `bb` takes its `bbₖ` operand, and all `phi`s in `bb`
read their operands *simultaneously* (before any is written). This makes SSA
well-defined without prescribing an implementation.

> **Realization note.** The prototype resolved `phi` by emitting the assignments at the
> *branch site* (on the edge), breaking assignment cycles with temporaries. That is exactly
> the edge semantics above and is the recommended lowering, but the architecture only
> requires the observable result.

---

## 6. Examples

The examples use a compact LLVM-like text syntax purely for illustration; ScratchArch does
not mandate a concrete surface syntax for v0.1.

### 6.1 Wrapping 8-bit add (single-cell, any profile)

```
; i8 add is exact mod 256 in every profile
define i8 @add8(i8 %a, i8 %b) {
entry:
  %s = add i8 %a, %b        ; ⟦s⟧ = (⟦a⟧ + ⟦b⟧) mod 256
  ret i8 %s
}
```

### 6.2 Signed division truncating toward zero

```
define i32 @sdiv_ex(i32 %a, i32 %b) {
entry:
  %q = sdiv i32 %a, %b      ; trunc(s³²(a)/s³²(b)); e.g. sdiv(-7, 2) = -3, not -4
  ret i32 %q
}
```
On `sa48` this is realized by the four-case sign tree; the *result* is the truncated
quotient regardless.

### 6.3 A 64-bit multiply as seen by two profiles

```
define i64 @mul64(i64 %a, i64 %b) {
entry:
  %p = mul i64 %a, %b       ; (⟦a⟧·⟦b⟧) mod 2^64
  ret i64 %p
}
```
- On `sa64` (`W=64`): one cell each; a single native/bignum multiply mod 2^64.
- On `sa48` (`W=48`): `%a`, `%b`, `%p` each decompose into 2 cells (little-endian: bits
  `[0,48)` then `[48,64)`); the multiply is the half-split algorithm across cells. The ISA
  semantics are identical; only the ABI-level cell decomposition differs.

### 6.4 Control flow with `phi`

```
define i32 @max(i32 %a, i32 %b) {
entry:
  %gt = icmp sgt i32 %a, %b
  br i1 %gt, label %ta, label %tb
ta:
  br label %join
tb:
  br label %join
join:
  %r = phi i32 [ %a, %ta ], [ %b, %tb ]
  ret i32 %r
}
```

---

## 7. Relationship with LLVM

ScratchArch is designed so that an LLVM backend targeting it is close to a structural
translation.

| LLVM concept | ScratchArch ISA |
|---|---|
| SSA values, virtual registers | SSA virtual registers (§2.5) — 1:1 |
| `iN`, floats, `ptr`, arrays, structs | Same type grammar (§3) |
| `add/sub/mul/div/rem/shl/…` | Same instructions, exact wrapping semantics (§4) |
| `nsw`/`nuw`/`exact` poison flags | Accepted as hints; wrapping is always total (§2.3) |
| `icmp`/`fcmp` predicates | Same predicate sets (§4.3) |
| `getelementptr`, `load`, `store`, `alloca` | Same, semantics in [`MEMORY.md`](./MEMORY.md) |
| `br`, `switch`, `ret`, `unreachable`, `phi`, `select` | Same CFG and terminators (§5) |
| `call`, varargs, function pointers | Same, ABI in [`ABI.md`](./ABI.md) |
| Intrinsics (`llvm.memcpy`, …) | Extensible intrinsic registry (§8) |

**Backend obligations.** An LLVM→ScratchArch backend must (a) legalize integer widths only
as far as the target profile requires — it does *not* need to legalize down to a fixed
register width, because ScratchArch accepts arbitrary `iN`; (b) preserve SSA or perform its
own out-of-SSA lowering (both are conforming since `phi` has edge semantics); (c) leave
poison-flag optimizations to the middle-end. Because ScratchArch accepts arbitrary-width
integers and does not force a fixed register file, the backend is *simpler* than a typical
hardware backend, not harder.

---

## 8. Intrinsics

Intrinsics are named operations outside the core instruction set (`memcpy`, `memset`,
`memmove`, `ctpop`, `ctlz`, `cttz`, `bswap`, saturating arithmetic, overflow-checked
arithmetic, `trap`, etc.). Per the Rust archaeology's redesign guidance, intrinsics are an
**extensible registry**, not a closed enum:

- Each intrinsic has a name, a signature, and a semantics defined in terms of core
  instructions (a *reference expansion*).
- A profile/runtime may implement an intrinsic directly (faster) or via its reference
  expansion (always correct).
- Unknown intrinsics are a **compile-time error**, never silently dropped.

The v0.1 **core intrinsic set** to be frozen: `memcpy`, `memmove`, `memset`, `bswap`,
`ctpop`, `ctlz`, `cttz`, `abs`, `smin/smax/umin/umax`, `sadd/uadd/ssub/usub.with.overflow`,
and `trap`. Everything else is an extension.

---

## 9. Future extensions

- **Vectors (`<M x T>`).** Present in the type grammar but not yet given elementwise
  instruction semantics; reserved for v0.2. (The prototype parsed but never lowered them.)
- **Atomics / concurrency.** The v0.1 machine is single-threaded and sequential. A memory
  model for concurrency is future work; the current model is trivially sequentially
  consistent because there is one thread.
- **Wider/other profiles.** Additional profiles (e.g. `sa32` for 32-bit native, a
  BigInt profile with `W = ∞`) can be added without changing instruction semantics.
- **Exceptions / unwinding.** `invoke`/`landingpad` and `setjmp`/`longjmp` are out of core
  v0.1; the prototype modeled `setjmp`/`longjmp` as activation-id + stack snapshots, which is
  the intended future direction (see [`ABI.md`](./ABI.md) §7).
- **Explicit poison/undef propagation.** v0.1 treats `undef`/poison operationally via
  `freeze`; a fuller poison semantics is deferred.

---

## 10. Frozen decisions (v0.1)

These are proposed **frozen** for v0.1 (rationale consolidated in [`README.md`](./README.md)):

1. Integers stored unsigned; signedness is an operation property.
2. All integer arithmetic is total and wraps mod 2^N; poison flags are hints only.
3. Cell width `W` is a profile parameter; `sa48` (W=48, Wptr=32) is the reference profile.
4. Multi-cell values are little-endian.
5. Bitwise and shift operations are primitive instructions.
6. Control flow is basic-blocks-plus-terminators with no fall-through; dispatch strategy is
   not part of the architecture.
7. `phi` has simultaneous, edge-selected semantics.
8. The type grammar is LLVM-shaped with opaque pointers.
9. Intrinsics form an extensible registry with reference expansions.
