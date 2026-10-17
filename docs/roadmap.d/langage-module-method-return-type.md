# Méthode de `module` : type de retour perdu au site d'appel (SIGSEGV)

Statut : **à corriger** — constaté le 2026-10-17 en complétant
`examples/tests/26_modulesTest.oc`.

## Constat

Le résultat d'une méthode reçue d'un `module`, utilisé directement en
argument (sans passer par une variable typée), est traité comme un pointeur :
`int` 0 → `null`, `bool` → `1`, `float` → **SIGSEGV** (code 139). La même
méthode déclarée dans la classe fonctionne.

```ocara
architecture permissive
import ocara.IO
module M {
    public method zero(): int { return 0 }
    public method yes(): bool { return true }
    public method half(): float { return 0.5 }
}
class C modules M {
    public method czero(): int { return 0 }
}
function main(): int {
    var c:C = use C()
    IO::writeln(c.czero())   // 0
    IO::writeln(c.zero())    // null  (attendu 0)
    IO::writeln(c.yes())     // 1     (attendu true)
    IO::writeln(c.half())    // SIGSEGV
    return 0
}
```

`var id:int = u.getId()` donne la bonne valeur (contournement utilisé dans
`26_modulesTest.oc`, signalé en commentaire).

Second symptôme lié : depuis un test, `import User from "26_modules"` ne
suffit pas — `field 'getId' not found in class 'User'` et
`undefined symbol 'module Timestamped'` tant que les modules ne sont pas
importés explicitement.

## Piste

`src/lower/builder.d/program.rs` : `fn_ret_types` est rempli pour les
méthodes de classe et d'interface, pas pour celles reçues des modules —
l'appel retombe sur `IrType::Ptr` par défaut (non vérifié en détail).

## Validation

La reproduction affiche `0`, `true`, `0.5` ; retirer le contournement de
`26_modulesTest.oc`.

## Priorité / Complexité

**Haute** (crash) — **Légère** probablement.
