# SIGSEGV : appeler une méthode D'INSTANCE `async` via le sucre `obj.methode()`

## Constat (vérifié indépendamment, gdb + HIR)

Trouvé en marge de [langage-interface-method-modifiers](langage-interface-method-modifiers.md)
en essayant d'exercer `async` sur une méthode d'interface D'INSTANCE dans un
test de régression. Reproduit d'abord avec une interface, puis ISOLÉ à une
classe **concrète ordinaire, sans la moindre interface** — ce n'est donc pas
un bug d'interface ni de `wiring`, mais un bug général et préexistant :

```ocara
import ocara.IO

class DoublingFetcher {
    public async method fetch(n:int): int {
        return n * 2
    }
}

function main(): int {
    var f:DoublingFetcher = use DoublingFetcher()
    var t:int = f.fetch(21)   // sucre d'instance, PAS Class::method()
    var r:int = resolve t
    IO::writeln(r)             // SIGSEGV
    return 0
}
```

Confirmé par gdb (`gdb -q -batch -ex run -ex bt`) : crash dans
`__task_resolve`, appelé avec une valeur qui n'est PAS un vrai task handle.
Confirmé par `ocara build --dump` (HIR) : le site d'appel `f.fetch(21)`
émet un `Inst::Call` DIRECT vers `DoublingFetcher_fetch` (la fonction
SYNCHRONE réelle, qui retourne `42` directement), jamais vers
`__async_wrap_DoublingFetcher_fetch`/`__task_spawn` (le vrai mécanisme de
spawn, qui EXISTE et fonctionne — voir plus bas). `main` reçoit donc `42`
(un entier ordinaire) et le passe tel quel à `resolve`/`__task_resolve`, qui
le traite à tort comme un pointeur de handle → SIGSEGV.

**Root cause localisée précisément** : dans `src/lower/expr.d/lower.rs`, le
bras `Expr::Call { callee: Expr::Field { .. } }` (sucre d'instance,
`obj.methode(...)`, lignes ~367-678) émet son `Inst::Call` (ligne ~649)
SANS JAMAIS consulter `builder.async_funcs` — contrairement aux DEUX autres
formes d'appel qui, elles, le font correctement :
- `Expr::Call { callee: Expr::Ident }` (fonction libre) — vérifie
  `builder.async_funcs.contains(func_name)` (ligne ~725) et spawn
  correctement une tâche si besoin.
- `Expr::StaticCall` (`Classe::methode()`, y compris une méthode STATIQUE
  d'interface après résolution `wiring`) — vérifie également
  `builder.async_funcs.contains(func_name)` (ligne ~949) et spawn
  correctement — confirmé par reproduction : `Doubler::fetch(21)` avec
  `public static async method fetch(...)` fonctionne parfaitement
  (`resolve` renvoie la bonne valeur).

Le bug touche donc UNIQUEMENT la troisième forme d'appel (sucre d'instance),
qui n'a simplement jamais eu son propre chemin de spawn de tâche — pas un
oubli spécifique aux interfaces, un trou resté invisible jusqu'ici faute
d'un test de régression exerçant `async method` (sans `static`) appelée via
`obj.methode()` plutôt que `Classe::methode()`.

## Pourquoi ce n'est pas dans le ticket `langage-interface-method-modifiers`

Ce ticket ne demandait que la symétrie GRAMMATICALE avec `class` pour les
modificateurs d'une méthode d'interface, plus une vérification de
conformité minimale (`is_static`/`is_async`) — la conformité elle-même
fonctionne correctement (vérifiée indépendamment du bug ci-dessus, qui est
un problème de CODEGEN, pas de sema). Le bug touche `async` sur N'IMPORTE
QUELLE classe, avec ou sans interface, avec ou sans `wiring` — corriger le
mécanisme de dispatch d'instance async en profondeur (répliquer toute la
logique de spawn de tâche — allocation d'environnement, wrapper, etc. —
déjà en place pour l'appel statique, plus son interaction avec
`class_dispatcher_name`/`generate_interface_dispatchers` pour le dispatch
virtuel) est un chantier de codegen à part entière, structurel, pas une
retouche mineure au périmètre d'un ticket de grammaire.

## Contournement disponible

Utiliser `static async method` (appelé via `Classe::methode()` ou
`Interface::methode()`) plutôt qu'une méthode `async` D'INSTANCE (appelée
via `obj.methode()`) — le chemin statique fonctionne correctement
aujourd'hui, le chemin d'instance non.

## Priorité / Complexité

**Priorité Haute** — SIGSEGV sur un usage a priori légitime (`async` étant
déjà une fonctionnalité documentée et fonctionnelle pour les fonctions
libres et les méthodes statiques). Complexité probablement **Structurel** :
il ne s'agit pas d'ajouter un simple `if builder.async_funcs.contains(...)`
au bras `Expr::Field` — il faut aussi déterminer comment `self`/le
récepteur s'intègre dans l'environnement heap alloué pour le spawn (les
deux chemins existants, fonction libre et appel statique, n'ont jamais eu
de récepteur `self` à empaqueter), et comment cela interagit avec le
dispatch dynamique (`class_dispatcher_name`/`generate_interface_dispatchers`)
— un objet dont la classe réelle n'est connue qu'à l'exécution complique le
choix du bon `__async_wrap_<Classe>_<méthode>` à spawn.

## Fichiers clés

`src/lower/expr.d/lower.rs` (bras `Expr::Call { callee: Expr::Field }`,
lignes ~367-678 — comparer avec les DEUX chemins déjà corrects, `Expr::Call
{ callee: Expr::Ident }` ~ligne 725 et `Expr::StaticCall` ~ligne 949),
`src/lower/builder.d/program.rs` (`async_funcs`, `generate_async_wrapper`),
`src/lower/builder.d/class_dispatch.rs` (`class_dispatcher_name` — à
recouper avec le spawn de tâche pour le cas virtuel).
