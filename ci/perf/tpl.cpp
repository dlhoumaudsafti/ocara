#include <bits/stdc++.h>
using namespace std;
int main(int, char **argv) {
    long n = atol(argv[1]); long tot = 0; string name = "ocara";
    for (long i = 0; i < n; i++) { string s = "id=" + to_string(i) + " name=" + name + " ok"; tot += s.size(); }
    cout << tot << "\n";
}
