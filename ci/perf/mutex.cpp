// perf-libs: -lpthread
#include <bits/stdc++.h>
using namespace std;
int main(int, char **argv) {
    long n = atol(argv[1]); mutex m; long c = 0;
    for (long i = 0; i < n; i++) { m.lock(); c++; m.unlock(); }
    cout << c << "\n";
}
