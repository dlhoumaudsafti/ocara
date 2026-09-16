# Couverture de tests et de CI incomplète

Le bug d'import qui cassait `examples/tests/11_interfacesTest.oc`, la CI pour `examples/advanced/httpserver`, le skip propre de `builtins/mysql` sans serveur local, et l'ajout d'`examples/generics/main.oc` à `ci/regression.sh` (`List.oc` est une classe importée, pas un point d'entrée ; `test_syntax.oc` reste une fixture de coloration syntaxique sans `main()`, volontairement hors CI) sont corrigés — voir git log.

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
