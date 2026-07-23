int __scratcharch_strlen(const char *s);

int main() {
    char s[6];
    s[0] = 'h';
    s[1] = 'e';
    s[2] = 'l';
    s[3] = 'l';
    s[4] = 'o';
    s[5] = 0;
    return __scratcharch_strlen(s);
}
