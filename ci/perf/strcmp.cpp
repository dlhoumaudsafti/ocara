#include <bits/stdc++.h>
using namespace std;
int main(int, char **argv) {
    long n = atol(argv[1]); vector<string> words = { "alpha", "beta", "gamma", "delta" }; long hits = 0;
    for (long i = 0; i < n; i++) for (auto &w : words) if (w == "gamma") hits++;
    cout << hits << "\n";
}
