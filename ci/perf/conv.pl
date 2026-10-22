use strict; use warnings;
my $n = $ARGV[0]; my ($ti, $tf) = (0, 0.0);
for my $i (0 .. $n - 1) { $ti += int("$i"); $tf += 0 + "$i.5"; }
print "$ti $tf\n";
