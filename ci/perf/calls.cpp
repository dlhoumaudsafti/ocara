#include <cstdio>
#include <cstdlib>
#include <memory>
struct Pt { long x = 0; long dist(long n) const { return std::labs(n - x); } };
struct Acc { long total = 0; void add(const Pt &p, long n) { total += p.dist(n); } };
int main(int, char **argv) {
    long n = std::atol(argv[1]);
    auto p = std::make_shared<Pt>(); auto a = std::make_shared<Acc>();
    for (long i = 0; i < n; i++) a->add(*p, i);
    std::printf("%ld\n", a->total);
}
