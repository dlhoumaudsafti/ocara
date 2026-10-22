#include <stdio.h>
#include <stdlib.h>
int main(int argc, char **argv) {
    long n = atol(argv[1]); long *xs = malloc(n * sizeof *xs);
    for (long i = 0; i < n; i++) xs[i] = i;
    long tot = 0;
    for (int r = 0; r < 10; r++) for (long i = 0; i < n; i++) tot += xs[i];
    printf("%ld\n", tot); free(xs); return 0;
}
