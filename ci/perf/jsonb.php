<?php
$n = (int)$argv[1]; $tot = 0;
for ($i = 0; $i < $n; $i++) {
    $t = json_encode(["id" => $i, "name" => "ocara", "ok" => true, "tags" => [1, 2, 3]]);
    $b = json_decode($t, true);
    $tot += strlen($t) + count($b);
}
echo $tot, "\n";
