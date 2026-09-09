// Feature: nested structs (aggregate-of-aggregate field access).
// Nested aggregates lower to flattened byte offsets via the natural-alignment
// layout; every load/store here is a whole i32, so the program runs identically
// on the interpreter, the VM, and (as constructible Scratch) the Scratch model.
// Expected: return 156.
struct Point {
    int x;
    int y;
};

struct Rect {
    struct Point tl; /* top-left  */
    struct Point br; /* bottom-right */
    int label;
};

int main(void) {
    struct Rect r;
    r.tl.x = 3;
    r.tl.y = 4;
    r.br.x = 10;
    r.br.y = 12;
    r.label = 100;
    /* area (7*8) + label (100) */
    return (r.br.x - r.tl.x) * (r.br.y - r.tl.y) + r.label;
}
