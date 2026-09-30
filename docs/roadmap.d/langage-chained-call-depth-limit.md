# Appel chaîné cassé à partir de 3 niveaux — `a.b().c().d()` (et non `a.b().c()`)

## Constat (vérifié indépendamment)

Trouvé en marge de [langage-chained-call-on-free-function-result](langage-chained-call-on-free-function-result.md)
en testant des variantes du correctif. **Général et préexistant**, sans
rapport avec les fonctions libres ni les interfaces — reproduit avec une
chaîne de méthodes d'instance PURE, aucune fonction libre en jeu :

```ocara
class Wrapper {
    public property inner:Circle
    init(c:Circle) { self.inner = c }
    public method getCircle(): Circle { return self.inner }
}
class Circle {
    public method shapeName(): string { return "circle" }
}
function main(): int {
    var w:Wrapper = use Wrapper(use Circle(2.0))
    IO::writeln(w.getCircle().shapeName().upper())   // affiche "null"
    return 0
}
```

`w.getCircle().shapeName()` (2 niveaux) fonctionne déjà correctement — le
échec commence au 3ème niveau (`.upper()` sur le résultat).

## Cause (localisée, pas encore corrigée)

Toute la famille de résolution "classe du récepteur d'un appel chaîné"
(`src/lower/expr.d/lower.rs`, `typeinfer.rs`, `helpers.rs::resolve_chained_field_class`)
ne recurse qu'**un seul niveau** : le cas `Expr::Call { callee: Expr::Field
{ object: inner_obj, .. } }` résout `inner_obj` uniquement s'il est lui-même
`Expr::Ident` (variable nommée) — jamais s'il est LUI-MÊME un
`Expr::Call`/`Expr::Field` imbriqué. Une vraie correction demanderait de
transformer ces blocs de `match` répétés (un par fichier, quasi-identiques)
en une seule fonction récursive partagée `resolve_receiver_class(expr) ->
Option<String>`, appelable à n'importe quelle profondeur — pas juste
ajouter un nouveau cas de plus au même patron non-récursif.

## Priorité / Complexité

**Haute** (résultat silencieusement faux, pas un crash) — **Structurel**
(refactor en fonction récursive partagée, pas un cas de plus dans un
`match` existant ; à répliquer identiquement dans au moins 3 fichiers
aujourd'hui dupliqués).

## Fichiers clés

`src/lower/expr.d/lower.rs`, `src/lower/expr.d/typeinfer.rs`,
`src/lower/expr.d/helpers.rs` (`resolve_chained_field_class`).

## Résolution (implémentée)

**Root cause reconfirmée, plus grave que la description initiale** : au
moment de coder, le repro cassait déjà À 2 NIVEAUX (`w.getCircle().shapeName()`
affichait `"null"`, pas seulement `.upper()` en 3ème position) — régression
survenue entre-temps dans le code (plusieurs correctifs de cette session ont
touché les mêmes fichiers : dispatch d'interface, dispatch async d'instance,
`Resolvable<T>`). Cause exacte : `IrModule::fn_ret_types`/`method_ret_types`
ne portent JAMAIS d'entrée pour une méthode de classe ORDINAIRE (seulement
fonctions libres, méthodes d'INTERFACE, et builtins) — la seule heuristique
existante pour un appel chaîné (`w.getCircle().shapeName()`) supposait donc,
à tort, qu'une méthode retournant un pointeur (`IrType::Ptr`) retournait la
MÊME classe que son récepteur (confondait `Wrapper::getCircle(): Circle`
avec `Wrapper` lui-même) — un mangling vers un symbole qui n'existe pas
(`Wrapper_shapeName` au lieu de `Circle_shapeName`), ignoré silencieusement
par le codegen.

**Fix** : nouvelle table `IrModule::method_ret_class` (`"Classe_méthode"` →
nom de classe retournée), peuplée dans `lower_program` exactement comme
`func_ret_class` l'est déjà pour les fonctions libres (même helper partagé
`concrete_return_class`, qui gère en plus `Type::Generic`, absent de l'ancien
`func_ret_class`) — statique ET d'instance, avec propagation à travers
l'héritage (méthode non surchargée). Toute la famille de blocs `match`
dupliqués/plafonnés (`lower.rs` ×2, `typeinfer.rs` ×2) est remplacée par une
seule fonction récursive partagée, `resolve_receiver_class` (nouvelle,
`src/lower/expr.d/helpers.rs`), qui gère `Expr::Ident`/`SelfExpr`/
`ParentExpr`/littéral string/`Expr::New`/`Expr::Field` (récursion mutuelle
avec `resolve_chained_field_class`, mise à jour pour déléguer sa résolution
de base à cette même fonction)/`Expr::Call` avec callee `Expr::Field`
(récursion sur le récepteur interne — le cas manquant)/`Expr::Call` avec
callee `Expr::Ident` (fonction libre, `func_ret_class`)/le raccourci
`HTTPRequest::get/post/...(...)`. `elem_type_after_index`/`is_map_target`
(même fichier) migrées aussi vers cette fonction pour la même raison.

**Vérifié par exécution réelle** (pas seulement compilation) : le repro exact
du ticket (2 niveaux, 3 niveaux avec `.upper()`, ET une chaîne à 4 niveaux à
travers 3 classes différentes pour prouver que la récursion est vraiment non
bornée) — voir `examples/66_chained_call_depth_limit.oc` et son test associé.
5 nouveaux tests Rust (`src/lower/expr.d/tests.rs`) : `method_ret_class`
connaît la classe réelle d'une méthode (utilisateur, propagée à travers
l'héritage, jamais peuplée pour un retour primitif), et le pipeline complet
(HIR) mangle bien vers `"Circle_shapeName"`/`"String_upper"` aux 3ème et 4ème
niveaux plutôt que `"_method_..."`.

Non-régression confirmée : `make build` propre (0 warning), `make tests`
189 passed (184 + 5), `make regression` 827 PASS/0 FAIL (824 + 3) — aucun des
mécanismes partageant ces fichiers (dispatch d'interface, dispatch async
d'instance/statique, `Resolvable<T>`, chaînage sur résultat de fonction
libre) n'a régressé.

Aucun changement de syntaxe/grammaire — `docs/EBNF.md`, l'extension VS Code,
`ocaracs`, `ocaraunit` n'ont pas eu besoin d'être touchés (vérifié, pas
supposé : `ocaracs` exécuté sur les nouveaux fichiers, 0 avertissement).
