// Exercises LLVM `and`/`or`/`xor`/`shl`/`lshr`/`ashr` through real clang IR at
// several widths. One helper per op keeps each operand a runtime parameter, so
// clang -O0 (-disable-O0-optnone folds nothing across the call boundary) and
// the raw bitwise instruction survives into the IR instead of being folded.
//
// main combines the helper results into a weighted sum. Every weight is a
// distinct power of two and each helper output is masked to its low byte, so a
// single wrong op (or a silent collapse of one op onto another) flips the
// checksum. Widths exercised:
//   band/bor/bxor/shl/lshr/ashr   — i32
//   ashr8                         — i8  (trunc/ashr i8/sext)
//   and64/xor64/shl64/ashr64      — i64 (two-limb on the VM backend)
static int band(int a, int b) { return a & b; }
static int bor(int a, int b) { return a | b; }
static int bxor(int a, int b) { return a ^ b; }
static int shl32(int a, int b) { return a << b; }
static int lshr32(int a, int b) { return (unsigned)a >> b; }
static int ashr32(int a, int b) { return a >> b; }

static int ashr8(int a, int b) {
    signed char x = (signed char)a;
    return (int)((signed char)(x >> b)) & 0xFF;
}

static long and64(long a, long b) { return a & b; }
static long xor64(long a, long b) { return a ^ b; }
static long shl64(long a, long b) { return a << b; }
static long ashr64(long a, long b) { return a >> b; }

static int b8(int v) { return v & 0xFF; }

int main(void) {
    int s = 0;
    s += band(0x0F, 0xF3) * 1;          // 0x03
    s += bor(0x0F, 0x30) * 2;           // 0x3F -> 126
    s += bxor(0x0F, 0x33) * 4;          // 0x3C -> 240
    s += shl32(0x01, 5) * 8;            // 32 -> 256
    s += lshr32(0xF0, 4) * 16;          // 15 -> 240
    s += ashr32(-8, 1) * 32;            // -4 (masked to 0xFC=252) -> 8064
    s += ashr8(-0x40, 4) * 64;          // -4 (masked 0xFC) -> 16128
    s += b8((int)(and64(0x123456789ABCDEF0L, 0xFFFFFFFF00000000L) >> 32)) * 128; // 0x12345678 &0xFF = 0x78
    s += b8((int)(xor64(0xFFFFFFFFFFFFFFFFL, 0x00FF00FF00FF00FFL) >> 32)) * 256; // 0xFF00FF00 low byte 0x00
    s += b8((int)(shl64(0x0000000000000001L, 40))) * 512; // 1<<40 -> low byte 0x00
    s += b8((int)(ashr64(0xFFFFFFFF00000000L, 32))) * 1024; // 0xFFFFFFFF -> low byte 0xFF=255 -> 261120
    return s;
}
