# Le compilateur "cargo-buildable" nativement : une vérification restante

Le lien OpenSSL inconditionnel du build du compilateur est retiré, `cargo build -p ocara` fonctionne maintenant seul (sans passer par `make build` au préalable), et le commentaire `-j1` périmé du `Makefile` est corrigé (`-j4` est utilisé sans incident depuis des dizaines de builds dans cette session — voir git log).

## Reste à faire : vérifier le round-trip complet "zéro `.a` → binaire fonctionnel"

`build.rs` compile maintenant lui-même chaque `.a` runtime manquant à la volée, vérifié partiellement (déclenchement de chaque recompilation imbriquée confirmé individuellement). Une nouvelle tentative (suppression des 3 `.a` puis `cargo build --release -p ocara` en une seule commande) a été lancée mais est restée silencieuse plus d'une heure sans qu'aucune sortie n'apparaisse (recompilation d'OpenSSL/SQLite vendored depuis les sources, ou blocage — impossible à distinguer sans sortie intermédiaire) — abandonnée avant d'aller au bout, sans avoir cassé quoi que ce soit (le binaire `ocara` déjà construit au moment de la suppression reste fonctionnel, vérifié). **Découverte utile au passage** : le build imbriqué que `build.rs` lance (`Command::new(env!("CARGO"))`) n'affiche aucune progression au processus `cargo` parent tant qu'il n'est pas terminé — un aller-retour complet donnerait l'impression d'un blocage même s'il progresse normalement. À reprendre avec un budget de temps large (potentiellement 10+ minutes) et un moyen de surveiller la progression réelle (ex. `strace`/vérifier que le processus enfant avance) plutôt qu'une commande bloquante sans retour.

## Fichiers clés

`build.rs` (`ensure_runtime_lib`), `Makefile`.

## Reste à faire — audit du 2026-10-13

Hypothèse forte, non vérifiée (cargo non lancé pendant l'audit) : `build.rs` (~l.48-57) lance un `cargo build --release` imbriqué dans le **même** répertoire `target/`, depuis un build script ; le cargo parent tient `target/release/.cargo-lock`. Le blocage d'« plus d'une heure sans sortie » serait un interblocage sur ce verrou (le message « Blocking waiting for file lock » de l'enfant est capturé, donc invisible), et non une recompilation lente. Piste : `CARGO_TARGET_DIR` séparé pour le build imbriqué.
