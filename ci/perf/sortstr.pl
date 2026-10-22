use strict; use warnings;
my $n = $ARGV[0]; my @xs = map { "k" . (($_ * 7919) % 100003) } 0 .. $n - 1;
my @s = sort @xs;
print "$s[0] $s[-1]\n";
