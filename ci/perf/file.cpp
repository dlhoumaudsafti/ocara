#include <bits/stdc++.h>
#include <unistd.h>
using namespace std;
int main(int, char **argv) {
    long n = atol(argv[1]); string path = "/tmp/ocara_perf_file_cpp_" + to_string(getpid()) + ".txt"; string text;
    for (long i = 0; i < n; i++) text += "ligne " + to_string(i) + "\n";
    { ofstream f(path); f << text; }
    string all; { ifstream f(path); stringstream ss; ss << f.rdbuf(); all = ss.str(); }
    vector<string> lines; size_t start = 0;
    for (;;) { size_t nl = all.find('\n', start); lines.push_back(all.substr(start, nl == string::npos ? string::npos : nl - start)); if (nl == string::npos) break; start = nl + 1; }
    long tot = 0; for (auto &l : lines) tot += l.size();
    remove(path.c_str());
    cout << lines.size() << " " << tot << "\n";
}
