use strict; use warnings;
exit 1 unless eval { require DBI; require DBD::SQLite; 1 };
my $n = $ARGV[0]; my $db = DBI->connect("dbi:SQLite:dbname=:memory:", "", "", { RaiseError => 1, AutoCommit => 1 });
$db->do("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT, v INTEGER)");
$db->begin_work;
my $ins = $db->prepare("INSERT INTO t (name, v) VALUES (?, ?)");
$ins->execute("n$_", $_) for 0 .. $n - 1;
$db->commit;
my $sel = $db->prepare("SELECT v FROM t WHERE id = ?"); my $tot = 0;
for my $i (0 .. int($n / 10) - 1) { $sel->execute($i + 1); $tot += ($sel->fetchrow_array)[0]; }
print "$tot\n";
