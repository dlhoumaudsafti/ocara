<?php
function numbers($n) { for ($i = 0; $i < $n; $i++) yield $i; }
$n = (int)$argv[1]; $t = 0;
foreach (numbers($n) as $v) $t += $v;
echo $t, "\n";
