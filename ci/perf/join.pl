use strict; use warnings;
my $n = $ARGV[0]; my @xs = qw(a b c d e); my $tot = 0;
$tot += length(join "-", @xs) + length("$_") for 0 .. $n - 1;
print "$tot\n";
