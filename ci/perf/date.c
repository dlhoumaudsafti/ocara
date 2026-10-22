#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
int main(int argc, char **argv) {
    long n = atol(argv[1]); long tot = 0; char s[64]; struct tm tm;
    for (long i = 0; i < n; i++) {
        time_t t = 1700000000 + i * 37; gmtime_r(&t, &tm);
        tot += strftime(s, sizeof s, "%d/%m/%Y %H:%M:%S", &tm);
    }
    printf("%ld\n", tot); return 0;
}
