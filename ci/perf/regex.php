<?php
$n = (int)$argv[1]; $hits = 0; $out = 0;
for ($i = 0; $i < $n; $i++) {
    $line = "user$i@example.com id=$i";
    if (preg_match('/^[a-z]+[0-9]+@example\.com/', $line)) $hits++;
    $out += strlen(preg_replace('/[0-9]+/', '#', $line, 1));
}
echo $hits, " ", $out, "\n";
