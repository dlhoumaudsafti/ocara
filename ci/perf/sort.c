#include <stdio.h>
#include <stdlib.h>
static int cmp(const void *a, const void *b) { long x = *(const long *)a, y = *(const long *)b; return (x > y) - (x < y); }
int main(int argc, char **argv) {
    long n = atol(argv[1]); long *xs = malloc(n * sizeof *xs);
    for (long i = 0; i < n; i++) xs[i] = (i * 7919) % 100003;
    qsort(xs, n, sizeof *xs, cmp);
    printf("%ld %ld\n", xs[0], xs[n - 1]); free(xs); return 0;
}
