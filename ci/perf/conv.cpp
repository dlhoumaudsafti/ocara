#include <bits/stdc++.h>
using namespace std;
int main(int, char **argv) {
    long n = atol(argv[1]); long ti = 0; double tf = 0.0;
    for (long i = 0; i < n; i++) { ti += stol(to_string(i)); tf += stod(to_string(i) + ".5"); }
    cout << ti << " " << tf << "\n";
}
