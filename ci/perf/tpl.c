#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(int argc, char **argv) {
    long n = atol(argv[1]); long tot = 0; const char *name = "ocara"; char s[64];
    for (long i = 0; i < n; i++) { snprintf(s, sizeof s, "id=%ld name=%s ok", i, name); tot += strlen(s); }
    printf("%ld\n", tot); return 0;
}
