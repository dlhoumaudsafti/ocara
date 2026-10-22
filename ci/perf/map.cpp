#include <bits/stdc++.h>
using namespace std;
int main(int, char **argv) {
    long n = atol(argv[1]); unordered_map<string, long> m;
    for (long i = 0; i < n; i++) m["k" + to_string(i)] = i;
    long tot = 0;
    for (long i = 0; i < n; i++) tot += m["k" + to_string(i)];
    cout << tot << "\n";
}
