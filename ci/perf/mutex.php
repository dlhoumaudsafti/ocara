<?php
// Pas de verrou d'exclusion mutuelle dans PHP sans extension (sync) : mesure sautée.
if (!class_exists('SyncMutex')) exit(1);
$n = (int)$argv[1]; $m = new SyncMutex(); $c = 0;
for ($i = 0; $i < $n; $i++) { $m->lock(); $c++; $m->unlock(); }
echo $c, "\n";
