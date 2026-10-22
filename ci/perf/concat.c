#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(int argc, char **argv) {
    long n = atol(argv[1]); size_t len = 0, cap = 16; char *s = malloc(cap); s[0] = 0;
    for (long i = 0; i < n; i++) {
        if (len + 2 > cap) { cap *= 2; s = realloc(s, cap); }
        s[len++] = 'x'; s[len] = 0;
    }
    printf("%zu\n", strlen(s)); free(s); return 0;
}
