use strict; use warnings;
my $n = $ARGV[0];
my %m = (alpha => 1, beta => 2, gamma => 3, delta => 4, epsilon => 5, zeta => 6, eta => 7, theta => 8);
my $tot = 0;
for my $i (0 .. $n - 1) {
    $tot += $m{alpha} + $m{delta} + $m{theta};
    $m{beta} = $i;
    $m{zeta} += 1;
}
print $tot + $m{beta} + $m{zeta}, "\n";
