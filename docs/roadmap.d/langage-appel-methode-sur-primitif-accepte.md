# Appel de méthode chaîné sur un récepteur `int`/`float`/`bool`/`null` accepté silencieusement, résultat faux

## Constat

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

**Compile sans la moindre erreur ni avertissement.** À l'exécution, affiche `null` au lieu de planter ou d'être rejeté — résultat silencieusement faux, même famille de gravité que E36 et que les bugs de [langage-use-chaine-valeur-retour-perdue](langage-use-chaine-valeur-retour-perdue.md).

## Cause

`src/sema/typecheck.rs`, résolution du récepteur d'un appel de méthode (`Expr::Call { callee: Expr::Field { object, field } }`) : `type_class_name(&obj_ty)` ne reconnaît que `Type::Named`/`Type::Qualified`/`Type::Union` (récursif)/`Type::String`/`Type::Array`/`Type::Map` — pour TOUT AUTRE type (`Type::Int`, `Type::Float`, `Type::Bool`, `Type::Null`, `Type::Message`, `Type::Function`), il retourne `None`, et le code retombe dans le même filet de sécurité permissif qui causait E36 : `_ => { for a in args { self.infer_expr(a); } return Type::Mixed; }` — aucune vérification que `field` existe seulement pour la classe résolue.

Le correctif E36 a délibérément restreint le rejet au SEUL cas `Type::Void` (voir sa doc : « portée volontairement restreinte à ce cas précis... plutôt que généraliser à toute utilisation d'un type non-classe comme valeur ») — ce ticket couvre exactement ce qui a été laissé de côté.

## Piste (non tranchée)

Généraliser le rejet de E36 à `Int`/`Float`/`Bool`/`Null` semble être le même correctif mécanique (une branche `matches!(obj_ty, Type::Int | Type::Float | Type::Bool | Type::Null)` à côté de celle pour `Type::Void`), avec un message qui nomme le type réel plutôt que de dire simplement « void ». `Type::Mixed` ne doit PAS être concerné — son imprécision est un choix assumé et documenté ailleurs dans le langage (désactive volontairement la vérification de types). `Type::Function`/`Type::Message` n'ont pas été vérifiés — `message<T>` a déjà des restrictions fortes ailleurs (jamais nommable, consommé immédiatement) qui rendent peut-être ce chemin déjà inatteignable pour lui ; à confirmer avant d'écrire le correctif, pas supposer.

## Priorité / Complexité

**Priorité Haute** — même motif que E36 et les tickets voisins : résultat silencieusement faux, aucun rejet à la compilation.
**Complexité : non évaluée** — probablement Légère (même mécanisme que E36, juste étendu à d'autres variantes de `Type`), à confirmer en écrivant le correctif, notamment pour `Function`/`Message`.

## Fichiers clés

`src/sema/typecheck.rs` (résolution du récepteur, fonction `type_class_name`, la branche déjà corrigée pour E36), `src/sema/error.rs` (`SemaError::MethodCallOnVoid`, à généraliser ou dupliquer), [langage-appel-methode-sur-void-accepte](langage-appel-methode-sur-void-accepte.md) (le correctif dont celui-ci reprend exactement le mécanisme, restreint à `void`).
