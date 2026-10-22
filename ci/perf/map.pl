use strict; use warnings;
my $n = $ARGV[0]; my %m;
$m{"k$_"} = $_ for 0 .. $n - 1;
my $tot = 0;
$tot += $m{"k$_"} for 0 .. $n - 1;
print "$tot\n";
