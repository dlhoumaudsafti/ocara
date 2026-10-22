use strict; use warnings;
my $n = $ARGV[0]; my $line = "alpha,beta,gamma,delta,epsilon,zeta"; my $tot = 0;
for my $i (0 .. $n - 1) { my @p = split /,/, $line, -1; $tot += @p; }
print "$tot\n";
