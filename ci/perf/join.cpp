#include <bits/stdc++.h>
using namespace std;
int main(int, char **argv) {
    long n = atol(argv[1]); vector<string> xs = { "a", "b", "c", "d", "e" }; long tot = 0;
    for (long i = 0; i < n; i++) {
        string joined; for (size_t k = 0; k < xs.size(); k++) { if (k) joined += "-"; joined += xs[k]; }
        tot += joined.size() + to_string(i).size();
    }
    cout << tot << "\n";
}
