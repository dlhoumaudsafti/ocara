// perf-libs: -lpthread
#include <bits/stdc++.h>
using namespace std;
struct Point { long x; };
int main(int, char **argv) {
    long n = atol(argv[1]);
    thread([] {}).join();
    vector<shared_ptr<Point>> pts;
    for (long i = 0; i < n; i++) pts.push_back(make_shared<Point>(Point{ i }));
    long tot = 0;
    for (auto &p : pts) tot += p->x;
    cout << tot << "\n";
}
