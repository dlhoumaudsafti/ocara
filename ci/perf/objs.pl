use strict; use warnings;
package Point; sub new { my ($c, $x, $y) = @_; bless { x => $x, y => $y }, $c }
sub dist { my ($s, $o) = @_; ($s->{x} - $o->{x}) ** 2 + ($s->{y} - $o->{y}) ** 2 }
package main;
my $n = $ARGV[0]; my @pts = map { Point->new($_, $_ * 2) } 0 .. $n - 1; my $tot = 0;
$tot += $pts[$_]->dist($pts[$_ - 1]) for 1 .. $n - 1;
print "$tot\n";
