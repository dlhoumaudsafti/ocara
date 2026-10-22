<?php
class AppError extends Exception {}
$n = (int)$argv[1]; $c = 0;
for ($i = 0; $i < $n; $i++) {
    try { throw new AppError("x", $i); } catch (AppError $e) { $c++; }
}
echo $c, "\n";
