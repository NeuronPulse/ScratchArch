// Runtime-length memory operations and the SART string builtins.
//
// The translator inlines *constant-length* `llvm.mem*` calls (and the
// three-operand `__scratcharch_memcpy`) into explicit byte sequences, so this
// fixture deliberately computes its lengths at run time: the calls survive
// translation and are resolved by the VM's load-time runtime resolver (the SAIR
// interpreter resolves them through the same runtime registry). `strcmp`,
// `strcpy`, `strncpy`, and `strlen` are runtime functions unconditionally.
//
// The shims take 32-bit lengths so each argument occupies one operand cell,
// matching the SART registry signatures.

void *__scratcharch_memcpy(void *dst, const void *src, unsigned int n);
void *__scratcharch_memmove(void *dst, const void *src, unsigned int n);
void *__scratcharch_memset(void *dst, int c, unsigned int n);
int __scratcharch_memcmp(const void *a, const void *b, unsigned int n);
int __scratcharch_strcmp(const char *a, const char *b);
void *__scratcharch_strcpy(char *dst, const char *src);
void *__scratcharch_strncpy(char *dst, const char *src, unsigned int n);
int __scratcharch_strlen(const char *s);

int main(void) {
    char a[8];
    char b[8];
    int i;
    unsigned int n = 5; // run time, never a literal at the call site
    int score = 0;

    for (i = 0; i < 8; i++) a[i] = (char)(i + 1); // 1..8
    for (i = 0; i < 8; i++) b[i] = 0;

    __scratcharch_memcpy(b, a, n); // b = 1,2,3,4,5,0,0,0
    if (b[0] == 1 && b[4] == 5 && b[5] == 0) score += 1;

    if (__scratcharch_memcmp(a, b, n) == 0) score += 2; // first 5 bytes equal

    __scratcharch_memset(b, 7, n); // b = 7,7,7,7,7,0,0,0
    if (b[0] == 7 && b[4] == 7 && b[5] == 0) score += 4;

    __scratcharch_memmove(b + 1, b, 4); // as-if-through-a-temporary: b1..4 = 7
    if (b[1] == 7 && b[4] == 7) score += 8;

    char s1[4];
    char s2[4];
    s1[0] = 'a'; s1[1] = 'b'; s1[2] = 'c'; s1[3] = 0;
    s2[0] = 'a'; s2[1] = 'b'; s2[2] = 'd'; s2[3] = 0;
    if (__scratcharch_strcmp(s1, s2) < 0) score += 16;

    char d[8];
    __scratcharch_strcpy(d, s1);
    if (d[0] == 'a' && d[2] == 'c' && d[3] == 0) score += 32;
    if (__scratcharch_strlen(d) == 3) score += 64;

    __scratcharch_strncpy(d, s2, 6);
    if (d[0] == 'a' && d[2] == 'd') score += 128;

    return score; // 1 + 2 + 4 + 8 + 16 + 32 + 64 + 128 == 255
}
