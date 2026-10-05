# `parent::init(...)` sans effet pour un parent builtin d'exception

## Constat

Trouvé en corrigeant [sema-use-args-without-init](sema-use-args-without-init.md) :

```ocara
class MyErr extends Exception {
    init(message:string, code:int) {
        parent::init(message, code)   // compile, n'affecte rien
    }
}

try {
    raise use MyErr("boom", 2)
} on err is MyErr {
    IO::writeln(err.message)          // affiche "null"
}
```

Toutes les sous-classes d'exception du corpus contournent le problème en
écrivant `self.message = message` et `self.code = code`
(`examples/tests/36_mutex_withlockTest.oc`…). À l'inverse, `parent::init("x")`
fonctionne pour `HTMLComponent` (`examples/advanced/httpserver/configs/components/`).

## À trancher

- **Appel réel** du constructeur des exceptions builtin (`message`, `code`,
  `source`), comme pour `HTMLComponent`.
- Ou **erreur de compilation**, qui renvoie vers `self.message = …`.

## Priorité / Complexité

Haute (erreur silencieuse) — **Simple à Légère**.

## Fichiers clés

`src/lower/expr.d/lower.rs` (appel `parent::init`, `class == "<parent>"`),
`src/builtins/exception.rs`, `runtime/src/exception.rs`.
