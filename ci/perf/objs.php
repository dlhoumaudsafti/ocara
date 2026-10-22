<?php
class Point {
    function __construct(public $x, public $y) {}
    function dist($o) { return ($this->x - $o->x) * ($this->x - $o->x) + ($this->y - $o->y) * ($this->y - $o->y); }
}
$n = (int)$argv[1]; $pts = [];
for ($i = 0; $i < $n; $i++) $pts[] = new Point($i, $i * 2);
$tot = 0;
for ($i = 1; $i < $n; $i++) $tot += $pts[$i]->dist($pts[$i - 1]);
echo $tot, "\n";
