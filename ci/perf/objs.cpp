#include <bits/stdc++.h>
using namespace std;
struct Point { long x, y; long dist(const Point &o) const { return (x - o.x) * (x - o.x) + (y - o.y) * (y - o.y); } };
int main(int, char **argv) {
    long n = atol(argv[1]); vector<unique_ptr<Point>> pts;
    for (long i = 0; i < n; i++) pts.push_back(make_unique<Point>(Point{ i, i * 2 }));
    long tot = 0;
    for (long i = 1; i < n; i++) tot += pts[i]->dist(*pts[i - 1]);
    cout << tot << "\n";
}
