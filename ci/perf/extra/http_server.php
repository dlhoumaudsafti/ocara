<?php
// Serveur intégré de PHP (`php -S`), 4 processus comme le serveur Ocara de
// http_server.oc ; il remplace ce processus (même pid, arrêté par le script).
pcntl_exec(PHP_BINARY, ["-S", "127.0.0.1:" . $argv[1], __DIR__ . "/http_router.php"], ["PHP_CLI_SERVER_WORKERS" => "4"]);
