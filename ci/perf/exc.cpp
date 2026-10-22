#include <bits/stdc++.h>
using namespace std;
int main(int, char **argv) {
    long n = atol(argv[1]); long c = 0;
    for (long i = 0; i < n; i++) { try { throw runtime_error("x"); } catch (const exception &) { c++; } }
    cout << c << "\n";
}
