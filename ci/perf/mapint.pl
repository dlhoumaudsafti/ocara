use strict; use warnings;
my $n = $ARGV[0]; my %m;
$m{$_} = $_ * 2 for 0 .. $n - 1;
my $t = 0;
$t += $m{$_} for 0 .. $n - 1;
print "$t\n";
