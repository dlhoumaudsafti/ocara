<?php
$n = (int)$argv[1]; $line = "alpha,beta,gamma,delta,epsilon,zeta"; $tot = 0;
for ($i = 0; $i < $n; $i++) $tot += count(explode(",", $line));
echo $tot, "\n";
