<?php
$n = (int)$argv[1]; $count = 0;
$add = function ($k) use (&$count) { $count += $k; };
for ($i = 0; $i < $n; $i++) $add($i % 3);
echo $count, "\n";
