// Exercises signed integer division/remainder semantics on negative operands
// through real clang IR: sdiv truncates toward zero, srem takes the dividend's
// sign. (Signed *comparisons* on negatives are a documented gap; this fixture
// uses only division/remainder.)
int main(void) {
    int a = -8, b = 3;
    int q = a / b;   // -2
    int r = a % b;   // -2
    int u = 8 / -3;  // -2
    return q * 100 + r * 10 + u + 300;
    // -200 - 20 - 2 + 300 = 78
}
