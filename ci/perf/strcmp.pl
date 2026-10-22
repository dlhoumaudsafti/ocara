use strict; use warnings;
my $n = $ARGV[0]; my @words = qw(alpha beta gamma delta); my $hits = 0;
for my $i (1 .. $n) { for my $w (@words) { $hits++ if $w eq "gamma"; } }
print "$hits\n";
