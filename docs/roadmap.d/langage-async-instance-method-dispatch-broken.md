# SIGSEGV : appeler une méthode D'INSTANCE `async` via le sucre `obj.methode()`

Vérifié :
- **Root cause confirmée toujours exacte** en relisant ce ticket avant de coder quoi que ce soit (comme demandé) : `src/lower/expr.d/lower.rs`, le bras `Expr::Call { callee: Expr::Field { .. } }` (sucre d'instance) émettait TOUJOURS son `Inst::Call` directement, sans jamais consulter `builder.async_funcs` — contrairement aux deux autres formes d'appel (fonction libre, `Classe::methode()` statique), qui le font déjà correctement.
- **Corrigé en réutilisant EXACTEMENT le même mécanisme d'empaquetage/spawn déjà en place** pour les deux autres formes d'appel — pas un second mécanisme : `src/lower/expr.d/lower.rs`, juste avant l'émission de l'`Inst::Call` direct (une fois `call_target` déjà résolu, dispatcher dynamique inclus), un nouveau garde `if builder.async_funcs.contains(call_target.as_str())` empaquette `all_args` (qui contient déjà `self` en premier — `obj_val` — suivi des arguments réels déjà boxés pour `mixed`, EXACTEMENT l'ordre attendu par le wrapper) dans un environnement heap et spawn `__async_wrap_<call_target>` via `__task_spawn`, retournant le task handle au lieu d'appeler la méthode directement.
- **`self` intégré à l'environnement de spawn sans changement à `generate_async_wrapper`** (`src/lower/builder.d/wrappers.rs`) : cette fonction générique ne se soucie jamais de QUI elle appelle, seulement de la signature (`param_tys`) — `src/lower/builder.d/program.rs` génère déjà, depuis avant ce ticket, le wrapper d'une méthode d'instance avec `self` (`IrType::Ptr`) en premier type de paramètre (ligne ~818, "Méthode d'instance : self en premier"). Le seul manque réel était le SITE D'APPEL qui ne spawnait jamais rien pour cette forme — la génération du wrapper lui-même était déjà correcte et prête à être utilisée.
- **Dispatch dynamique résolu SANS second mécanisme parallèle, confirmant l'intuition de départ** : le dispatcher synchrone déjà existant (`__dispatch_Classe_méthode` pour l'héritage — `class_dispatch.rs` — et `Interface_méthode` pour une interface — `interfaces.rs`) reste une fonction 100% SYNCHRONE, qui choisit déjà la bonne implémentation concrète par `__class_id` puis l'appelle directement — exactement comme pour une méthode non-async. Il suffisait de générer AUSSI un wrapper async POUR CE DISPATCHER LUI-MÊME (`__async_wrap___dispatch_Classe_méthode`/`__async_wrap_Interface_méthode`), et de peupler `async_funcs` avec son nom (`src/lower/builder.d/program.rs`, deux nouvelles boucles : une juste après `compute_classes_with_subclasses` pour peupler `async_funcs` — nécessaire AVANT le lowering du moindre corps, même contrainte d'ordre que `classes_with_subclasses` — et une juste après `generate_class_dispatchers`/`generate_interface_dispatchers` pour générer les wrappers, une fois les dispatchers eux-mêmes lowered). `callable_instance_method_names`/`find_method_decl` (`class_dispatch.rs`) rendues `pub` pour cet usage cross-module.
- **Scope général vérifié, pas seulement le repro minimal du ticket** (les 4 cas demandés) :
  - Classe concrète ordinaire (le repro exact) — `f.fetch(21)` → `42`.
  - Sous-classe qui surcharge une méthode `async` héritée — `var a:Animal = use Dog(); a.soundCode()` → `7` (Dog, pas Animal) ; la classe de base seule (`use Animal()`) continue de fonctionner (`0`).
  - Interface `wiring`avec un contrat D'INSTANCE `async` — le scénario qui a mené à la découverte du bug (`langage-interface-method-modifiers`) — `var r:Repo = use Repo(); r.fetchCode()` → `99`, résolu vers `ConcreteRepo` via le premier `wiring`.
  - Type concret choisi UNIQUEMENT à l'exécution (paramètre de fonction) derrière un récepteur interface-typé — le seul cas où un vrai dispatch par identité de classe est incontournable, `wiring` ne produisant jamais ce genre de binding : `pickTalker(0)` → `Cat` → `1`, `pickTalker(1)` → `Robot` → `2`, toutes deux via le MÊME dispatcher partagé `Talker_speakCode`.
