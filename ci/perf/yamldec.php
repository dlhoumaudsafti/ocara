<?php
if (!function_exists('yaml_parse')) exit(1);
$n = (int)$argv[1]; $text = "";
for ($i = 0; $i < $n; $i++) $text .= "k$i:\n  name: item$i\n  value: $i\n";
echo count(yaml_parse($text)), "\n";
