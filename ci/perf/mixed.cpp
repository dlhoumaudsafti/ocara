#include <bits/stdc++.h>
using namespace std;
// Valeur dynamique : std::variant<long, double>.
int main(int, char **argv) {
    long n = atol(argv[1]); variant<long, double> t = 0L;
    for (long i = 0; i < n; i++) {
        variant<long, double> x = i;
        t = visit([](auto a, auto b) -> variant<long, double> { return a + b * 2; }, t, x);
    }
    cout << get<long>(t) << "\n";
}
