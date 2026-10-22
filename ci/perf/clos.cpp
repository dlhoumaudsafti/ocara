#include <bits/stdc++.h>
using namespace std;
int main(int, char **argv) {
    long n = atol(argv[1]); long count = 0;
    function<void(long)> add = [&count](long k) { count += k; };
    for (long i = 0; i < n; i++) add(i % 3);
    cout << count << "\n";
}
