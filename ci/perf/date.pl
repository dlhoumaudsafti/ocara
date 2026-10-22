use strict; use warnings; use POSIX qw(strftime);
my $n = $ARGV[0]; my $tot = 0;
$tot += length strftime("%d/%m/%Y %H:%M:%S", gmtime(1700000000 + $_ * 37)) for 0 .. $n - 1;
print "$tot\n";
