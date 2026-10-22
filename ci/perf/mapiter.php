<?php
$n = (int)$argv[1]; $m = [];
for ($i = 0; $i < 1000; $i++) $m["k$i"] = $i;
$tot = 0;
for ($r = 0; $r < $n; $r++) foreach ($m as $k => $v) $tot += $v;
echo $tot, "\n";
