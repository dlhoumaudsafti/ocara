#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(int argc, char **argv) {
    long n = atol(argv[1]); const char *words[] = { "alpha", "beta", "gamma", "delta" }; long hits = 0;
    for (long i = 0; i < n; i++) for (int k = 0; k < 4; k++) if (strcmp(words[k], "gamma") == 0) hits++;
    printf("%ld\n", hits); return 0;
}
