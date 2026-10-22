#include <bits/stdc++.h>
using namespace std;
struct Counter {
    long total = 0;
    void add(long n) { total += weight(n); }
    long weight(long n) { return n * 2; }
};
int main(int, char **argv) {
    long n = atol(argv[1]); Counter c;
    for (long i = 0; i < n; i++) c.add(i);
    cout << c.total << "\n";
}
