#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(int argc, char **argv) {
    long n = atol(argv[1]); const char *xs[] = { "a", "b", "c", "d", "e" }; long tot = 0; char num[24];
    for (long i = 0; i < n; i++) {
        char *joined = malloc(16); joined[0] = 0;
        for (int k = 0; k < 5; k++) { if (k) strcat(joined, "-"); strcat(joined, xs[k]); }
        tot += strlen(joined) + snprintf(num, sizeof num, "%ld", i);
        free(joined);
    }
    printf("%ld\n", tot); return 0;
}
