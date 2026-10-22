<?php
$n = (int)$argv[1]; $xs = range(0, 999); $tot = 0;
for ($i = 0; $i < $n; $i++) {
    if (in_array($i % 1500, $xs, true)) $tot++;
    $tot += array_search($i % 1000, $xs, true);
    $part = array_slice($xs, $i % 900, 50);
    $tot += array_reverse($part)[0];
    $xs[] = $i;
    $tot += array_pop($xs);
}
echo $tot, "\n";
