use strict; use warnings;
package AppError; sub new { my ($c, $m, $code) = @_; bless { message => $m, code => $code }, $c }
package main;
my $n = $ARGV[0]; my $c = 0;
for my $i (0 .. $n - 1) {
    eval { die AppError->new("x", $i) };
    $c++ if ref $@ && $@->isa("AppError");
}
print "$c\n";
