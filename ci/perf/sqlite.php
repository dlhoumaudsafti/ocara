<?php
if (!class_exists('SQLite3')) exit(1);
$n = (int)$argv[1]; $db = new SQLite3(":memory:");
$db->exec("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT, v INTEGER)");
$db->exec("BEGIN");
$ins = $db->prepare("INSERT INTO t (name, v) VALUES (:name, :v)");
for ($i = 0; $i < $n; $i++) {
    $ins->bindValue(":name", "n$i"); $ins->bindValue(":v", $i); $ins->execute(); $ins->reset();
}
$db->exec("COMMIT");
$sel = $db->prepare("SELECT v FROM t WHERE id = :id"); $tot = 0;
for ($i = 0; $i < intdiv($n, 10); $i++) {
    $sel->bindValue(":id", $i + 1); $row = $sel->execute()->fetchArray(SQLITE3_ASSOC); $tot += $row["v"]; $sel->reset();
}
echo $tot, "\n";
