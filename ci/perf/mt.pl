use strict; use warnings; use threads;
package Point; sub new { bless { x => $_[1] }, $_[0] }
package main;
my $n = $ARGV[0];
threads->create(sub { })->join;
my @pts; push @pts, Point->new($_) for 0 .. $n - 1;
my $tot = 0; $tot += $_->{x} for @pts;
print "$tot\n";
