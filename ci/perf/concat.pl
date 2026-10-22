use strict; use warnings;
my $n = $ARGV[0]; my $s = "";
for my $i (0 .. $n - 1) { $s = $s . "x"; }
print length($s), "\n";
