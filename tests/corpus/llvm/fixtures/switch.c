// Feature: switch (LLVM `switch` terminator).
// Multiple cases plus a default: the interpreter and VM lower `switch` via
// edge copies, so a full multi-way jump is exercised.
int classify(int x) {
    switch (x) {
        case 1: return 10;
        case 2: return 20;
        case 3: return 30;
        default: return 0;
    }
}

int main(void) {
    // 20 + 10 + 0
    return classify(2) + classify(1) + classify(9);
}
