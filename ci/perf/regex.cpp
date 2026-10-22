#include <bits/stdc++.h>
using namespace std;
int main(int, char **argv) {
    long n = atol(argv[1]); long hits = 0, out = 0;
    regex mail("^[a-z]+[0-9]+@example\\.com"), digits("[0-9]+");
    for (long i = 0; i < n; i++) {
        string line = "user" + to_string(i) + "@example.com id=" + to_string(i);
        if (regex_search(line, mail)) hits++;
        out += regex_replace(line, digits, "#", regex_constants::format_first_only).size();
    }
    cout << hits << " " << out << "\n";
}
