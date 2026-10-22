#include <bits/stdc++.h>
using namespace std;
int main(int, char **argv) {
    long n = atol(argv[1]); unordered_map<long, long> m;
    for (long i = 0; i < n; i++) m[i] = i * 2;
    long t = 0;
    for (long i = 0; i < n; i++) t += m[i];
    cout << t << "\n";
}
