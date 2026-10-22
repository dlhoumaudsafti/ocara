#include <bits/stdc++.h>
using namespace std;
struct AppError : runtime_error { long code; AppError(const string &m, long c) : runtime_error(m), code(c) {} };
int main(int, char **argv) {
    long n = atol(argv[1]); long c = 0;
    for (long i = 0; i < n; i++) { try { throw AppError("x", i); } catch (const AppError &) { c++; } }
    cout << c << "\n";
}
