use strict; use warnings;
exit 1 unless eval { require YAML::XS; 1 };
my $n = $ARGV[0]; my $text = "";
$text .= "k$_:\n  name: item$_\n  value: $_\n" for 0 .. $n - 1;
print scalar(keys %{ YAML::XS::Load($text) }), "\n";
