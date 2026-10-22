<?php
class Node { public $next = null; public $v = 0; function get() { return $this->v; } }
$n = (int)$argv[1]; $nodes = []; $prev = null;
for ($i = 0; $i < $n; $i++) { $x = new Node(); $x->v = $i; $x->next = $prev; $nodes[] = $x; $prev = $x; }
$tot = 0;
for ($r = 0; $r < 5; $r++) foreach ($nodes as $x) $tot += $x->get();
echo $tot, "\n";
