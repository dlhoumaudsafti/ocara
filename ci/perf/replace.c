#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static char *replace(const char *s, const char *from, const char *to) {
    size_t fl = strlen(from), tl = strlen(to), count = 0;
    for (const char *p = s; (p = strstr(p, from)); p += fl) count++;
    char *out = malloc(strlen(s) + count * (tl - fl) + 1), *o = out;
    for (const char *p = s, *q; (q = strstr(p, from)); p = q + fl) {
        memcpy(o, p, q - p); o += q - p; memcpy(o, to, tl); o += tl; s = q + fl;
    }
    strcpy(o, s);
    return out;
}
int main(int argc, char **argv) {
    long n = atol(argv[1]); long tot = 0;
    for (long i = 0; i < n; i++) {
        char *r = replace("hello world hello", "hello", "bye");
        for (char *p = r; *p; p++) *p = toupper((unsigned char)*p);
        tot += strlen(r); free(r);
    }
    printf("%ld\n", tot); return 0;
}
