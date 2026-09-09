// Feature: atomic (C11 / clang `__atomic_*`, lowers to `atomicrmw`/atomic
// load/store at -O0), a clean front-end capability probe.
int main(void) {
    static int counter = 10;
    __atomic_fetch_add(&counter, 5, __ATOMIC_SEQ_CST);
    return counter;
}
