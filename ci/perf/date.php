<?php
$n = (int)$argv[1]; $tot = 0;
for ($i = 0; $i < $n; $i++) $tot += strlen(gmdate("d/m/Y H:i:s", 1700000000 + $i * 37));
echo $tot, "\n";
