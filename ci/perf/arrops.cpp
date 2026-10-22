#include <bits/stdc++.h>
using namespace std;
int main(int, char **argv) {
    long n = atol(argv[1]); vector<long> xs(1000); iota(xs.begin(), xs.end(), 0); long tot = 0;
    for (long i = 0; i < n; i++) {
        if (find(xs.begin(), xs.end(), i % 1500) != xs.end()) tot++;
        tot += find(xs.begin(), xs.end(), i % 1000) - xs.begin();
        vector<long> part(xs.begin() + i % 900, xs.begin() + i % 900 + 50);
        reverse(part.begin(), part.end()); tot += part[0];
        xs.push_back(i); tot += xs.back(); xs.pop_back();
    }
    cout << tot << "\n";
}
