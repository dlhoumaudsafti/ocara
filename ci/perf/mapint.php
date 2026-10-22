<?php
$n = (int)$argv[1]; $m = [];
for ($i = 0; $i < $n; $i++) $m[$i] = $i * 2;
$t = 0;
for ($i = 0; $i < $n; $i++) $t += $m[$i];
echo $t, "\n";
