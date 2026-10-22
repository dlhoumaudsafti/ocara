use strict; use warnings;
my $n = $ARGV[0]; my $path = "/tmp/ocara_perf_file_pl_$$.txt"; my $text = "";
$text .= "ligne $_\n" for 0 .. $n - 1;
open my $w, ">", $path or die; print $w $text; close $w;
open my $r, "<", $path or die; local $/; my $all = <$r>; close $r;
my @lines = split /\n/, $all, -1; my $tot = 0; $tot += length for @lines;
unlink $path;
print scalar(@lines), " $tot\n";
