<?php
$n = (int)$argv[1]; $ti = 0; $tf = 0.0;
for ($i = 0; $i < $n; $i++) { $ti += intval("$i"); $tf += floatval("$i.5"); }
echo $ti, " ", $tf, "\n";
