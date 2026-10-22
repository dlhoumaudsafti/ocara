// perf-libs: -lsqlite3
#include <sqlite3.h>
#include <stdio.h>
#include <stdlib.h>
int main(int argc, char **argv) {
    long n = atol(argv[1]); sqlite3 *db; sqlite3_stmt *ins, *sel; char name[32]; long tot = 0;
    sqlite3_open(":memory:", &db);
    sqlite3_exec(db, "CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT, v INTEGER)", NULL, NULL, NULL);
    sqlite3_exec(db, "BEGIN", NULL, NULL, NULL);
    sqlite3_prepare_v2(db, "INSERT INTO t (name, v) VALUES (:name, :v)", -1, &ins, NULL);
    for (long i = 0; i < n; i++) {
        snprintf(name, sizeof name, "n%ld", i);
        sqlite3_bind_text(ins, 1, name, -1, SQLITE_TRANSIENT); sqlite3_bind_int64(ins, 2, i);
        sqlite3_step(ins); sqlite3_reset(ins);
    }
    sqlite3_exec(db, "COMMIT", NULL, NULL, NULL);
    sqlite3_prepare_v2(db, "SELECT v FROM t WHERE id = :id", -1, &sel, NULL);
    for (long i = 0; i < n / 10; i++) {
        sqlite3_bind_int64(sel, 1, i + 1);
        if (sqlite3_step(sel) == SQLITE_ROW) tot += sqlite3_column_int64(sel, 0);
        sqlite3_reset(sel);
    }
    sqlite3_finalize(ins); sqlite3_finalize(sel); sqlite3_close(db);
    printf("%ld\n", tot); return 0;
}
