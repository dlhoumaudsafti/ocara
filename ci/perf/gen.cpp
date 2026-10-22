#include <bits/stdc++.h>
using namespace std;
// Générateur : itérateur à état (pas de std::generator en C++17/20 avec GCC 13).
struct Numbers {
    long i = 0, n;
    explicit Numbers(long n) : n(n) {}
    optional<long> next() { return i < n ? optional<long>(i++) : nullopt; }
};
int main(int, char **argv) {
    long n = atol(argv[1]); long t = 0; Numbers it(n);
    while (auto v = it.next()) t += *v;
    cout << t << "\n";
}