- **Bug SÉPARÉ et préexistant découvert en testant le scope général avec un type de retour non-`int` (`string`), délibérément NON corrigé ici** : `async method`/`async function` dont le retour déclaré n'est pas `int` casse la vérification de types — reproduit aussi bien pour un appel STATIQUE (`Classe::methode(): string`) que D'INSTANCE, donc SANS RAPPORT avec le bug de ce ticket (qui est un problème de CODEGEN, celui-ci est un problème de SEMA). Cause localisée : `src/sema/typecheck.rs` n'applique la règle « un appel `async` retourne `Type::Int` » qu'à un seul endroit (fonction libre, ligne ~1161) — ni `Expr::StaticCall` ni le sucre d'instance ne l'ont. Toutes les démonstrations `async` préexistantes dans ce compilateur utilisaient `int` comme retour, masquant ce bug par coïncidence (type réel == type task handle). Documenté séparément dans [langage-async-non-int-return-type-check](langage-async-non-int-return-type-check.md), ajouté à `docs/roadmap.md`. Les tests/exemples de CE ticket utilisent `int` partout pour rester dans le périmètre du codegen corrigé ici.
- **Confirmé, comme demandé** : aucun changement de grammaire (`docs/EBNF.md`) — un pur correctif de lowering/codegen, aucune nouvelle syntaxe. Aucun nouveau diagnostic (`docs/diagnostics.md`) — le programme compilait déjà (silencieusement faux avant, jamais un rejet).
- **5 nouveaux tests Rust** (`src/lower/expr.d/tests.rs`) : le repro exact spawn bien `__task_spawn` via `__async_wrap_DoublingFetcher_fetch` (jamais un appel direct à `DoublingFetcher_fetch`) ; non-régression — une méthode d'instance NON-async continue d'être appelée directement, jamais spawnée ; sous-classe — le wrapper spawné est celui du DISPATCHER (`__async_wrap___dispatch_Animal_soundCode`), qui appelle bien le dispatcher synchrone (lui-même vérifié appeler `Dog_soundCode` ET `Animal_soundCode`, non-régression du mécanisme préexistant) ; interface `wiring` — le wrapper spawné est celui du dispatcher d'interface (`__async_wrap_Repo_fetchCode`), qui appelle `Repo_fetchCode` (interface), qui appelle `ConcreteRepo_fetchCode` ; type concret runtime-déterminé — le dispatcher partagé `Talker_speakCode` connaît bien les DEUX implémenteurs (`Cat_speakCode` ET `Robot_speakCode`). `cargo test --bin ocara --release` : **184 passed, 0 failed** (179 avant ce ticket + 5).
- **Exemple de régression bout-en-bout** : `examples/64_async_instance_method_dispatch.oc` (illustratif, les 4 scénarios) et `examples/tests/64_async_instance_method_dispatchTest.oc` (7 assertions, valeurs vérifiées après `resolve`, pas seulement l'absence de crash). `examples/advanced/mini_project`/`mini_project_hexa` explicitement PAS touchés.
- `make build` (les 4 crates) + `RUSTFLAGS="-D warnings"` : 0 warning. `./ci/regression.sh` : tous verts. `./ci/unittests.sh examples/project/tests` : 50 PASS / 0 FAIL (inchangé). `./ci/unittests.sh examples/tests` : **819 PASS / 0 FAIL, 0 ERREUR(S)** (812 avant ce ticket, +7 nouvelles assertions).

## Constat (ticket original)

Trouvé en marge de [langage-interface-method-modifiers](langage-interface-method-modifiers.md)
en essayant d'exercer `async` sur une méthode d'interface D'INSTANCE dans un
test de régression. Reproduit d'abord avec une interface, puis ISOLÉ à une
classe concrète ordinaire, sans la moindre interface :

```ocara
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

Confirmé par gdb : crash dans `__task_resolve`. Confirmé par `ocara build
--dump` (HIR) : le site d'appel émettait un `Inst::Call` DIRECT vers la
fonction synchrone réelle, jamais vers `__async_wrap_.../__task_spawn`.
`static async method` (appelée via `Classe::methode()`) fonctionnait déjà
correctement.

## Priorité / Complexité

**Terminé.** Confirmé **Structurel** comme anticipé — mais nettement moins
qu'un second mécanisme de dispatch complet, comme redouté à l'ouverture :
le dispatch dynamique (héritage ET interface) était déjà entièrement
correct et réutilisable tel quel, il ne manquait que (1) le spawn de tâche
au site d'appel d'instance, en tout point identique à celui déjà écrit pour
`Classe::methode()`/`parent::methode()`, et (2) générer AUSSI un wrapper
async pour les fonctions dispatcher elles-mêmes — jamais une seconde copie
du dispatch. Même principe que le correctif précédent
([langage-interface-instance-dispatch-segfault](langage-interface-instance-dispatch-segfault.md)) :
le mécanisme existant était déjà correctement câblé, seule la propagation
de métadonnées (ici, `async_funcs` pour un nom de dispatcher) manquait. Un
bug SÉPARÉ et préexistant (sema, `async` + retour non-`int`) a été trouvé
en testant le scope général et documenté à part plutôt que traité ici.

## Fichiers clés

`src/lower/expr.d/lower.rs` (nouveau garde `async_funcs`/empaquetage/spawn
dans le bras `Expr::Call { callee: Expr::Field }`), `src/lower/builder.d/program.rs`
(population de `async_funcs` avec les noms de dispatcher async, génération
de leurs wrappers), `src/lower/builder.d/class_dispatch.rs`
(`callable_instance_method_names`/`find_method_decl` rendues `pub`),
`src/lower/builder.d/wrappers.rs` (`generate_async_wrapper` — lu, non
modifié, déjà correct pour une méthode d'instance), `src/lower/expr.d/tests.rs`
(5 nouveaux tests), `examples/64_async_instance_method_dispatch.oc`,
`examples/tests/64_async_instance_method_dispatchTest.oc`.
