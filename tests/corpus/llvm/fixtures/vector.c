// Feature: vector (LLVM vector types <4 x i32>).
// clang -O0 emits `alloca <4 x i32>` / `store <4 x i32>` / `extractelement`,
// which is a clean front-end capability probe.
typedef int v4 __attribute__((vector_size(16)));

int main(void) {
    v4 a = {1, 2, 3, 4};
    return a[2];
}
