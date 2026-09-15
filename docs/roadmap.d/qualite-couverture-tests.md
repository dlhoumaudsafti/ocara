# Couverture de tests et de CI incomplète

Le bug d'import qui cassait `examples/tests/11_interfacesTest.oc`, la CI pour `examples/advanced/httpserver`, et le skip propre de `builtins/mysql` sans serveur local sont corrigés — voir git log.

## Reste à faire : `examples/generics/` toujours absent de toute cible Makefile/CI

La syntaxe obsolète (`T[]`) y a été corrigée (ces fichiers compilent), mais aucune cible Makefile/CI ne les exécute — `examples/generics/` reste entièrement hors du filet de `make regression`. À ajouter, avec un script dédié si besoin (même format que `httpserver.sh`).

## Pour plus tard : vraie infrastructure CI pour MySQL

Le skip local (`ci/regression.sh` sonde `127.0.0.1:3306`) évite un faux échec, mais ne teste jamais réellement une connexion MySQL. Ce projet n'a aujourd'hui aucun pipeline CI versionné (`.github/workflows/`, `.gitlab-ci.yml`...) — le jour où il en aura un, fournir un service MySQL au runner :

```yaml
jobs:
  regression:
    runs-on: ubuntu-latest
    services:
      mysql:
        image: mysql:8
        env:
          MYSQL_ALLOW_EMPTY_PASSWORD: yes
          MYSQL_DATABASE: test_db
        ports:
          - 3306:3306
        options: >-
          --health-cmd="mysqladmin ping"
          --health-interval=5s --health-timeout=3s --health-retries=5
    steps:
      - uses: actions/checkout@v4
      - run: make build
      - run: make regression   # builtins/mysql s'exécute réellement : le port 3306 répond
```

## Fichiers clés

`ci/regression.sh`, `examples/generics/`.
