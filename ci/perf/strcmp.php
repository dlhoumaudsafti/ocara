<?php
$n = (int)$argv[1]; $words = ["alpha", "beta", "gamma", "delta"]; $hits = 0;
for ($i = 0; $i < $n; $i++) foreach ($words as $w) if ($w === "gamma") $hits++;
echo $hits, "\n";
