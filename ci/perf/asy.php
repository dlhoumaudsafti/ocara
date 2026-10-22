<?php
// Tâche asynchrone : une Fiber par appel (PHP n'a pas de boucle d'événements standard).
function sq($n) { return $n * $n; }
$n = (int)$argv[1]; $tot = 0;
for ($i = 0; $i < $n; $i++) {
    $f = new Fiber(fn() => sq($i));
    $f->start();
    $tot += $f->getReturn();
}
echo $tot, "\n";
