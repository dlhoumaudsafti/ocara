<?php
$n = (int)$argv[1]; $t = 0;
for ($i = 0; $i < $n; $i++) { $x = $i; $t = $t + $x * 2; }
echo $t, "\n";
