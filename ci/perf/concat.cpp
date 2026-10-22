#include <bits/stdc++.h>
using namespace std;
int main(int, char **argv) {
    long n = atol(argv[1]); string s;
    // `s = s + "x"` recopie la chaîne à chaque tour (quadratique) : forme idiomatique.
    for (long i = 0; i < n; i++) s += "x";
    cout << s.size() << "\n";
}
