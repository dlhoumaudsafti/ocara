use strict; use warnings;
# Itérateur par fermeture : Perl n'a pas de générateur.
sub numbers { my $n = shift; my $i = 0; sub { $i < $n ? $i++ : undef } }
my $n = $ARGV[0]; my $t = 0; my $it = numbers($n);
while (defined(my $v = $it->())) { $t += $v; }
print "$t\n";
