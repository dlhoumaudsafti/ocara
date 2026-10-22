use strict; use warnings;
my $n = $ARGV[0]; my %m = map { ("k$_" => $_) } 0 .. 999; my $tot = 0;
for my $r (1 .. $n) { while (my ($k, $v) = each %m) { $tot += $v; } }
print "$tot\n";
