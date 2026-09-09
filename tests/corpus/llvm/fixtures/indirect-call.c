// Feature: indirect-call (call through a function pointer).
// clang -O0 emits a `call` whose callee is an SSA value of pointer-to-function
// type, not a direct function reference.
static int add1(int x) {
    return x + 1;
}

int main(void) {
    int (*fp)(int) = add1;
    return fp(41);
}
