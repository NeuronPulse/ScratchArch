// Feature: mixed-width (sub-word) struct fields with real packing/padding.
// char + int + short exercise the natural-alignment layout offsets (0, 4, 8),
// width-exact i8/i16 loads, and a sign-extending short read. Expected: return 7988.
struct Record {
    char tag;   /* offset 0, then 3 bytes padding */
    int num;    /* offset 4 */
    short code; /* offset 8 */
};

int main(void) {
    struct Record r;
    r.tag = 7;
    r.num = 1000;
    r.code = -12;
    return (int)r.tag * 1000 + (int)r.code + r.num; /* 7000 - 12 + 1000 */
}
