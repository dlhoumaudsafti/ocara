#include <bits/stdc++.h>
using namespace std;
int main(int, char **argv) {
    long n = atol(argv[1]); string line = "alpha,beta,gamma,delta,epsilon,zeta"; long tot = 0;
    for (long i = 0; i < n; i++) {
        vector<string> parts; stringstream ss(line); string part;
        while (getline(ss, part, ',')) parts.push_back(part);
        tot += parts.size();
    }
    cout << tot << "\n";
}
