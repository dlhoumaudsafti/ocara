use strict; use warnings;
my $n = $ARGV[0]; my $tot = 0;
for my $i (0 .. $n - 1) { (my $s = "hello world hello") =~ s/hello/bye/g; $tot += length uc $s; }
print "$tot\n";
