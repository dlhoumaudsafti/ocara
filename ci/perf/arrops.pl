use strict; use warnings; use List::Util qw(first any);
my $n = $ARGV[0]; my @xs = (0 .. 999); my $tot = 0;
for my $i (0 .. $n - 1) {
    my $v = $i % 1500; $tot++ if any { $_ == $v } @xs;
    my $w = $i % 1000; $tot += first { $xs[$_] == $w } 0 .. $#xs;
    my @part = @xs[$i % 900 .. $i % 900 + 49];
    $tot += (reverse @part)[0];
    push @xs, $i; $tot += pop @xs;
}
print "$tot\n";
