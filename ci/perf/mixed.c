#include <stdio.h>
#include <stdlib.h>
// Typage statique : la même arithmétique sur des entiers.
int main(int argc, char **argv) {
    long n = atol(argv[1]); long t = 0;
    for (long i = 0; i < n; i++) { long x = i; t = t + x * 2; }
    printf("%ld\n", t); return 0;
}
