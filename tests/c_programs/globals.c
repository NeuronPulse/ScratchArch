// Globals corpus fixture: module-level global variables exercised through every
// access shape — read/modify/write of a mutable global, a pointer relocation to
// a global, constant-indexed global-array elements (inline getelementptr
// constant expressions), an i8 scalar with a negative value, a byte string, and
// a 64-bit negative constant. The interpreter result is compared against the
// native exit code (95) in the pipeline and fresh-clang corpus tests; the VM
// backend rejects the module (sub-word/byte data is interpreter-exact only).
int counter = 41;
const char *greeting = "hi";
static int table[3] = {7, 8, 9};
static unsigned char byte_val = 200;
static char msg[16] = "ok";
static long long big = -4294967297LL;

int read_and_bump(void) { return counter++; }

int init_only(void) { return counter + table[2]; }

int main(void) {
    int base = init_only();
    int b = read_and_bump();
    int s = greeting[1] == 'i' ? 1 : 0;
    int u = byte_val == 200 ? 1 : 0;
    int m = msg[1] == 'k' ? 1 : 0;
    int h = big == -4294967297LL ? 1 : 0;
    return base + b + s + u + m + h;
}
