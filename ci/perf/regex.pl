use strict; use warnings;
my $n = $ARGV[0]; my ($hits, $out) = (0, 0);
for my $i (0 .. $n - 1) {
    my $line = "user$i\@example.com id=$i";
    $hits++ if $line =~ /^[a-z]+[0-9]+\@example\.com/;
    (my $r = $line) =~ s/[0-9]+/#/; $out += length $r;
}
print "$hits $out\n";
