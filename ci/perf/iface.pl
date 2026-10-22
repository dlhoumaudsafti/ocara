use strict; use warnings;
package Double; sub new { bless {}, shift } sub weight { $_[1] * 2 }
package Triple; sub new { bless {}, shift } sub weight { $_[1] * 3 }
package main;
sub total { my ($w, $n) = @_; my $t = 0; $t += $w->weight($_) for 0 .. $n - 1; $t }
my $n = $ARGV[0];
print total(Double->new, $n) + total(Triple->new, $n), "\n";
