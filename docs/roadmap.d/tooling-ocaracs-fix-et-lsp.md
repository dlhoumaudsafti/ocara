# ocaracs `--fix`, serveur de langage et extension VS Code : corrections qui cassent le code, états périmés

Audit du 2026-10-19. Reproduit (scripts JSON-RPC pour le serveur, moteur
TextMate de VS Code pour la grammaire).

## ocaracs `--fix` : corrections qui changent le programme

- **CRLF** converti en LF partout, y compris dans les chaînes backtick
  multilignes (`s.len()` passe de 4 à 3) (`fix.rs` ~l.181, 203).
- **Collision** : `userID` et `userId` deviennent tous deux `user_id`
  (`rename.rs` ~l.197-221, pas de contrôle de collision).
- Membres renommés quel que soit le receveur : `isOpen` renommé réécrit aussi
  `win.isOpen()` sur `SDL` (`rename.rs` ~l.232).
- Champ de struct : déclaration renommée, pas les accès `u.userName`
  (~l.298).
- Fonction renommée : import réécrit, fichier non déplacé (~l.349) ; import
  suivi d'un commentaire : fichier renommé, import non (~l.329-352).
- `when` absent de `KEYWORDS` (`When` → `when`) ; `'{'` dans une chaîne entre
  apostrophes fait prendre une constante globale pour une locale
  (`scope.rs` ~l.469).
- Indentation : `*` en fin de SQL backtick ou `import ocara.*` pris pour une
  continuation (`fix.rs` ~l.254, 290) ; `when default function …` sur une
  ligne supprime la ligne vide suivante (`check.rs` ~l.64).
- Analyse par `main.oc` : 16 fichiers sur 107 dans `mini_project_hexa`, sans
  message (imports résolus depuis le dossier du fichier, `runtime … is main`
  non suivi, `import X from "…"` non compris) ; `.oc` non UTF-8 → exit 0 ;
  option mal tapée et erreurs de `.ocaracs` ignorées ; colonnes de R03, R04,
  R10, R11 en octets.

## Serveur de langage

- **Fichier importé modifié (non enregistré) : ses importeurs ne sont pas
  réanalysés** (diagnostics et survol périmés ; `src/lsp/mod.rs` ~l.229 ne
  rafraîchit que le document modifié).
- **Fichier importé fermé sans enregistrer** : erreurs et survol périmés dans
  les importeurs (`DidCloseTextDocument`, ~l.266, n'invalide rien).
- **Soulignement décalé** : erreur sur un membre inconnu placée sur le `.`,
  `word_at` vide, la `)` est soulignée (`features.rs` ~l.37,
  `position.rs` ~l.11) ; message « field 'x' not found » pour un appel de
  méthode.
- **Renommage sans détection de conflit** (vers une locale existante, vers
  `IO`) → `duplicate symbol` après application.
- Mono-thread sans `$/cancelRequest` (la première construction de l'index,
  1 à 1,8 s, bloque tout) ; documents `untitled:` sans diagnostics ; rename
  de fichier envoyé sans vérifier les capacités du client.

## Extension VS Code

- **Benchmark : le délai ne tue pas le programme mesuré** (seul
  `/usr/bin/time` est tué, `benchmark.ts` ~l.323-331) ; code de sortie non nul
  → bilan de fuites et profil jetés ; historique qui mélange un nombre de
  valeurs et des Ko sous le genre `leak` ; résultat attaché au mauvais script
  si l'éditeur change ; fichiers `.<nom>.bench<N>.oc` écrits dans le projet.
- Lint : diagnostics d'un `ocaracs` en retard republiés sur un fichier fermé,
  deux exécutions à chaque ouverture, surlignage laissé après désactivation
  (`lint.ts`).
- `ocara.compilerPath` modifié : le serveur n'est pas redémarré, pas de
  commande « Redémarrer » (`lspclient.ts`).
- Grammaire TextMate : `` `a \` b` `` colore le reste du fichier comme une
  chaîne ; `\${x}` coloré comme interpolation ; `generic List<T> {` jamais
  reconnu ; `if (`, `while (`, `return (` colorés comme des fonctions ;
  `import` décoloré avec un commentaire de fin, une indentation ou un alias
  minuscule ; HTML dans les gabarits (`<!-- ${x}`) qui avale le fichier.
- Divers : ancres `#…` des liens de documentation perdues (`docs.ts` ~l.60) ;
  diagnostics de compilation gardés jusqu'à la compilation suivante ; « Compiler
  et lancer » suppose un shell POSIX ; `'` fermé automatiquement dans les
  commentaires.

## CI et Makefile

- `ci/start.sh` : `set -e` commenté ; `ci/memory_bound.sh` exécute sans
  `timeout` ; `Makefile` `lint-examples` finit par `|| true`.

## Priorité / Complexité

**Haute** (corrections automatiques qui cassent le code, états périmés) —
**Légère**.

## Fichiers clés

`tools/ocaracs/src/` (`fix.rs`, `rename.rs`, `scope.rs`, `check.rs`,
`config.rs`), `src/lsp/` (`mod.rs`, `features.rs`, `position.rs`,
`navigation.rs`), `tools/highlight/vsode/src/` (`benchmark.ts`, `lint.ts`,
`lspclient.ts`, `docs.ts`, `compile.ts`), `tools/highlight/vsode/syntaxes/`,
`ci/`, `Makefile`.
