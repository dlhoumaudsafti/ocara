# Cohérence interne de l'EBNF et de la stdlib — la doc grandit par accrétion sans passe de relecture

## Constat général

Le noyau du langage est cohérent en pratique (tous les exemples lus utilisent la même syntaxe sans divergence). C'est la **documentation de référence** (`docs/EBNF.md`, `docs/builtins/*.md`) qui accumule des incohérences internes au fil des sections ajoutées une à une, sans passe de convergence finale — aucune ne casse le compilateur, mais chacune érode la confiance qu'on peut accorder à la spec comme source de vérité, ce qui compte pour la stabilité perçue du langage autant que l'absence de bugs runtime.

## EBNF — incohérences internes vérifiées

1. **`Program` a deux définitions différentes dans le même fichier.** `docs/EBNF.md:79-83` (§2, "Structure d'un programme") inclut `GenericDecl` dans l'alternation top-level. `docs/EBNF.md:3429-3434` (§31, "Grammaire EBNF complète" — censée être LA référence canonique) **l'omet**. `GenericDecl` est bien définie deux fois (`docs/EBNF.md:2422` et `:3462`) mais devient, à la lettre de la grammaire "complète", inaccessible depuis `Program`.
2. **`ModuleDecl`** est référencée dans `Program` (les deux sections) mais sa règle (`docs/EBNF.md:2329`, `"module" Identifier ClassBody`) n'est définie qu'en §19 — jamais rapatriée dans la section "grammaire complète", contrairement à la plupart des autres règles qui y sont dupliquées.
3. **Noms de règles instables entre sections** : §27.2/27.3 nomment deux règles distinctes `ForInStmt` (`docs/EBNF.md:3152`) et `ForMapStmt` (`:3164`) ; §31 les fusionne en une seule règle `ForStmt` à deux alternatives (`:3558-3560`) sans jamais utiliser ces deux noms.
4. **`Function<T(...)>` documenté comme obligatoire, contredit par 10 exemples.** §6.5 (`docs/EBNF.md:897-899`) : *« La syntaxe avec parenthèses est **obligatoire** depuis la version 0.1.0 »*, avec une grammaire stricte (`docs/EBNF.md:894`) qui n'admet que `Function<Type(...)>`. Pourtant `Function` **nu, sans aucun générique**, est utilisé comme type dans au moins 10 exemples des sections voisines (`docs/EBNF.md:1746,1755,1770,1775,1786,1799,1822,1828,1835,3014-3015` — §14.3 "Fonctions de première classe", §16.6, §23). La note historique (`:907`) ne documente que la suppression de l'ancienne forme `Function<ReturnType>` sans parenthèses, jamais celle de la forme totalement nue — les exemples n'ont simplement jamais été mis à jour après le durcissement de syntaxe de §6.5.

## Stdlib — incohérences sémantiques entre modules jumeaux

1. **`Map::get` vs le reste de `Map.md`** : la section propre de `Map::get` (`docs/builtins/Map.md:55-60`) dit *« Si la clé est absente, le comportement dépend du runtime (retourne `0` / chaîne vide par défaut) »* — formulation vague qui ne s'engage même pas sur un comportement précis. Plus loin (`docs/builtins/Map.md:363-368`), la doc énumère les méthodes qui *« ne lèvent jamais d'exception »* (`has`, `set`, `remove`, `size`/`isEmpty`, `keys`/`values`, `merge`) — `get` est absente de cette liste, ce qui laisse penser par omission qu'elle PEUT lever une exception, sans jamais le confirmer explicitement dans sa propre section. Les deux passages du même document ne racontent pas la même histoire.
2. **`String::replace` vs `Regex::replace` — même nom, sémantique opposée.** `String::replace(s, from, to)` (`docs/builtins/String.md:82-84`) remplace **toutes** les occurrences (exemple vérifié : `"chat noir chat blanc"` → `"chien noir chien blanc"`). `Regex::replace(pattern, s, repl)` (`docs/builtins/Regex.md:60-62`) remplace **uniquement la première** occurrence (exemple vérifié : `"ref-123-abc-456"` → `"ref-NUM-abc-456"`, le second nombre non touché) — `Regex::replaceAll` existe séparément pour l'équivalent de `String::replace`. Un développeur qui connaît `String::replace` et passe à `Regex::replace` en s'attendant au même comportement introduit un bug silencieux.
3. **`queryOne` — sentinelle d'absence différente entre `SQLite` et `MySQL`.** `db.queryOne(query) → map<string, mixed>` (`docs/builtins/SQLite.md:54`) retourne une **map vide** si aucun résultat (`Map::size(user) > 0` dans l'exemple). `db.queryOne(query) → map<string, mixed>|null` (`docs/builtins/MySQL.md:71`) retourne **`null`** si aucun résultat (`user not equal null` dans l'exemple). Ce sont deux builtins jumeaux (même rôle, même nom de méthode) avec deux conventions différentes pour représenter "pas de résultat" — piège garanti pour quiconque écrit du code générique sur les deux. Accessoirement, l'exemple MySQL lui-même (`docs/builtins/MySQL.md:76`) déclare `const user:map<string, mixed> = db.queryOne(...)` — un type non-nullable pour une méthode dont la signature documentée juste au-dessus est `|null` : l'exemple ne correspond pas à sa propre signature.

## Ce qui est demandé

Pas une réécriture — des corrections ciblées et une règle de discipline pour la suite :
- Corriger les 4 points EBNF et les 3 points stdlib listés ci-dessus.
- Ajouter au processus déjà suivi (`docs/roadmap.md` § "Méthode de travail") une étape explicite : après toute modification de l'EBNF, relire §31 ("grammaire complète") pour vérifier qu'elle reste réellement la source unique et à jour — c'est déjà l'intention affichée de cette section, il manque juste la vérification systématique qui l'empêche de dériver.

## Priorité / Complexité

**Priorité Basse** — confort et confiance dans la documentation de référence, aucun de ces points ne cause de bug de compilation ou d'exécution. **Complexité : Simple** — corrections de texte ciblées, pas de changement de comportement du compilateur.

## Fichiers clés

`docs/EBNF.md` (§2, §6.5, §14.3, §16.6, §19, §23, §27, §31), `docs/builtins/Map.md`, `docs/builtins/String.md`, `docs/builtins/Regex.md`, `docs/builtins/SQLite.md`, `docs/builtins/MySQL.md`, `docs/roadmap.md` (méthode de travail).
