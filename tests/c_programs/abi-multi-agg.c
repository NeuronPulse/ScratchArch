// Feature: more than one aggregate parameter in a single call.
//
// Each aggregate keeps its own slot in the positional parameter list, including
// the `byval` copies the callee makes, so a call with several records must not let
// one clobber another.
//
// Expected: return 119.
struct Pair {
    int a;
    int b;
};

struct Big {
    int a;
    int b;
    int c;
    int d;
    int e;
};

static int combine(struct Big x, struct Big y) {
    return x.a + y.e; /* 1 + 50 */
}

static int pair_and_big(struct Pair p, struct Big b) {
    return p.a * 10 + p.b + b.a; /* 60 + 7 + 1 */
}

int main(void) {
    struct Big x = {1, 2, 3, 4, 5};
    struct Big y = {10, 20, 30, 40, 50};
    struct Pair p = {6, 7};
    return combine(x, y) + pair_and_big(p, x); /* 51 + 68 */
}
