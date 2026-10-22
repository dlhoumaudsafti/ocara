#include <stdio.h>
#include <stdlib.h>
// Interface : table de pointeurs de fonctions.
typedef struct { long (*weight)(long); } Weight;
static long dbl(long n) { return n * 2; }
static long tpl(long n) { return n * 3; }
static long total(const Weight *w, long n) { long t = 0; for (long i = 0; i < n; i++) t += w->weight(i); return t; }
int main(int argc, char **argv) {
    long n = atol(argv[1]); Weight d = { dbl }, t = { tpl };
    printf("%ld\n", total(&d, n) + total(&t, n)); return 0;
}
