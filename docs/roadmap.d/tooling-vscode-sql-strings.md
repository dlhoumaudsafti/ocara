# Extension VS Code : coloration SQL dans les chaînes

Proposé le 2026-10-21. Fonctionnalité nouvelle : ce n'est pas un défaut.

## Existant

- Les gabarits `` `…` `` sont colorés en HTML par un sous-ensemble maison
  (`#template-html` dans `tools/highlight/vsode/syntaxes/ocara.tmLanguage.json`) :
  balises, attributs, commentaires, entités. Chaque région s'arrête au `` ` ``
  fermant. `text.html.basic` a été abandonné parce qu'il avalait la suite du
  fichier sur `<!-- ${x}` ou sur un attribut non fermé.
- Les chaînes `"…"` et `'…'` (`#string-double`, `#string-single`) ne colorent
  que les échappements.
- Le SQL est passé en chaîne à `SQLite` et `MySQL` : `execute`, `query`,
  `queryOne`, `prepare` (`docs/builtins/SQLite.md`, `docs/builtins/MySQL.md`).
  Ses paramètres sont nommés (`:nom`), il n'y a pas de `?`.

Aujourd'hui une requête s'affiche comme une chaîne uniforme.

## Proposition

### 1. Détection

Une chaîne (`"…"`, `'…'` ou gabarit) est traitée comme du SQL si l'une de ces
règles s'applique :

1. **Premier mot-clé** : après les espaces et sauts de ligne, elle commence
   par `SELECT`, `INSERT`, `UPDATE`, `DELETE`, `CREATE`, `DROP`, `ALTER`,
   `WITH`, `REPLACE`, `PRAGMA`, `BEGIN`, `COMMIT`, `ROLLBACK` ou `VACUUM`,
   **suivi d'un espace ou de la fin de la chaîne**. La casse est ignorée
   (`(?i)`), mais seulement pour ce premier mot. Ainsi `"Select a file"`
   n'est pas reconnu, alors que `"select * from t"` l'est. Cette règle est à
   trancher : exiger la majuscule sur ce premier mot donnerait moins de faux
   positifs.
2. **Position d'argument**, plus fiable : la chaîne est le premier argument
   de `.execute(`, `.query(`, `.queryOne(` ou `.prepare(`. TextMate n'a pas
   de types, donc tout appel de ces noms est concerné. Le motif reconnaît
   `\.(execute|query|queryOne|prepare)\(\s*` suivi d'une chaîne ouvrante.
3. **Marqueur explicite** : un commentaire `/* sql */` placé juste avant la
   chaîne, sur le modèle des extensions JS et Python. Il sert d'échappatoire
   quand les deux premières règles ne suffisent pas.

### 2. Coloration

Comme pour le HTML, on écrit un **sous-ensemble maison** et on n'injecte pas
`source.sql` : la grammaire SQL de VS Code ne connaît pas `${…}` et ne
s'arrête pas sur le délimiteur Ocara. Chaque région doit se terminer au
`"`, `'` ou `` ` `` fermant.

- mots-clés (`keyword.other.sql`) : `SELECT … FROM … WHERE … JOIN … ON …
  GROUP BY … ORDER BY … LIMIT … VALUES … SET … INTO … AND … OR … NOT …
  NULL … AS …`, insensibles à la casse ;
- types (`storage.type.sql`) : `INTEGER`, `TEXT`, `REAL`, `BLOB`,
  `VARCHAR(n)`, `PRIMARY KEY`… ;
- fonctions (`support.function.sql`) : `COUNT`, `SUM`, `MAX`, `MIN`, `AVG`,
  `COALESCE`, `NOW`… ;
- paramètres nommés `:nom` (`variable.parameter.sql`), à distinguer
  nettement : c'est le seul moyen d'injecter une valeur sans risque ;
- littéraux internes `'…'` dans une chaîne `"…"` (`string.quoted.single.sql`)
  et nombres (`constant.numeric.sql`) ;
- commentaires `-- …` jusqu'à la fin de la ligne **ou jusqu'au délimiteur
  fermant**, et `/* … */` ;
- dans un gabarit, `${…}` garde sa coloration Ocara (`#template-expression`)
  et passe **avant** les règles SQL.

Les échappements Ocara (`#string-escape`) gardent la priorité dans tous les
cas.

### 3. Avertissement facultatif (hors grammaire)

Un `${…}` interpolé dans une chaîne reconnue comme SQL est une injection
potentielle (`docs/builtins/SQLite.md`, « Requêtes paramétrées »). On peut
le signaler par un diagnostic d'information dans l'extension ou dans le
serveur de langage, avec une correction rapide qui propose `:nom`. Ce point
est à décider à part de la coloration.

### 4. Réglage

`ocara.highlight.sqlStrings` : `"auto"` (règles 1 à 3), `"arguments"`
(règles 2 et 3 seulement) ou `"off"`.

TextMate ne lit pas les réglages, donc il faut l'une de ces deux approches :

- deux grammaires injectées (`injectionSelector`) que l'on active ou non ;
- une seule règle fixe, sans réglage.

À trancher.

## Points d'attention

- **Faux positifs** de la règle 1 sur du texte en anglais (`"Update
  available"`). D'où l'exigence d'un espace puis d'un deuxième mot SQL
  attendu (`UPDATE x SET`, `DELETE FROM`, `INSERT INTO`, `SELECT …`), ou
  bien la majuscule obligatoire.
- **Chaînes sur plusieurs lignes** : TextMate gère les régions
  `begin`/`end`, mais une détection par le premier mot doit tolérer un saut
  de ligne juste après le guillemet.
- **Ordre d'inclusion** dans `#string-template` : `#string-escape`, puis
  `#template-expression`, puis la région SQL si elle est détectée, sinon
  `#template-html`. Une chaîne ne doit jamais être colorée à la fois en HTML
  et en SQL.
- **Parité JetBrains** : la grammaire TextMate est réutilisée
  (`roadmap.d/tooling-jetbrains-plugin.md`), donc le gain vaut pour les deux
  extensions.
- **Tests** : des fichiers d'exemple avec des cas positifs et négatifs
  (`vscode-tmgrammar-test` ou des captures de scopes).

## Priorité / Complexité

**Basse** (confort d'édition) — **Légère** (règles TextMate et tests de
scopes). Le diagnostic d'injection (point 3) relève du serveur de langage.

## Fichiers clés

`tools/highlight/vsode/syntaxes/ocara.tmLanguage.json` (`#string-template`,
`#string-double`, `#string-single`, nouveau `#embedded-sql`),
`tools/highlight/vsode/package.json` (`embeddedLanguages`, réglage),
`docs/builtins/SQLite.md`, `docs/builtins/MySQL.md`.
