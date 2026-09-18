# Réflexion (non tranchée) : permettre une `property` de type ressource sur une classe utilisateur

## Constat

Aujourd'hui, une `property` de type `Mutex`/`SQLite`/`MySQL`/`MariaDB` sur une classe utilisateur est **rejetée à la compilation** (diagnostic E29, `docs/diagnostics.md` §E29) :

```
fichier.oc:5:5: error: 'Cache.lock' ('Mutex') is a native resource field — it is never closed when a 'Cache' instance is destroyed (no mechanism exists for this today), so this handle always leaks; manage it outside the class instead, or expose an explicit method the caller must invoke before discarding the instance
```

Raison : `__free_<Classe>` (généré pour une instance `scoped`/`consumed`, ou un `var` dont l'échappement est prouvé impossible — voir `docs/EBNF.md` §9) sait libérer un champ `string`/`array`/`map`/instance de classe utilisateur, jamais fermer une ressource native. Sans mécanisme dédié, ce champ fuirait systématiquement son handle natif à chaque libération de l'instance porteuse.

Cas concret rencontré : `examples/advanced/tauri_httpserver/configs/Database.oc` voudrait garder une connexion `SQLite` ouverte comme champ d'instance (`private property db:SQLite`, ouverte une fois dans `init()`, réutilisée par `migrate()`/`recordVisit()`) — pattern d'encapsulation courant (« la classe possède sa ressource »). C'est aujourd'hui rejeté ; le contournement documenté par E29 (gérer la ressource hors de la classe, ou rouvrir une connexion à chaque méthode) fonctionne mais casse l'encapsulation voulue.

## Pourquoi ce n'est pas trivial

Le problème n'est pas seulement "ajouter un appel `.close()` dans `__free_<Classe>`" : il faut d'abord garantir que `__free_<Classe>` sera **effectivement appelée** avant que le handle ne devienne inaccessible, ce qui dépend de comment l'INSTANCE elle-même est possédée — exactement la même question déjà tranchée pour une ressource nue (`scoped`/`consumed` obligatoire, `var`/`const` rejeté si l'échappement ne peut pas être prouvé impossible, voir E28) :

- Une instance `scoped`/`consumed` de la classe : cas facile, même mécanisme de fin de bloc que pour une ressource nue.
- Une instance `var`/`const` dont l'échappement est prouvé impossible (`var_never_escapes`) : devrait pouvoir bénéficier du même auto-free — mais `var_never_escapes` n'a jamais été audité pour un champ de type ressource CONTENU dans l'objet, seulement pour l'objet lui-même.
- Une instance qui s'échappe réellement (stockée dans un champ d'une autre classe, retournée, passée en argument retenu...) : aujourd'hui, ce cas resterait un vrai problème ouvert — soit on le rejette (même esprit que E28 pour une ressource nue non prouvée non-échappante), soit on assume la fuite et on documente la limite.

## Pistes (aucune tranchée)

1. **Réutiliser l'analyse d'échappement existante** (`var_never_escapes`, `crate::sema::escape`) : autoriser une `property` ressource UNIQUEMENT sur une classe dont CHAQUE site d'instanciation est prouvé non-échappant (`scoped`/`consumed`, ou `var` non-échappant) — sinon rejeter avec un message pointant le site d'échappement précis (comme E28 aujourd'hui). Rejoint le mécanisme déjà en place, mais demande d'étendre `__free_<Classe>` pour émettre `.close()`/`.destroy()` sur les champs ressource, et de faire remonter l'analyse au niveau du champ (pas seulement de la variable).
2. **Ne rien changer au langage, améliorer seulement la pédagogie** : garder E29 tel quel, mais documenter clairement le patron recommandé (méthode `close()` explicite que l'appelant doit invoquer, comme suggéré par le message d'erreur lui-même) avec un exemple complet dans `docs/EBNF.md`/`docs/diagnostics.md`. Option à coût quasi nul, mais qui n'apporte rien de neuf au langage.
3. **Détecteur explicite `close()`/`destroy()` généré automatiquement** sur la classe porteuse (une méthode de convention, ex. `__close()`, qui ferme tous les champs ressource déclarés) que le compilateur exige d'appeler avant la fin de vie prouvée de l'instance — proche de l'esprit de `Thread` (`.join()`/`.detach()` obligatoire, E19), mais appliqué à une classe entière plutôt qu'à un type natif unique.

## Priorité / Complexité

**Priorité Moyenne** — n'affecte aucun programme existant (E29 rejette déjà ce pattern, aucune régression possible), mais bloque un patron d'encapsulation courant (« la classe possède sa ressource ») sans bonne alternative aujourd'hui à part rouvrir la ressource à chaque appel ou casser l'encapsulation.
**Complexité : Structurel** — touche l'analyse d'échappement, la génération de `__free_<Classe>`, et potentiellement un nouveau diagnostic (site d'échappement d'une instance porteuse de ressource) ; zone sensible (mémoire/ressources natives), à traiter avec les mêmes précautions que E18/E28 (tests Rust dédiés + exemple de régression avant tout merge).

## Fichiers clés

`docs/diagnostics.md` (§E29), `docs/EBNF.md` (§9.2/9.3, ownership `scoped`/`consumed`/`var`), `src/sema/escape.rs` (`var_never_escapes`, à étendre pour un champ ressource), génération de `__free_<Classe>` (lowering des classes), `examples/advanced/tauri_httpserver/configs/Database.oc` (cas d'usage concret qui a motivé cette fiche).
