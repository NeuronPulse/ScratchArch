// Feature (BOUNDARY): an aggregate whose fields total a non-power-of-two byte
// width.
//
// Three `char`s make a 3-byte record, which the x86-64 ABI passes and returns in a
// single register coerced to `i24`. SAIR represents i1/i8/i16/i32/i64 only, so
// there is no faithful lowering: the translator must reject this with a named
// diagnostic rather than silently widening to i32 (which would change the value's
// width) or flattening it. See docs/design/AGGREGATE_ABI.md §Boundaries and
// docs/specification/LLVM_COMPATIBILITY.md.
struct Odd {
    char a;
    char b;
    char c;
};

static int odd_sum(struct Odd o) {
    return o.a + o.b + o.c;
}

int main(void) {
    struct Odd o = {1, 2, 3};
    return odd_sum(o); /* would be 6; rejected instead */
}
