use strict; use warnings; use JSON::PP;
my $n = $ARGV[0]; my $json = JSON::PP->new; my $tot = 0;
for my $i (0 .. $n - 1) {
    my $t = $json->encode({ id => $i, name => "ocara", ok => JSON::PP::true, tags => [1, 2, 3] });
    my $b = $json->decode($t); $tot += length($t) + keys %$b;
}
print "$tot\n";
