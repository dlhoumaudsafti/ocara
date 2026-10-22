use strict; use warnings;
my $n = $ARGV[0]; my $c = 0;
for my $i (0 .. $n - 1) { eval { die "x\n" }; $c++ if $@; }
print "$c\n";
