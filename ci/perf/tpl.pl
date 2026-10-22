use strict; use warnings;
my $n = $ARGV[0]; my $tot = 0; my $name = "ocara";
for my $i (0 .. $n - 1) { my $s = "id=$i name=$name ok"; $tot += length $s; }
print "$tot\n";
