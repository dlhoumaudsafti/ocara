<?php
function fib($n) { return $n < 2 ? $n : fib($n - 1) + fib($n - 2); }
$n = (int)$argv[1]; $acc = 0.0;
for ($i = 0; $i < $n; $i++) $acc += ($i % 7) * 0.5;
echo fib(30), " ", $acc, "\n";
