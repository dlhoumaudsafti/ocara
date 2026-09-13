# Le compilateur n'est pas "cargo-buildable" nativement

## ✅ Lien OpenSSL inconditionnel — retiré

`build.rs` (racine) liait inconditionnellement `-lssl`/`-lcrypto` au binaire du **compilateur** `ocara` lui-même (`println!("cargo:rustc-link-lib=ssl/crypto")`), alors que `src/codegen/link.rs` documente et applique explicitement le choix inverse pour les **programmes utilisateur compilés** : OpenSSL est vendored (compilé depuis les sources, feature `vendored` de `runtime/Cargo.toml`) et intégré statiquement à `libocara_runtime.a` précisément pour qu'aucun lien dynamique vers `libssl`/`libcrypto` ne soit nécessaire nulle part — le commentaire de `link.rs` dit littéralement que passer `-lssl` "ferait échouer le lien sur une machine de build sans libssl-dev/zlib1g-dev". Les deux lignes dans `build.rs` contredisaient directement ce choix, pour le build du compilateur lui-même : sans rapport avec MySQL (jamais utilisé par le compilateur), pure incohérence issue d'un reliquat antérieur au vendoring.

**Corrigé** : les deux lignes supprimées. Vérifié : `make build` (rebuild complet forcé) réussit sans elles, et un programme utilisateur qui importe `ocara.MySQL` continue de compiler, se linker et s'exécuter correctement (`examples/builtins/mysql.oc`) — confirmant que le vendoring dans `libocara_runtime.a` suffisait déjà à lui seul.

## Réévaluation du shim pkg-config (`.pkgconfig-shim/`)

En le relisant en détail (`Makefile:58-66`), ce mécanisme est en fait déjà robuste, pas seulement "ad hoc" comme précédemment caractérisé ici : il ne crée un symlink `webkit2gtk-4.0.pc`/`javascriptcoregtk-4.0.pc` **que si** `pkg-config --exists <pkg>-4.0` échoue **et** `pkg-config --exists <pkg>-4.1` réussit — une vraie détection dynamique de l'état de `pkg-config`, pas une hypothèse figée sur une version d'Ubuntu précise. Il ne fait rien (no-op) sur une distribution où `-4.0` existe déjà. Pas de changement apporté ici : rien d'identifié qui mérite une réécriture.

## Toujours ouvert (hors périmètre "Légère")

- **`build.rs` exige toujours que les 3 `.a` existent déjà** dans `target/release/` avant de pouvoir compiler `ocara` — un simple `cargo build -p ocara` échoue toujours sans passer par `make build` au préalable. Reste un chantier Structurel (voir plus bas).
- **Contrainte `-j1` pour Cranelift** : toujours documentée mais non résolue (Cranelift sature la mémoire en parallèle) — non réévaluée, pas de piste de correctif léger identifiée.

## Ampleur (restante)

Rendre `cargo build -p ocara` fonctionnel seul (sans Makefile) reste Structurel : soit un `build.rs` qui invoque lui-même la compilation des sous-crates runtime, soit une fusion en dépendances Cargo normales avec ordre de build déclaré nativement.

## Fichiers clés

`build.rs`, `src/codegen/link.rs` (justification du choix "pas de -lssl"), `Makefile` (`.pkgconfig-shim`), `runtime/Cargo.toml` (feature `vendored`).
