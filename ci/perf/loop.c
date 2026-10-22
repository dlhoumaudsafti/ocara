#include <stdio.h>
#include <stdlib.h>
static long fib(long n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
int main(int argc, char **argv) {
    long n = atol(argv[1]); double acc = 0.0;
    for (long i = 0; i < n; i++) acc += (i % 7) * 0.5;
    printf("%ld %g\n", fib(30), acc); return 0;
}
