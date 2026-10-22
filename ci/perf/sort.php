<?php
$n = (int)$argv[1]; $xs = [];
for ($i = 0; $i < $n; $i++) $xs[] = ($i * 7919) % 100003;
sort($xs);
echo $xs[0], " ", $xs[$n - 1], "\n";
