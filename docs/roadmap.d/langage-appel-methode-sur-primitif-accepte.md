# Appel de méthode chaîné sur un récepteur `int`/`float`/`bool`/`null` accepté silencieusement, résultat faux

## Terminé — rejet à la compilation (E37)

Voir §"Ce qui a été fait" plus bas.

## Constat (avant correctif)

Découvert en discutant du correctif de [langage-appel-methode-sur-void-accepte](langage-appel-methode-sur-void-accepte.md) (E36) — l'utilisateur a demandé si le même genre de garde-fou existait pour un récepteur `int`/`float`/`bool` chaîné vers une méthode qui n'a de sens que sur un autre type. Réponse vérifiée : non.

Repro :

```ocara
import ocara.IO

class Foo {
    public method getCount(): int {
        return 42
    }
}

function main(): int {
    var f:Foo = use Foo()
    var r:string = f.getCount().upper()   // .upper() n'existe sur AUCUN int
    IO::writeln(r)
    return 0
}
```

**Compilait sans la moindre erreur ni avertissement.** À l'exécution, affichait `null` au lieu de planter ou d'être rejeté — résultat silencieusement faux, même famille de gravité que E36 et que les bugs de [langage-use-chaine-valeur-retour-perdue](langage-use-chaine-valeur-retour-perdue.md).

## Cause

`src/sema/typecheck.rs`, résolution du récepteur d'un appel de méthode (`Expr::Call { callee: Expr::Field { object, field } }`) : `type_class_name(&obj_ty)` ne reconnaît que `Type::Named`/`Type::Qualified`/`Type::Union` (récursif)/`Type::String`/`Type::Array`/`Type::Map` — pour TOUT AUTRE type (`Type::Int`, `Type::Float`, `Type::Bool`, `Type::Null`, `Type::Message`, `Type::Function`), il retourne `None`, et le code retombait dans le même filet de sécurité permissif qui causait E36 : `_ => { for a in args { self.infer_expr(a); } return Type::Mixed; }` — aucune vérification que `field` existe seulement pour la classe résolue.

Le correctif E36 avait délibérément restreint le rejet au SEUL cas `Type::Void` (voir sa doc : « portée volontairement restreinte à ce cas précis... plutôt que généraliser à toute utilisation d'un type non-classe comme valeur ») — ce ticket couvrait exactement ce qui avait été laissé de côté.

## Ce qui a été fait

Nouveau diagnostic **E37** (`SemaError::MethodCallOnNonClass`, `docs/diagnostics.md` §E37) : même mécanisme que E36, dans `src/sema/typecheck.rs`, juste après la branche déjà existante pour `Type::Void` — un second `if matches!(obj_ty, Type::Int | Type::Float | Type::Bool | Type::Null | Type::Message(_) | Type::Function { .. })` rejette explicitement le récepteur au lieu de retomber dans le filet permissif, avec un message qui nomme le type réel (`type_name(&obj_ty)`) plutôt que de dire simplement « void ».

`Type::Mixed` n'est PAS concerné — son imprécision reste un choix de langage assumé (déjà documenté par l'avertissement W02), pas un oubli.

`Type::Function`/`Type::Message` : **vérifiés par reproduction avant d'écrire le correctif, pas supposés**, comme le préconisait la piste initiale de ce ticket —

- `Type::Message(T)` — malgré les fortes restrictions de `message<T>` ailleurs (jamais nommable en `var`/`scoped`/`consumed`, voir sa doc dans le `Type` enum), un appel de fonction déclarée `: message<T>` utilisé DIRECTEMENT comme récepteur d'un appel de méthode (`gen().foo()` où `gen(): message<int>`) était bien atteignable et acceptait silencieusement n'importe quelle méthode avant ce correctif — confirmé par reproduction.
- `Type::Function { .. }` — les fonctions sont des valeurs de premier ordre dans ce langage (`var f:Function<int(int,int)> = add`), et `f.foo()` était lui aussi silencieusement accepté avant ce correctif — confirmé par reproduction.

### Vérifications

- 9 nouveaux tests Rust unitaires (`src/sema/tests/method_call_on_non_class.rs`) : le cas exact du ticket (`int`), plus `float`/`bool`/`null` (littéral direct)/`message<T>`/`Function<...>` rejetés ; et en non-régression : `mixed` toujours accepté, les méthodes d'instance sucrées `string`/`array` toujours acceptées, le chaînage sur un retour de classe réel toujours accepté.
- `make tests` : 110 passed (101 + 9). `make regression` (cache vidé) : 684 + 50 PASS, 0 FAIL, 0 ERREUR — confirme qu'aucun exemple existant du corpus ne reposait, même involontairement, sur ce chaînage désormais rejeté. `make build` : 0 warning.
- Cette section de la roadmap (« Priorité Haute ») est désormais vide — au sens de sa propre définition (« cette section vide = le langage est stable »), c'était le dernier point qui l'occupait.

## Fichiers clés

`src/sema/error.rs` (`SemaError::MethodCallOnNonClass`), `src/sema/typecheck.rs` (le rejet, juste après la branche E36), `src/sema/tests/method_call_on_non_class.rs` (nouveau), `docs/diagnostics.md` (§E37, nouveau), [langage-appel-methode-sur-void-accepte](langage-appel-methode-sur-void-accepte.md) (E36, le correctif dont celui-ci reprend exactement le mécanisme).
