<?php
$n = (int)$argv[1]; $tot = 0; $name = "ocara";
for ($i = 0; $i < $n; $i++) { $s = "id=$i name=$name ok"; $tot += strlen($s); }
echo $tot, "\n";
