<?php
class Counter {
    public $total = 0;
    function add($n) { $this->total += $this->weight($n); }
    function weight($n) { return $n * 2; }
}
$n = (int)$argv[1]; $c = new Counter();
for ($i = 0; $i < $n; $i++) $c->add($i);
echo $c->total, "\n";
