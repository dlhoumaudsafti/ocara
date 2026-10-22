<?php
$n = (int)$argv[1]; $path = "/tmp/ocara_perf_file_php_" . getmypid() . ".txt"; $text = "";
for ($i = 0; $i < $n; $i++) $text .= "ligne $i\n";
file_put_contents($path, $text);
$lines = explode("\n", file_get_contents($path));
$tot = 0;
foreach ($lines as $l) $tot += strlen($l);
unlink($path);
echo count($lines), " ", $tot, "\n";
