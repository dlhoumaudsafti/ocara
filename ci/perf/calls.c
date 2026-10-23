#include <stdio.h>
#include <stdlib.h>
typedef struct { long x; } Pt;
typedef struct { long total; } Acc;
static long dist(Pt *p, long n) { return labs(n - p->x); }
static void add(Acc *a, Pt *p, long n) { a->total += dist(p, n); }
int main(int argc, char **argv) {
    long n = atol(argv[1]); Pt p = {0}; Acc a = {0};
    for (long i = 0; i < n; i++) add(&a, &p, i);
    printf("%ld\n", a.total);
}
