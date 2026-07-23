void *__scratcharch_memcpy(void *dest, const void *src, unsigned int n);

int main() {
    int src[3];
    int dst[3];
    src[0] = 1;
    src[1] = 2;
    src[2] = 3;
    __scratcharch_memcpy(dst, src, 12);
    return dst[0] + dst[1] + dst[2];
}
