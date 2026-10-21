import sys, sqlite3
n=int(sys.argv[1]); db=sqlite3.connect(":memory:"); db.row_factory=sqlite3.Row
db.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT, v INTEGER)")
with db:
    for i in range(n): db.execute("INSERT INTO t (name, v) VALUES (:name, :v)", {"name": f"n{i}", "v": i})
tot=0
for i in range(n//10):
    row=db.execute("SELECT v FROM t WHERE id = :id", {"id": i+1}).fetchone()
    tot+=row["v"]
print(tot)
