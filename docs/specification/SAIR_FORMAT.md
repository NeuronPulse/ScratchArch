# ScratchArch SAIR Text Format v0.1

This document defines the stable textual representation of the ScratchArch
Intermediate Representation (SAIR). The format is independent of the Rust
implementation: an `IrModule` serialized to text can be parsed back into a
semantically equivalent module, and the textual IDs are renumbered during
parsing.

## 1. Overview

A SAIR text file contains:

1. A version header.
2. An optional entry function declaration.
3. One or more function definitions.

```text
sair 0.1
entry "main"

func @main -> i32 entry "entry" {
  block "entry":
    %0 = const i32 42
    ret i32 %0
}
```

## 2. Lexical Conventions

- Whitespace (spaces, tabs, newlines) separates tokens.
- A `;` begins a comment that extends to the end of the line.
- Strings are delimited by double quotes (`"`). The escape sequence `\"`
  represents a literal double quote; `\\` represents a literal backslash.
- Identifiers for function names and block labels are written as strings.
- Global names (functions) are prefixed with `@`.
- Local value references are prefixed with `%`.

## 3. File Header

```text
sair 0.1
entry "<function-name>"
```

The `entry` line names the function where execution begins. It must refer to a
function defined in the file.

## 4. Functions

```text
func @<name> -> <type> [entry "<block-label>"] {
  <param>*
  <block>+
}
```

- `<type>` is the function's return type.
- `entry "<block-label>"` declares the entry block. If omitted, it defaults to
  `"entry"`.
- Parameters are declared before blocks.

### Parameters

```text
param <type> %<id>
```

Parameters are assigned `ValueId`s `0, 1, ..., p-1` in declaration order.

## 5. Basic Blocks

```text
block "<label>":
  <instruction>*
  <terminator>
```

- Every block ends with exactly one terminator.
- The first block in a function is the entry block.

## 6. Types

| Type | Meaning |
| ---- | ------- |
| `i1` | 1-bit boolean |
| `i8` | 8-bit integer |
| `i16` | 16-bit integer |
| `i32` | 32-bit integer |
| `f64` | 64-bit IEEE-754 floating point |
| `ptr` | Pointer |
| `void` | No value |

## 7. Values

A value is referenced by `%<id>`, where `<id>` is a non-negative decimal
integer. The textual `%<id>` is only a label; the parser renumbers values
internally in declaration order.

The textual IDs in a well-formed file must satisfy:

1. Parameters use the IDs declared by their `param` lines.
2. A defined value (the result of an instruction that produces a value) is
   referenced only after it has been defined.
3. The canonical writer emits dense IDs (`%0`, `%1`, `%2`, ...).

## 8. Instructions

### 8.1 Constants

```text
%r = const <type> <literal>
```

Integer literals are decimal unsigned integers. For signed values, the bit
pattern is encoded using the unsigned representation (e.g. `-1` as `i32` is
written as `4294967295`).

`f64` constants are written as 16-digit hexadecimal bit patterns prefixed with
`0x` to guarantee exact round-trip.

`i1` constants may be written as `true` or `false`.

### 8.2 Binary Arithmetic and Comparison

```text
%r = add <type> %a, %b
%r = sub <type> %a, %b
%r = mul <type> %a, %b
%r = div <type> %a, %b
%r = rem <type> %a, %b
%r = eq  <type> %a, %b
%r = lt  <type> %a, %b
%r = gt  <type> %a, %b
```

All arithmetic is wrapping. Comparisons produce `i1`.

### 8.3 Memory

```text
%r = alloca <type> [, <count>]
%r = load <type> %addr
store <type> %value, %addr
```

If `<count>` is omitted, it defaults to `1`.

### 8.4 Calls

```text
%r = call <ret-type> @callee(%arg0, %arg1, ...)
call void @callee(%arg0, %arg1, ...)
```

### 8.5 Phi

```text
%r = phi <type> ["<label>", %v], ...
```

Each incoming pair associates a predecessor block label with a value.

### 8.6 GetElementPtr

```text
%r = gep <elem-type> %base, <index>, ...
```

An `<index>` is either a dynamic value `%id` or a struct field index
`field <u32>`.

## 9. Terminators

```text
br "<label>"
cond_br %cond, "<true-label>", "<false-label>"
ret <type> %value
ret void
```

## 10. Debug Locations

Debug locations are optional and do not affect execution semantics. A debug
location annotation may be attached to any instruction:

```text
%0 = add i32 %1, %2 !loc "src.c" 42 10
```

Syntax:

```text
!loc "<file>" <line> [<column>]
```

The column is optional. The reference v0.1 implementation reads and writes debug
locations, preserving them through round-trip. Implementations that do not need
debug information may ignore `!loc` annotations on parse and omit them on write
without changing program semantics.

## 11. Round-Trip Invariants

1. Parameters receive `ValueId`s `0..p-1` in declaration order.
2. Each result-producing instruction receives the next `ValueId` in block order,
   instruction order.
3. The canonical writer always produces dense textual IDs.
4. Parsing then re-serializing a canonical module yields byte-identical text.

## 12. Example

```text
sair 0.1
entry "max"

func @max -> i32 entry "entry" {
  param i32 %0
  param i32 %1

  block "entry":
    %2 = gt i32 %0, %1
    cond_br %2, "then", "else"

  block "then":
    br "merge"

  block "else":
    br "merge"

  block "merge":
    %3 = phi i32 ["then", %0], ["else", %1]
    ret i32 %3
}
```
