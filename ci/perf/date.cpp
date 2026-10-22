#include <bits/stdc++.h>
using namespace std;
int main(int, char **argv) {
    long n = atol(argv[1]); long tot = 0;
    for (long i = 0; i < n; i++) {
        time_t t = 1700000000 + i * 37; tm parts; gmtime_r(&t, &parts);
        ostringstream s; s << put_time(&parts, "%d/%m/%Y %H:%M:%S"); tot += s.str().size();
    }
    cout << tot << "\n";
}
