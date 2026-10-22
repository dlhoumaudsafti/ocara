// perf-libs: -lpthread
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
typedef struct { long x; } Point;
static void *noop(void *arg) { return arg; }
int main(int argc, char **argv) {
    long n = atol(argv[1]); pthread_t t;
    pthread_create(&t, NULL, noop, NULL); pthread_join(t, NULL);
    Point **pts = malloc(n * sizeof *pts);
    for (long i = 0; i < n; i++) { pts[i] = malloc(sizeof(Point)); pts[i]->x = i; }
    long tot = 0;
    for (long i = 0; i < n; i++) tot += pts[i]->x;
    printf("%ld\n", tot);
    for (long i = 0; i < n; i++) free(pts[i]);
    free(pts); return 0;
}
