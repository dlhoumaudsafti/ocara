// perf-libs: -lpthread
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
int main(int argc, char **argv) {
    long n = atol(argv[1]); pthread_mutex_t m = PTHREAD_MUTEX_INITIALIZER; long c = 0;
    for (long i = 0; i < n; i++) { pthread_mutex_lock(&m); c++; pthread_mutex_unlock(&m); }
    printf("%ld\n", c); return 0;
}
