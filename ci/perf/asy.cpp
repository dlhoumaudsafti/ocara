// perf-libs: -lpthread
#include <bits/stdc++.h>
using namespace std;
static long sq(long n) { return n * n; }
int main(int, char **argv) {
    long n = atol(argv[1]); long tot = 0;
    for (long i = 0; i < n; i++) tot += async(launch::async, sq, i).get();
    cout << tot << "\n";
}
