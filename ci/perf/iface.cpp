#include <bits/stdc++.h>
using namespace std;
struct Weight { virtual long weight(long n) const = 0; virtual ~Weight() = default; };
struct Double : Weight { long weight(long n) const override { return n * 2; } };
struct Triple : Weight { long weight(long n) const override { return n * 3; } };
__attribute__((noinline)) static long total(const Weight &w, long n) { long t = 0; for (long i = 0; i < n; i++) t += w.weight(i); return t; }
int main(int, char **argv) {
    long n = atol(argv[1]); unique_ptr<Weight> d = make_unique<Double>(), t = make_unique<Triple>();
    cout << total(*d, n) + total(*t, n) << "\n";
}
