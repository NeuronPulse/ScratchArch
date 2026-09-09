// Feature: float (IEEE-754 f64 arithmetic).
// Real clang IR for double multiply/add plus a narrowing fp->int truncation.
double sq(double x) {
    return x * x;
}

int main(void) {
    // sq(3.5) = 12.25, + 1.25 = 13.5, truncated to 13.
    double a = sq(3.5) + 1.25;
    return (int)a;
}
