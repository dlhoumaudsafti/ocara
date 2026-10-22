#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
int main(int argc, char **argv) {
    long n = atol(argv[1]); char path[64]; snprintf(path, sizeof path, "/tmp/ocara_perf_file_c_%d.txt", getpid());
    size_t len = 0, cap = 1024; char *text = malloc(cap); char num[48];
    for (long i = 0; i < n; i++) {
        int k = snprintf(num, sizeof num, "ligne %ld\n", i);
        if (len + k + 1 > cap) { cap *= 2; text = realloc(text, cap); }
        memcpy(text + len, num, k); len += k;
    }
    FILE *f = fopen(path, "w"); fwrite(text, 1, len, f); fclose(f); free(text);
    f = fopen(path, "r"); fseek(f, 0, SEEK_END); long size = ftell(f); rewind(f);
    char *all = malloc(size + 1); fread(all, 1, size, f); all[size] = 0; fclose(f);
    long count = 0, tot = 0; char *start = all;
    for (;;) {
        char *nl = strchr(start, '\n'); size_t l = nl ? (size_t)(nl - start) : strlen(start);
        char *part = strndup(start, l); tot += strlen(part); free(part); count++;
        if (!nl) break;
        start = nl + 1;
    }
    free(all); remove(path);
    printf("%ld %ld\n", count, tot); return 0;
}
