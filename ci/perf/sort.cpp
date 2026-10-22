#include <bits/stdc++.h>
using namespace std;
int main(int, char **argv) {
    long n = atol(argv[1]); vector<long> xs(n);
    for (long i = 0; i < n; i++) xs[i] = (i * 7919) % 100003;
    sort(xs.begin(), xs.end());
    cout << xs[0] << " " << xs[n - 1] << "\n";
}
