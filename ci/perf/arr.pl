use strict; use warnings;
my $n = $ARGV[0]; my @xs;
push @xs, $_ for 0 .. $n - 1;
my $tot = 0;
for my $r (1 .. 10) { for my $i (0 .. $n - 1) { $tot += $xs[$i]; } }
print "$tot\n";
