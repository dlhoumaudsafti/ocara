#include <bits/stdc++.h>
using namespace std;
int main(int, char **argv) {
    long n = atol(argv[1]);
    unordered_map<string, long> m = {{"alpha", 1}, {"beta", 2}, {"gamma", 3}, {"delta", 4}, {"epsilon", 5}, {"zeta", 6}, {"eta", 7}, {"theta", 8}};
    long tot = 0;
    for (long i = 0; i < n; i++) {
        tot += m["alpha"] + m["delta"] + m["theta"];
        m["beta"] = i;
        m["zeta"] += 1;
    }
    cout << tot + m["beta"] + m["zeta"] << "\n";
}
