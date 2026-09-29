# Erreur de type sur `async method`/`async function` dont le retour déclaré n'est pas `int`

## Constat (vérifié indépendamment)

Trouvé en marge de [langage-async-instance-method-dispatch-broken](langage-async-instance-method-dispatch-broken.md)
en testant les cas généraux du correctif avec un type de retour `string`
plutôt que `int`. Rien à voir avec le codegen d'instance corrigé par ce
ticket — un bug de SEMA (vérification de types), reproduit aussi bien pour
un appel STATIQUE que D'INSTANCE :

```ocara
class Doubler {
    public static async method fetch(): string {
        return "hi"
    }
}
function main(): int {
    var t:int = Doubler::fetch()   // ❌ error: expected type 'int', found 'string'
    var r:string = resolve t        // ❌ error: expected type 'string', found 'int'
    return 0
}
```

Toutes les démonstrations `async` existantes dans ce compilateur (tests,
exemples, la ticket `wiring` elle-même) utilisent `int` comme type de
retour — un choix qui masque ce bug par coïncidence, puisque le type
« réel » et le type « task handle » sont alors identiques.

## Cause (localisée)

`src/sema/typecheck.rs` n'applique la règle « un appel `async` retourne
`Type::Int` (le task handle), pas son type déclaré » qu'à UN SEUL endroit
(ligne ~1161, dans la résolution `Expr::Call` → fonction LIBRE via
`self.symbols.lookup_function`). Ni `Expr::StaticCall` (`Classe::methode()`)
ni le sucre d'instance (`Expr::Field` en position d'appel,
`objet.methode()`) n'ont l'équivalent — les deux retournent le type de
retour DÉCLARÉ de la méthode telle quelle, jamais `Type::Int`.

## Priorité / Complexité

**Haute** (bloque un usage légitime documenté — `async` sur une méthode
statique OU d'instance retournant autre chose qu'un `int`) — **Légère à
Structurel** : ajouter la même substitution `is_async → Type::Int` aux DEUX
points de résolution manquants (`Expr::StaticCall`, sucre d'instance) dans
`typecheck.rs`, en vérifiant s'il y a d'autres points de résolution
équivalents à couvrir (recherche par `sig.ret_ty.clone()` sans garde
`is_async` à proximité).

## Fichiers clés

`src/sema/typecheck.rs` (résolution de type d'`Expr::StaticCall` et
d'`Expr::Field` en position d'appel — comparer avec la ligne ~1161, déjà
correcte pour une fonction libre).
