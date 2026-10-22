<?php
$n = (int)$argv[1]; $xs = ["a", "b", "c", "d", "e"]; $tot = 0;
for ($i = 0; $i < $n; $i++) $tot += strlen(implode("-", $xs)) + strlen((string)$i);
echo $tot, "\n";
