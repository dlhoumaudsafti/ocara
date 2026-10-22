#include <stdio.h>
#include <stdlib.h>
#include <string.h>
// contains / indexOf / slice / reverse / push / pop écrits à la main : le C n'en a pas.
int main(int argc, char **argv) {
    long n = atol(argv[1]); long len = 1000, cap = 1024; long *xs = malloc(cap * sizeof *xs); long tot = 0;
    for (long i = 0; i < len; i++) xs[i] = i;
    for (long i = 0; i < n; i++) {
        long v = i % 1500;
        for (long k = 0; k < len; k++) if (xs[k] == v) { tot++; break; }
        long w = i % 1000;
        for (long k = 0; k < len; k++) if (xs[k] == w) { tot += k; break; }
        long from = i % 900; long *part = malloc(50 * sizeof *part); memcpy(part, xs + from, 50 * sizeof *part);
        long *rev = malloc(50 * sizeof *rev); for (int k = 0; k < 50; k++) rev[k] = part[49 - k];
        tot += rev[0]; free(part); free(rev);
        if (len == cap) { cap *= 2; xs = realloc(xs, cap * sizeof *xs); }
        xs[len++] = i; tot += xs[--len];
    }
    printf("%ld\n", tot); free(xs); return 0;
}
