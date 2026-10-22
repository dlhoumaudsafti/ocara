<?php
$n = (int)$argv[1]; $m = [];
for ($i = 0; $i < $n; $i++) $m["k$i"] = $i;
$tot = 0;
for ($i = 0; $i < $n; $i++) $tot += $m["k$i"];
echo $tot, "\n";
