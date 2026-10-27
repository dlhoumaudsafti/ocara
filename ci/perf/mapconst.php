<?php
$n = (int)$argv[1];
$m = ["alpha" => 1, "beta" => 2, "gamma" => 3, "delta" => 4, "epsilon" => 5, "zeta" => 6, "eta" => 7, "theta" => 8];
$tot = 0;
for ($i = 0; $i < $n; $i++) {
    $tot += $m["alpha"] + $m["delta"] + $m["theta"];
    $m["beta"] = $i;
    $m["zeta"] += 1;
}
echo $tot + $m["beta"] + $m["zeta"], "\n";
