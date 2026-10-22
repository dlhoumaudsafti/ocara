use strict; use warnings;
sub fib { my $n = shift; $n < 2 ? $n : fib($n - 1) + fib($n - 2) }
my $n = $ARGV[0]; my $acc = 0.0;
$acc += ($_ % 7) * 0.5 for 0 .. $n - 1;
print fib(30), " $acc\n";
