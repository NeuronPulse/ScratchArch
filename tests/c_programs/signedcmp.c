// Exercises exact signed icmp (Part 2) on negatives and mixed signs through real
// clang IR. Each helper performs one signed predicate on i32 params (runtime
// operands, so clang cannot fold the icmp away); main combines the 0/1 results
// arithmetically into a checksum so any predicate that degrades to the unsigned
// bit-pattern compare flips the answer.
//
// Predicate truth table over the chosen operand pairs:
//   lt(-5, 1)=1  lt(-5,-1)=1  lt(-1,-5)=0  le(-5,-5)=1
//   gt(1,-5)=1   ge(-1,-5)=1  ge(-5,-1)=0  gt(-3,2)=0
// Checksum = 1 + 2 + 8 + 16 + 32 = 59
static int lt(int a, int b) { return a < b; }
static int le(int a, int b) { return a <= b; }
static int gt(int a, int b) { return a > b; }
static int ge(int a, int b) { return a >= b; }

int main(void) {
  return lt(-5, 1) + lt(-5, -1) * 2 + lt(-1, -5) * 4 + le(-5, -5) * 8
       + gt(1, -5) * 16 + ge(-1, -5) * 32 + ge(-5, -1) * 64 + gt(-3, 2) * 128;
}
