# Windows : variables locales perdues après une exception attrapée

Statut : **bug confirmé par reproduction** (2026-10-27), sous Wine 9.0 avec
`ocara.exe` et `libocara_runtime.a` cross-compilés (`make build-windows`).
Priorité haute : corruption silencieuse de valeurs, et plantage observé.

## Reproduction

```ocara
architecture permissive

import ocara.File
import ocara.IO

function main(): void {
    var s:string = File::read("loc.oc")
    var arr:array<int> = [1, 2, 3]
    try {
        File::read("C:/absent.txt")
    } on e {
        IO::writeln(`A ${e.code}`)
    }
    IO::writeln(`taille ${s.len()} ${arr[2]}`)
}
```

Compilé et lié selon [docs/windows.md](../windows.md) §4, puis exécuté sous
Wine : `A 101` puis `taille 0 0`. Sous Linux, le même programme affiche
`taille 456 3`.

Symptômes liés, vus en vérifiant `ocara.Image` sous Wine
(`examples/tests/117_image_builtinTest.oc`, mêmes appels) :

- après une première exception attrapée, l'appel de méthode suivant reçoit
  un receveur invalide (`img.crop(...)` lève `ImageException` 100 « valeur
  qui n'est pas une Image ») ;
- une exception levée par `img.setDrawColor(0, 0, 0, 300)` dans un `try`
  termine le processus (code 1, aucune sortie) : la trace `WINEDEBUG=+seh`
  montre une violation d'accès en lecture à l'adresse 8 dans une DLL de
  Wine, répétée en boucle.

Les exceptions levées par le runtime sont bien attrapées (code et message
corrects) : c'est l'état de la fonction englobante qui est perdu au retour
du `longjmp`.

## Pistes

- `setjmp`/`longjmp` de msvcrt sur x64 (`runtime/src/lib.rs`, `try`) :
  le `longjmp` Windows déroule les frames par SEH et restaure les registres
  non volatils sauvegardés par `setjmp` ; vérifier ce que le code Cranelift
  du `main` garde en registre (ou dans les cases sur pile de
  `Inst::AllocaWords`) à travers l'appel à `__ocara_try_exec`.
- Comparer avec `__builtin_setjmp`/`_setjmp` sans cadre (second argument
  nul de `_setjmp` sous mingw-w64) qui ne déroule pas par SEH.
- Ajouter un test de non-régression exécuté sous Wine (la machine de dev a
  Wine et mingw-w64) dès que la cause est trouvée.

## Complexité

Dangereuse — mécanisme d'exceptions et ABI Windows.
