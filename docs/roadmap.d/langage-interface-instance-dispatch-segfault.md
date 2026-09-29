# SIGSEGV : appeler une méthode d'instance sur une variable typée par une interface

Vérifié :
- **Reproduit et confirmé indépendamment par backtrace gdb** (`gdb -q -batch -ex run -ex bt`), pas seulement par l'exemple du ticket : le crash ne se produit PAS dans le dispatch dynamique lui-même — `bt` pointe vers `ocara_runtime::val_to_string` (appelé depuis `IO_writeln`, lui-même depuis `main`), pas vers `Shape_area` ni `Circle_area`. Le dispatch par identité de classe (`__class_id`, voir `generate_one_dispatcher`) identifiait déjà correctement `Circle` et calculait déjà la bonne valeur `12.56` — le dispatcher lui-même n'a jamais été le problème.
- **Root cause confirmée par inspection directe du HIR** (`ocara build --dump`) : le site d'appel de `s.area()` dans `main` émettait `Call { func: "Shape_area", args: [...], ret_ty: Ptr }`, alors que la fonction `Shape_area` réellement générée (le dispatcher, `generate_interface_dispatchers`, `src/lower/builder.d/interfaces.rs`) a un VRAI type de retour `F64` (`area(): float`). Cause : `fn_ret_types` (`src/lower/builder.d/program.rs`, la table consultée par le site d'appel pour connaître le type de retour de la fonction ciblée) n'était peuplée que depuis `program.classes` (mangled `"Classe_méthode"`) — jamais depuis `program.interfaces` (`"Interface_méthode"`, ex. `"Shape_area"`). Le lookup retombait donc sur le défaut `IrType::Ptr`, en désaccord avec le VRAI retour `F64` du dispatcher — un désaccord de convention d'appel Cranelift (registre entier `RAX` vs registre flottant `XMM0`) qui produit une valeur totalement arbitraire au site d'appel, ensuite passée telle quelle à `IO::writeln`/`val_to_string`, qui la déréférence comme un pointeur → SIGSEGV.
- **La piste initiale (notée dans ce ticket avant investigation) était partiellement erronée, et le report du coordinateur avait raison de demander à la vérifier plutôt que de la prendre pour acquise** : `class_dispatcher_name`/`classes_with_subclasses` (héritage de classe) ne reconnaît en effet jamais les interfaces — mais ce n'était PAS un problème ici, parce que `class_dispatcher_name("Shape", "area")` renvoie simplement `None` pour un nom d'interface (pas une classe avec sous-classes), et le site d'appel retombe alors DÉJÀ correctement sur `func_mangled = "Shape_area"` tel quel (`src/lower/expr.d/lower.rs`, variable `call_target`) — qui EST le dispatcher réel généré par `generate_interface_dispatchers`, un mécanisme préexistant (`docs/roadmap.d/langage-interfaces.md`, PAS introduit par `wiring` comme le ticket `wiring` l'affirmait à tort) déjà entièrement branché sur ce chemin d'appel. Aucun second mécanisme de dispatch n'a donc été nécessaire — seule la PROPAGATION DE MÉTADONNÉES (type de retour, et types de paramètres pour le boxing `mixed`) entre la génération du dispatcher et le site d'appel était manquante.
- **Corrigé en une seule fois pour toutes les tables de métadonnées concernées**, en miroir exact du traitement déjà existant pour `program.classes` (`src/lower/builder.d/program.rs`) : `fn_ret_types` (type de retour — la cause directe du SIGSEGV), `module.method_param_types` (types de paramètres d'une méthode d'instance, décisif pour le boxing `mixed` d'un argument — voir `box_arg_for_mixed_param`), `fn_param_types`/`fn_param_names`/`fn_variadic_info`/`func_default_args` (branche `is_static`, jamais atteinte en pratique pour un appel via le nom nu d'une interface — `wiring` résout déjà tout bare `Interface::méthode()` vers un nom concret avant le lowering, et un appel sans `wiring` est rejeté par E38 — mais peuplée quand même par souci de cohérence/défense en profondeur), tous désormais peuplés aussi depuis `program.interfaces`, pas seulement `program.classes`.
- **Cas généraux couverts et vérifiés, pas seulement l'exemple à deux classes du ticket** : plusieurs classes implémentant la même interface (chaque instance dispatche vers SA propre méthode, jamais celle d'une autre — vérifié par les VALEURS retournées, pas seulement l'absence de crash) ; une classe implémentant plusieurs interfaces (chaque interface obtient son propre dispatcher, tous deux ciblant correctement la même classe concrète) ; un `array<Interface>` mêlant plusieurs classes concrètes itéré par `for` ; un type concret choisi UNIQUEMENT à l'exécution (`if`/paramètre de fonction, `pickShape(kind:int): Shape`) — le seul cas où un vrai dispatch par identité de classe est réellement incontournable, contrairement à `wiring` dont la substitution est toujours résolue à la compilation.
- **Bug distinct, sans rapport, découvert en écrivant les tests de cas généraux ci-dessus — délibérément NON corrigé ici (hors périmètre)** : chaîner un appel de méthode directement sur le résultat d'une fonction LIBRE (`pickShape(0).shapeName()`) retourne silencieusement `null` (PAS un crash) — confirmé reproductible avec une classe CONCRÈTE ordinaire, sans la moindre interface en jeu (`pickCircle(0).shapeName()` où `pickCircle(): Circle`). Inspection du HIR : le site d'appel mangle vers `"_method_shapeName"` (sans le moindre préfixe de classe, un symbole qui n'existe jamais), exactement la même famille de bug déjà documentée ailleurs dans `src/lower/expr.d/lower.rs` pour d'autres formes de récepteur chaîné non reconnues (`Expr::New`, `HTTPRequest::get(...)`, voir les commentaires existants sur `resolve_chained_field_class`) — juste un cas de plus ("appel de fonction libre en position de récepteur chaîné") que ce correctif-là avait laissé de côté. Contourné dans les tests en capturant le résultat dans une variable avant d'appeler la méthode (`var s:Shape = pickShape(0); s.methode()`, motif déjà correct). Recommandation : ticket dédié séparé, distinct de celui-ci.
- **6 nouveaux tests Rust** (`src/lower/builder.d/interfaces.rs`, module `tests` existant) : le site d'appel généré pour `s.area()` a bien le même `ret_ty` (`F64`) que le VRAI dispatcher (test qui cible directement la cause du SIGSEGV) ; le dispatcher contient bien une branche par implémenteur pour 3 classes différentes ; une classe implémentant 2 interfaces obtient bien 2 dispatchers distincts, chacun ciblant correctement la classe concrète ; `module.method_param_types` connaît bien la signature d'une méthode d'interface avec un paramètre `mixed`. `cargo test --bin ocara --release` : **168 passed, 0 failed** (164 avant ce ticket + 4 nouveaux — les 2 tests déjà présents dans ce module, du ticket `wiring`, continuent de passer sans changement). `cargo test -p ocara_runtime --release` : 105 passed, 7 ignored (inchangé, aucun fichier runtime touché).
- **Exemple de régression bout-en-bout** : `examples/61_interface_instance_dispatch.oc` (illustratif, exécutable seul) et `examples/tests/61_interface_instance_dispatchTest.oc` (16 assertions : cas exact du ticket, plusieurs implémenteurs dispatchant chacun vers sa propre méthode, `array<Interface>` itéré, classe à interfaces multiples, type concret choisi à l'exécution). `examples/advanced/mini_project`/`mini_project_hexa` explicitement PAS touchés (contrainte du ticket).
- `make build` (les 4 crates) + `RUSTFLAGS="-D warnings"` : 0 warning. `./ci/regression.sh` : tous verts. `./ci/unittests.sh examples/project/tests` : 50 PASS / 0 FAIL (inchangé). `./ci/unittests.sh examples/tests` : **794 PASS / 0 FAIL, 0 ERREUR(S)** (778 avant ce ticket, +16 nouvelles assertions dans le nouveau fichier 61).

## Constat (ticket original)

Trouvé en marge du chantier [langage-interface-wiring](langage-interface-wiring.md)
(confirmé orthogonal à `wiring` lui-même — `wiring` ne produit jamais de
binding interface-typé, sa substitution résout toujours vers un type
CONCRET avant que le reste du pipeline ne s'en mêle) — un cas de
polymorphisme d'interface pourtant déjà documenté comme fonctionnel
(`implements`, existant bien avant `wiring`) segfaultait dès qu'on appelait
une méthode d'instance à travers le type interface :

```ocara
interface Shape {
    method area(): float
}
class Circle implements Shape {
    public property r:float
    init(r:float) { self.r = r }
    public method area(): float { return 3.14 * self.r * self.r }
}
function main(): int {
    const c:Circle = use Circle(2.0)
    var s:Shape = c        // affectation valide : Circle implémente Shape
    IO::writeln(s.area())  // SIGSEGV
    return 0
}
```

## Priorité / Complexité

**Terminé.** Complexité réelle bien plus faible que redoutée à l'ouverture
du ticket : pas un problème de dispatch manquant (le mécanisme réel,
`generate_interface_dispatchers`, existait déjà et était déjà branché sur ce
chemin d'appel — préexistant à `wiring`, pas un sous-produit de ce chantier
comme supposé à tort) mais un pur problème de PROPAGATION DE MÉTADONNÉES
(type de retour, types de paramètres) entre la génération de ce dispatcher
et le site d'appel qui le cible — un correctif localisé (`src/lower/builder.d/program.rs`),
sans toucher au mécanisme de dispatch lui-même. Aucun second système de
dispatch dupliqué. Un bug distinct et sans rapport (chaîner un appel de
méthode sur le résultat d'une fonction libre) a été découvert en testant les
cas généraux et documenté ci-dessus comme hors périmètre, pas silencieusement
laissé de côté.

## Fichiers clés

`src/lower/builder.d/program.rs` (population de `fn_ret_types`/
`module.method_param_types`/`fn_param_types`/`fn_param_names`/
`fn_variadic_info`/`func_default_args` étendue à `program.interfaces`),
`src/lower/builder.d/interfaces.rs` (`generate_interface_dispatchers` —
mécanisme préexistant, non modifié, seulement mieux testé — module `tests`
étendu de 4 nouveaux tests), `src/lower/expr.d/lower.rs`
(`class_dispatcher_name`/`call_target` — lu et compris, non modifié : déjà
correct), `examples/61_interface_instance_dispatch.oc` (nouveau),
`examples/tests/61_interface_instance_dispatchTest.oc` (nouveau).
