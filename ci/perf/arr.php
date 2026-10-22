<?php
$n = (int)$argv[1]; $xs = [];
for ($i = 0; $i < $n; $i++) $xs[] = $i;
$tot = 0;
for ($r = 0; $r < 10; $r++) for ($i = 0; $i < $n; $i++) $tot += $xs[$i];
echo $tot, "\n";
