#include <bits/stdc++.h>
using namespace std;
int main(int, char **argv) {
    long n = atol(argv[1]); vector<long> xs;
    for (long i = 0; i < n; i++) xs.push_back(i);
    long tot = 0;
    for (int r = 0; r < 10; r++) for (long i = 0; i < n; i++) tot += xs[i];
    cout << tot << "\n";
}
