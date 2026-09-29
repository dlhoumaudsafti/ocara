# Appel chaîné cassé sur le résultat d'une fonction LIBRE — `maFonction(...).methode()` retourne `null` silencieusement

## Constat (vérifié indépendamment)

Même famille de bug que [langage-use-chaine-valeur-retour-perdue](langage-use-chaine-valeur-retour-perdue.md)
(déjà clos) — un récepteur chaîné qui n'est PAS une variable nommée fait
échouer la résolution de sa classe — mais un déclencheur différent, non
couvert par ce correctif : le résultat d'un appel de **fonction libre**
(pas `use Classe(...)`, pas `Classe::méthode(...)`, une fonction top-level
ordinaire) utilisé directement comme récepteur d'un appel de méthode chaîné.

Trouvé en marge de [langage-interface-instance-dispatch-segfault](langage-interface-instance-dispatch-segfault.md)
en écrivant des tests de dispatch avec un type concret choisi à
l'exécution — reproduit indépendamment avec une classe CONCRÈTE ordinaire,
sans la moindre interface en jeu :

```ocara
import ocara.IO

class Circle {
    public property r:float
    init(r:float) { self.r = r }
    public method shapeName(): string {
        return "circle"
    }
}

function pickCircle(kind:int): Circle {
    return use Circle(2.0)
}

function main(): int {
    IO::writeln(pickCircle(0).shapeName())  // affiche "null", pas "circle"
    return 0
}
```

Contournement disponible (comme pour le ticket original) : lier le résultat
à une variable avant d'appeler la méthode —
`var c:Circle = pickCircle(0); c.shapeName()` fonctionne correctement.

## Cause (hypothèse, à confirmer avant de corriger — pas encore investiguée en profondeur)

Le ticket original a établi le patron exact de ce bug : plusieurs points de
résolution de la classe d'un récepteur chaîné (`src/lower/expr.d/lower.rs`,
`typeinfer.rs`, `helpers.rs::resolve_chained_field_class`, etc.) ne
reconnaissaient que certaines formes de récepteur (`Expr::Ident`,
`Expr::SelfExpr`, `Expr::ParentExpr`, `Expr::Field`), avec des cas ajoutés
au fil des découvertes pour `Expr::New` (`use Classe(...)`) et
`Expr::StaticCall` limité à `HTTPRequest::get/post/put/delete/patch`. Un
appel de fonction libre (`Expr::Call` dont le `callee` est un `Expr::Ident`
référençant une FONCTION top-level, pas une variable) est vraisemblablement
un troisième cas jamais couvert — la fonction connaît pourtant son type de
retour déclaré (`function pickCircle(kind:int): Circle`), donc l'information
nécessaire existe déjà côté signature, il s'agit probablement d'ajouter ce
cas aux mêmes points de résolution que le ticket original, pas de
réinventer un mécanisme.

Symptôme légèrement différent de l'original : inspection du HIR montre un
mangling vers `"_method_shapeName"` (sans préfixe de classe, symbole
inexistant) plutôt que la devinette `"String_méthode"` du ticket original —
le filet de sécurité générique semble avoir changé depuis, ou ce chemin
particulier retombe sur un filet différent. À élucider avant de corriger.

## Priorité / Complexité

**Priorité Haute** — comportement silencieusement faux (pas un crash) sur un
usage très courant (chaîner un appel de méthode sur le retour d'une
fonction), dans la même famille qu'un bug déjà traité à ce niveau de
priorité. Complexité probablement Légère à en juger par le correctif du
ticket original (ajout d'un cas de plus aux mêmes points de résolution déjà
identifiés) — mais à confirmer une fois la cause réellement investiguée,
pas seulement supposée par analogie.

## Fichiers clés

`src/lower/expr.d/lower.rs`, `src/lower/expr.d/typeinfer.rs`,
`src/lower/expr.d/helpers.rs` (`resolve_chained_field_class`) — mêmes
fichiers que [langage-use-chaine-valeur-retour-perdue](langage-use-chaine-valeur-retour-perdue.md),
probablement les mêmes points d'ajout, plus le mécanisme de mangling
`"_method_<nom>"` mentionné ci-dessus à localiser précisément.
