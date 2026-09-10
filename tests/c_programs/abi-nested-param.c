// Feature: a nested struct passed by value as a parameter.
//
// `struct Wide` contains two `struct Pair`s, so the argument's field offsets come
// from the layout of an aggregate-of-aggregates. It is 20 bytes (MEMORY class),
// hence a `byval` pointer: `wide_mutate` writes through it, and the caller's
// record must still read back as `w.p.a == 1` afterwards.
//
// Expected: return 125.
struct Pair {
    int a;
    int b;
};

struct Wide {
    struct Pair p;
    struct Pair q;
    int k;
};

static int wide_sum(struct Wide w) {
    return w.p.a + w.p.b + w.q.a + w.q.b + w.k;
}

static int wide_mutate(struct Wide w) {
    w.p.a = 100; /* must not be observable by the caller */
    return w.p.a + w.q.b + w.k;
}

int main(void) {
    struct Wide w = {{1, 2}, {3, 4}, 5};
    int a = wide_sum(w);    /* 1 + 2 + 3 + 4 + 5  */
    int b = wide_mutate(w); /* 100 + 4 + 5        */
    return a + b + w.p.a;   /* 15 + 109 + 1       */
}
