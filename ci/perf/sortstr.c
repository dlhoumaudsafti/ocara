#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static int cmp(const void *a, const void *b) { return strcmp(*(char *const *)a, *(char *const *)b); }
int main(int argc, char **argv) {
    long n = atol(argv[1]); char **xs = malloc(n * sizeof *xs); char s[32];
    for (long i = 0; i < n; i++) { snprintf(s, sizeof s, "k%ld", (i * 7919) % 100003); xs[i] = strdup(s); }
    qsort(xs, n, sizeof *xs, cmp);
    printf("%s %s\n", xs[0], xs[n - 1]);
    for (long i = 0; i < n; i++) free(xs[i]);
    free(xs); return 0;
}
