#include <bits/stdc++.h>
using namespace std;
static long fib(long n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
int main(int, char **argv) {
    long n = atol(argv[1]); double acc = 0.0;
    for (long i = 0; i < n; i++) acc += (i % 7) * 0.5;
    cout << fib(30) << " " << acc << "\n";
}
