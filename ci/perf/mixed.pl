use strict; use warnings;
my $n = $ARGV[0]; my $t = 0;
for my $i (0 .. $n - 1) { my $x = $i; $t = $t + $x * 2; }
print "$t\n";
