#include <regex.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(int argc, char **argv) {
    long n = atol(argv[1]); long hits = 0, out = 0; regex_t mail, digits; regmatch_t m; char line[64];
    regcomp(&mail, "^[a-z]+[0-9]+@example\\.com", REG_EXTENDED | REG_NOSUB);
    regcomp(&digits, "[0-9]+", REG_EXTENDED);
    for (long i = 0; i < n; i++) {
        snprintf(line, sizeof line, "user%ld@example.com id=%ld", i, i);
        if (regexec(&mail, line, 0, NULL, 0) == 0) hits++;
        if (regexec(&digits, line, 1, &m, 0) == 0) out += strlen(line) - (m.rm_eo - m.rm_so) + 1;
        else out += strlen(line);
    }
    printf("%ld %ld\n", hits, out); return 0;
}
