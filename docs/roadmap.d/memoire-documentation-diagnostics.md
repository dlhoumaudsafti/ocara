# Trous de documentation et de diagnostics sur le modèle mémoire

## ✅ Documentation — corrigée

- `docs/EBNF.md` §9.1 (`var`) précise maintenant explicitement qu'une variable `var` n'est **jamais** libérée (pas de ramasse-miettes) — auparavant seul le tableau §9.2 sur `scoped` documentait un vrai comportement de libération, `var` n'était jamais mentionné comme ne libérant rien.
- `docs/EBNF.md` §9.2 (`scoped`) documente désormais aussi la limite connue de l'échappement par argument (`scoped`/`consumed` passée en paramètre à une fonction qui la conserve) en plus de la limite déjà documentée sur `raise`/`longjmp` — voir [memoire-echappement-argument](memoire-echappement-argument.md) (non corrigée, seulement documentée).
- `docs/EBNF.md` §1 : "pas de GC imposé" (ambigu, laissait penser à un GC optionnel) reformulé en "aucun ramasse-miettes — jamais, par choix de design définitif".
- `docs/workflow-compilation.md` : nouvelle sous-section 4️⃣d décrivant la phase de lowering qui insère les libérations `scoped`/`consumed`, avec renvoi vers les diagnostics E17-E19 (qui n'étaient mentionnés nulle part dans le seul document d'architecture du projet).
- `README.md` : nouvelle puce dans "Caractéristiques" signalant l'absence de GC dès la page d'entrée du projet.

## ✅ Diagnostic du double-free explicite — corrigé (E25)

Sur les 4 diagnostics manquants identifiés initialement, un est désormais traité : **double-free explicite** sur une ressource (`Mutex`/`SQLite`/`MySQL`/`MariaDB`) — `m.destroy(); m.destroy()` (deux appels MANUELS dans le code source, pas la libération automatique de fin de bloc) provoquait un **SEGFAULT confirmé par reproduction**, sans le moindre diagnostic à la compilation.

**Cause** : le mécanisme existant qui empêchait la libération automatique de fin de bloc de re-libérer une ressource déjà manuellement fermée (`builder.owned_locals[var].dropped = true` après un `.destroy()`/`.close()` reconnu, voir [memoire-double-free-et-fuites-scoped](memoire-double-free-et-fuites-scoped.md)) ne protégeait QUE contre CE cas précis — deux appels manuels explicites dans le code, chacun lowered indépendamment, restaient tous les deux émis sans condition.

**Corrigé** (`src/sema/typecheck.rs`, `src/sema/scope.rs`) : généralisation du mécanisme déjà utilisé pour `Thread.join()`/`.detach()` (E22) — `ScopeStack::mark_thread_finalized` renommé `mark_resource_finalized` (et le champ `LocalBinding::thread_finalized` en `resource_finalized`), et le typecheck d'appel de méthode reconnaît maintenant aussi `Mutex::destroy`/`SQLite::close`/`MySQL::close`/`MariaDB::close` comme finalisations manuelles à suivre — un second appel sur la même variable est rejeté (nouveau diagnostic **E25**).

Vérifié : `m.destroy(); m.destroy()` sur un `Mutex` → rejeté à la compilation (`'m' ('Mutex') was already '.destroy()'ed`) au lieu de SEGFAULT à l'exécution. Nouveau cas ajouté à `examples/21_errors.oc` (déjà suivi par `ci/regression.sh`). `make regression` sans régression (388 PASS / 0 FAIL, 49 PASS / 0 FAIL / 0 ERREUR côté projet).

## Diagnostics restants — toujours bloqués

Les deux diagnostics restants (fuite d'un handle natif déclaré en `var`, fuite d'un champ de classe non pris en charge) nécessitent une vraie stratégie de suivi mémoire pour `var` — aujourd'hui `var` ne libère jamais rien par conception, et aucun mécanisme n'existe pour distinguer "ce `var` contient un handle natif qui fuit sans jamais être fermé" d'un usage parfaitement normal. Voir [memoire-strategie-var](memoire-strategie-var.md) (Haute priorité, Massive) — ces deux diagnostics restent bloqués tant que ce chantier, bien plus large, n'a pas eu lieu.

## Fichiers clés

`src/sema/typecheck.rs` (méthode d'appel `Expr::Field`, `manual_finalizer`), `src/sema/scope.rs` (`ScopeStack::mark_resource_finalized`, `LocalBinding::resource_finalized`), `src/sema/error.rs` (`ResourceAlreadyFinalized`), `examples/21_errors.oc`.
