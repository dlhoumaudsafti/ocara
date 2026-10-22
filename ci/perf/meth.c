#include <stdio.h>
#include <stdlib.h>
typedef struct { long total; } Counter;
static long weight(Counter *c, long n) { (void)c; return n * 2; }
static void add(Counter *c, long n) { c->total += weight(c, n); }
int main(int argc, char **argv) {
    long n = atol(argv[1]); Counter c = { 0 };
    for (long i = 0; i < n; i++) add(&c, i);
    printf("%ld\n", c.total); return 0;
}
