# Suite différée de `securite-lien-no-pie` — activer `is_pic` côté Cranelift pour un vrai PIE

## Constat

[securite-lien-no-pie](securite-lien-no-pie.md) est clos : `-no-pie` reste au lien final, décision justifiée et documentée (retirer `-no-pie` produit un PIE avec `TEXTREL`, pas un vrai gain de sécurité, parce que Cranelift n'émet jamais de code indépendant de la position — `settings::Flags::new` ne met jamais `is_pic` à `true`, `src/codegen/emit.d/emitter.rs`). Ce ticket-ci **n'annule pas** cette conclusion.

Ce qu'il reprend, c'est le chantier que le ticket clos identifie explicitement mais refuse volontairement d'ouvrir tout de suite : *« La vraie correction (un binaire PIE sans `TEXTREL`) demanderait d'activer `is_pic = true` côté Cranelift — un chantier bien plus large qu'un flag de lien [...] hors du périmètre "Simple" de ce ticket. Pas de nouvelle fiche ouverte pour ça tant qu'aucun besoin concret ne l'exige. »* Ce ticket est cette fiche — pour que le suivi existe et ne se reperde pas, pas parce qu'un besoin concret est apparu depuis.

Sans ASLR (conséquence actuelle de `-no-pie`), un binaire produit par Ocara offre une protection en profondeur plus faible qu'un exécutable lié PIE par défaut (comportement standard des toolchains gcc/clang récentes) en cas de bug mémoire exploitable côté runtime (`mixed`, boxing — voir [memoire-boxing-durcissement](memoire-boxing-durcissement.md)). Ce n'est pas un bug fonctionnel — le langage n'a pas de GC et une base de code bas niveau qu'on veut, par construction, durcir plutôt que fragiliser davantage.

## Ce qui est demandé

Investiguer l'activation de `is_pic = true` dans `settings::Flags::new` (Cranelift, `src/codegen/emit.d/emitter.rs`) : impact sur tout l'adressage émis par le codegen (globales, appels, chaînes littérales) — c'est précisément ce que le ticket clos qualifie de "chantier bien plus large qu'un flag de lien". Vérifier ensuite avec `readelf -d`/`file` que le binaire résultant est un PIE **sans** `TEXTREL`, puis retirer `-no-pie` (`src/codegen/link.rs:127`) une fois cette condition remplie — `make regression` doit rester à 100% vert.

## Priorité / Complexité

**Priorité Très Basse** — défense en profondeur, pas un bug fonctionnel ; aucune urgence tant qu'aucun incident concret ne l'exige, exactement comme le ticket clos le concluait déjà. **Complexité Dangereuse** — touche l'émission d'adresses dans tout le codegen (Cranelift), zone sensible où une erreur peut casser silencieusement des binaires qui fonctionnaient auparavant ; à traiter avec de bons tests de non-régression (`make regression` avant/après sur l'ensemble du corpus d'exemples, pas seulement un sous-ensemble).

## Fichiers clés

`src/codegen/emit.d/emitter.rs` (`settings::Flags::new`), `src/codegen/link.rs:127` (`-no-pie`), [securite-lien-no-pie](securite-lien-no-pie.md) (ticket clos, contexte complet de la décision actuelle).
