# Appel de méthode inexistante sur une variable d'exception accepté en silence

## Constat

Reproduction (trouvée en écrivant le test de `HTTPServerSession`) :

```ocara
import ocara.IO
import ocara.Exception

function main(): int {
    try {
        raise use Exception("boom", 3)
    } on e is Exception {
        IO::writeln("msg=" + e.message())   // affiche "msg=null"
        IO::writeln("nope=" + e.nothing())  // affiche "nope=null"
    }
    return 0
}
```

Les deux appels compilent sans aucun diagnostic et valent `null`.
`message`/`code`/`source` sont des **champs** des exceptions builtin
(`src/builtins/exception.rs`, `make_exception_class`), pas des méthodes ; la
forme correcte est `e.message`. Sur une classe utilisateur, la même erreur est
rejetée (`error: field 'x' not found in class 'A'`).

Effet : une faute de frappe dans un gestionnaire d'erreur passe inaperçue et
masque le message de l'exception, exactement là où on en a besoin.

## À faire

- Dans `src/sema/typecheck.rs` (appel de méthode sur un receveur de type
  classe builtin), signaler une méthode introuvable sur une classe
  d'exception, comme pour une classe utilisateur. Message suggéré quand le
  nom est un champ : « `message` est un champ, pas une méthode : écrire
  `e.message` ».
- Vérifier le même chemin pour les autres classes builtin dont les méthodes
  manquantes retombent sur `mixed`.
- Test sema dans `src/sema/tests/`.

## Priorité / Complexité

Haute (erreur silencieuse) — **Simple**.

## Fichiers clés

`src/sema/typecheck.rs`, `src/builtins/exception.rs`.
