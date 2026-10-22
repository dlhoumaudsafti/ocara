#include <bits/stdc++.h>
using namespace std;
int main(int, char **argv) {
    long n = atol(argv[1]); long tot = 0;
    for (long i = 0; i < n; i++) {
        string s = "hello world hello"; size_t pos = 0;
        while ((pos = s.find("hello", pos)) != string::npos) { s.replace(pos, 5, "bye"); pos += 3; }
        transform(s.begin(), s.end(), s.begin(), ::toupper);
        tot += s.size();
    }
    cout << tot << "\n";
}
