#include <stdio.h>
#include <stdlib.h>
#include <string.h>
// Chaque morceau est copié, comme les chaînes des autres langages.
int main(int argc, char **argv) {
    long n = atol(argv[1]); const char *line = "alpha,beta,gamma,delta,epsilon,zeta"; long tot = 0;
    for (long i = 0; i < n; i++) {
        char *parts[16]; int count = 0; const char *start = line;
        for (;;) {
            const char *comma = strchr(start, ',');
            size_t len = comma ? (size_t)(comma - start) : strlen(start);
            parts[count++] = strndup(start, len);
            if (!comma) break;
            start = comma + 1;
        }
        tot += count;
        for (int k = 0; k < count; k++) free(parts[k]);
    }
    printf("%ld\n", tot); return 0;
}
