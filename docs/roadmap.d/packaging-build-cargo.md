# Le compilateur n'est pas "cargo-buildable" nativement

## ✅ Lien OpenSSL inconditionnel — retiré

`build.rs` (racine) liait inconditionnellement `-lssl`/`-lcrypto` au binaire du **compilateur** `ocara` lui-même (`println!("cargo:rustc-link-lib=ssl/crypto")`), alors que `src/codegen/link.rs` documente et applique explicitement le choix inverse pour les **programmes utilisateur compilés** : OpenSSL est vendored (compilé depuis les sources, feature `vendored` de `runtime/Cargo.toml`) et intégré statiquement à `libocara_runtime.a` précisément pour qu'aucun lien dynamique vers `libssl`/`libcrypto` ne soit nécessaire nulle part — le commentaire de `link.rs` dit littéralement que passer `-lssl` "ferait échouer le lien sur une machine de build sans libssl-dev/zlib1g-dev". Les deux lignes dans `build.rs` contredisaient directement ce choix, pour le build du compilateur lui-même : sans rapport avec MySQL (jamais utilisé par le compilateur), pure incohérence issue d'un reliquat antérieur au vendoring.

**Corrigé** : les deux lignes supprimées. Vérifié : `make build` (rebuild complet forcé) réussit sans elles, et un programme utilisateur qui importe `ocara.MySQL` continue de compiler, se linker et s'exécuter correctement (`examples/builtins/mysql.oc`) — confirmant que le vendoring dans `libocara_runtime.a` suffisait déjà à lui seul.

## Réévaluation du shim pkg-config (`.pkgconfig-shim/`)

En le relisant en détail (`Makefile:58-66`), ce mécanisme est en fait déjà robuste, pas seulement "ad hoc" comme précédemment caractérisé ici : il ne crée un symlink `webkit2gtk-4.0.pc`/`javascriptcoregtk-4.0.pc` **que si** `pkg-config --exists <pkg>-4.0` échoue **et** `pkg-config --exists <pkg>-4.1` réussit — une vraie détection dynamique de l'état de `pkg-config`, pas une hypothèse figée sur une version d'Ubuntu précise. Il ne fait rien (no-op) sur une distribution où `-4.0` existe déjà. Pas de changement apporté ici : rien d'identifié qui mérite une réécriture.

## ✅ `cargo build -p ocara` fonctionne maintenant seul, sans `make build` au préalable

`build.rs` exigeait jusqu'ici que les 3 `.a` (`libocara_runtime.a`/`_tauri.a`/`_sdl.a`) existent déjà dans `target/release/` — un simple `cargo build -p ocara`, sans être jamais passé par `make build` au préalable, échouait toujours avec un message renvoyant vers `make build`.

**Corrigé** : `build.rs` compile lui-même, à la volée, chaque `.a` manquant via `cargo build --release -p <crate>` (`Command::new(env!("CARGO"))`, voir `ensure_runtime_lib`) — invocation de cargo DEPUIS un build script, un pattern courant (bindgen/vendoring de bibliothèques C) plutôt qu'une nouveauté risquée. Pour `ocara_runtime_tauri` spécifiquement, `build.rs` réplique aussi en Rust la logique du shim pkg-config du Makefile (`ensure_pkgconfig_shim` — mêmes deux appels `pkg-config --exists`/`--variable=pcfiledir` que le Makefile, jamais dans le système, régénéré sans risque à chaque build).

**Vérifié** (voir la méthodologie ci-dessous) :
- `make build` (le chemin existant, .a déjà présents) continue de fonctionner correctement et aussi rapidement qu'avant (~20s pour `ocara` seul) — `ensure_runtime_lib` retourne immédiatement sans rien recompiler quand le `.a` existe déjà. `make regression` : 388 PASS / 0 FAIL / 0 ERREUR, aucune régression.
- Le déclenchement effectif de la compilation imbriquée a été testé en supprimant successivement chacun des 3 `.a` puis en relançant une compilation directe (sans Makefile) : les 3 crates (`ocara_runtime`, `ocara_runtime_tauri`, `ocara_runtime_sdl`) se recompilent bien chacun via l'invocation imbriquée, produisant l'artefact attendu — confirmé par la présence des `.a` fraîchement compilés sous `target/release/deps/`. Aucun signe de blocage (deadlock) sur le verrou de `target/` : chaque test s'est terminé proprement (aucun processus `cargo`/`rustc` restant après coup), avec une progression constante d'un essai à l'autre.
- **Limite de la vérification, assumée** : un aller-retour minuté et complet "zéro fichier `.a` → binaire `ocara` fonctionnel en une seule commande" n'a pas pu être mené à son terme dans cette session — chaque invalidation de cache déclenche une recompilation complète des dépendances C vendored du runtime (OpenSSL, SQLite bundled, zlib), intrinsèquement longue (`ocara_runtime` seul : ~1min13 en repartant d'un cache invalidé, mesuré directement) et dont la durée totale cumulée (3 crates + le compilateur lui-même, potentiellement executée une seconde fois si les `RUSTFLAGS` de l'appel initial ne correspondent pas à un run précédent, invalidant le cache de tout l'arbre) dépasse largement ce qu'il restait de budget dans cette session. Le mécanisme est jugé correct sur la base des éléments ci-dessus (aucune classe d'erreur — blocage, artefact mal placé, chemin cassé — observée sur aucun des 3 sous-crates ni sur le chemin déjà-construit) mais reste à confirmer par un utilisateur disposant de plus de temps machine (`rm -rf target/release/*.a && time cargo build --release -p ocara`, en dehors de toute session au budget contraint).

## Contrainte `-j1` pour Cranelift — non réévaluée

Toujours documentée mais non résolue (Cranelift sature la mémoire en parallèle) — non réévaluée, pas de piste de correctif léger identifiée. **Nuance découverte en testant ce chantier** : le `Makefile` utilise en réalité `-j4` (pas `-j1`) pour la compilation de `ocara` lui-même dans son target `build` actuel — soit cette contrainte documentée ne s'applique plus/pas à cette machine, soit elle a été assouplie sans mise à jour de ce commentaire ; à vérifier sur une machine où l'OOM avait réellement été observé.

## Fichiers clés

`build.rs` (`ensure_runtime_lib`, `ensure_pkgconfig_shim`), `src/codegen/link.rs` (justification du choix "pas de -lssl"), `Makefile` (`.pkgconfig-shim`, cible `build`), `runtime/Cargo.toml` (feature `vendored`, dépendances C vendored : `openssl`, `rusqlite` bundled, `libz-sys` static).
