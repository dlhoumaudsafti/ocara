<?php
// Pas de threads dans PHP CLI sans extension (parallel) : mesure sautée.
if (!class_exists('parallel\Runtime')) exit(1);
