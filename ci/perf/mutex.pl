use strict; use warnings; use threads; use threads::shared;
my $n = $ARGV[0]; my $guard :shared = 0; my $c = 0;
for my $i (1 .. $n) { lock($guard); $c++; }
print "$c\n";
