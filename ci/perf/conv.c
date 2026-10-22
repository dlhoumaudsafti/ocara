#include <stdio.h>
#include <stdlib.h>
int main(int argc, char **argv) {
    long n = atol(argv[1]); long ti = 0; double tf = 0.0; char s[48];
    for (long i = 0; i < n; i++) {
        snprintf(s, sizeof s, "%ld", i); ti += strtol(s, NULL, 10);
        snprintf(s, sizeof s, "%ld.5", i); tf += strtod(s, NULL);
    }
    printf("%ld %g\n", ti, tf); return 0;
}
