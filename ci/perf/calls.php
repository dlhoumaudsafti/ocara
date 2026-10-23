<?php
class Pt {
    public $x = 0;
    function dist($n) { return abs($n - $this->x); }
}
class Acc {
    public $total = 0;
    function add($p, $n) { $this->total += $p->dist($n); }
}
$n = (int)$argv[1]; $p = new Pt(); $a = new Acc();
for ($i = 0; $i < $n; $i++) $a->add($p, $i);
echo $a->total, "\n";
