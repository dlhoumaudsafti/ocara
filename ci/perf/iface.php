<?php
interface Weight { function weight($n); }
class Double implements Weight { function weight($n) { return $n * 2; } }
class Triple implements Weight { function weight($n) { return $n * 3; } }
function total(Weight $w, $n) { $t = 0; for ($i = 0; $i < $n; $i++) $t += $w->weight($i); return $t; }
$n = (int)$argv[1];
echo total(new Double(), $n) + total(new Triple(), $n), "\n";
