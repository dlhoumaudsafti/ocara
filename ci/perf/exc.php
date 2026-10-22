<?php
$n = (int)$argv[1]; $c = 0;
for ($i = 0; $i < $n; $i++) {
    try { throw new Exception("x"); } catch (Exception $e) { $c++; }
}
echo $c, "\n";
