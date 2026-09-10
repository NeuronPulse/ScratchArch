// Feature: scalar and aggregate parameters interleaved in one signature.
//
// Scalar leaves occupy ordinary cells and each aggregate keeps its own slot, in
// declaration order, whether the aggregate comes first or last. Interleaving must
// reorder neither kind.
//
// Expected: return 87.
struct Pair {
    int a;
    int b;
};

static int mix_scalar_first(int s, struct Pair p) {
    return s + p.a * 10 + p.b;
}

static int mix_aggregate_first(struct Pair p, int s) {
    return p.a * 10 + p.b + s;
}

int main(void) {
    struct Pair p = {4, 2};
    return mix_scalar_first(1, p) + mix_aggregate_first(p, 2); /* 43 + 44 */
}
