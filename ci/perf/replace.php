<?php
$n = (int)$argv[1]; $tot = 0;
for ($i = 0; $i < $n; $i++) $tot += strlen(strtoupper(str_replace("hello", "bye", "hello world hello")));
echo $tot, "\n";
