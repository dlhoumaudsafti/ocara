#include <bits/stdc++.h>
using namespace std;
int main(int, char **argv) {
    long n = atol(argv[1]); unordered_map<string, long> m;
    for (long i = 0; i < 1000; i++) m["k" + to_string(i)] = i;
    long tot = 0;
    for (long r = 0; r < n; r++) for (auto &[k, v] : m) tot += v;
    cout << tot << "\n";
}
