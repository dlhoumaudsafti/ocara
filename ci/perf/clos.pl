use strict; use warnings;
my $n = $ARGV[0]; my $count = 0;
my $add = sub { $count += $_[0] };
$add->($_ % 3) for 0 .. $n - 1;
print "$count\n";
