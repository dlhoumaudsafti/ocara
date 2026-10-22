#include <bits/stdc++.h>
using namespace std;
struct Node { shared_ptr<Node> next; long v = 0; long get() const { return v; } };
int main(int, char **argv) {
    long n = atol(argv[1]); vector<shared_ptr<Node>> nodes; shared_ptr<Node> prev;
    for (long i = 0; i < n; i++) { auto x = make_shared<Node>(); x->v = i; x->next = prev; nodes.push_back(x); prev = x; }
    long tot = 0;
    for (int r = 0; r < 5; r++) for (auto &x : nodes) tot += x->get();
    cout << tot << "\n";
    for (auto &x : nodes) x->next.reset();
}
