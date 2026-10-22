use strict; use warnings;
package Node; sub new { bless { next => undef, v => 0 }, shift } sub get { $_[0]{v} }
package main;
my $n = $ARGV[0]; my @nodes; my $prev;
for my $i (0 .. $n - 1) { my $x = Node->new; $x->{v} = $i; $x->{next} = $prev; push @nodes, $x; $prev = $x; }
my $tot = 0;
for my $r (1 .. 5) { $tot += $_->get for @nodes; }
print "$tot\n";
