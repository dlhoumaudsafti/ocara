use strict; use warnings;
package Pt; sub new { bless { x => 0 }, shift } sub dist { abs($_[1] - $_[0]{x}) }
package Acc; sub new { bless { total => 0 }, shift }
sub add { my ($s, $p, $n) = @_; $s->{total} += $p->dist($n) }
package main;
my $n = $ARGV[0]; my $p = Pt->new; my $a = Acc->new;
$a->add($p, $_) for 0 .. $n - 1;
print $a->{total}, "\n";
