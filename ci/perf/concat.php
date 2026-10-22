<?php
$n = (int)$argv[1]; $s = "";
// `$s = $s . "x"` recopie la chaîne à chaque tour (quadratique) : forme idiomatique.
for ($i = 0; $i < $n; $i++) $s .= "x";
echo strlen($s), "\n";
