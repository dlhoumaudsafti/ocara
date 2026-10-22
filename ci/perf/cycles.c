#include <stdio.h>
#include <stdlib.h>
typedef struct Node { struct Node *next; long v; } Node;
static long get(const Node *x) { return x->v; }
int main(int argc, char **argv) {
    long n = atol(argv[1]); Node **nodes = malloc(n * sizeof *nodes); Node *prev = NULL;
    for (long i = 0; i < n; i++) { Node *x = malloc(sizeof *x); x->v = i; x->next = prev; nodes[i] = x; prev = x; }
    long tot = 0;
    for (int r = 0; r < 5; r++) for (long i = 0; i < n; i++) tot += get(nodes[i]);
    printf("%ld\n", tot);
    for (long i = 0; i < n; i++) free(nodes[i]);
    free(nodes); return 0;
}
