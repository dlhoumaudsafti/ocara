#include <stdio.h>
#include <stdlib.h>
typedef struct { long x, y; } Point;
static long dist(const Point *s, const Point *o) { return (s->x - o->x) * (s->x - o->x) + (s->y - o->y) * (s->y - o->y); }
int main(int argc, char **argv) {
    long n = atol(argv[1]); Point **pts = malloc(n * sizeof *pts);
    for (long i = 0; i < n; i++) { pts[i] = malloc(sizeof(Point)); pts[i]->x = i; pts[i]->y = i * 2; }
    long tot = 0;
    for (long i = 1; i < n; i++) tot += dist(pts[i], pts[i - 1]);
    printf("%ld\n", tot);
    for (long i = 0; i < n; i++) free(pts[i]);
    free(pts); return 0;
}
