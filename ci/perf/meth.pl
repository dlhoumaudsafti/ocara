use strict; use warnings;
package Counter; sub new { bless { total => 0 }, shift }
sub add { my ($s, $n) = @_; $s->{total} += $s->weight($n) } sub weight { $_[1] * 2 }
package main;
my $n = $ARGV[0]; my $c = Counter->new;
$c->add($_) for 0 .. $n - 1;
print $c->{total}, "\n";
