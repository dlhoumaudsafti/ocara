# Le compilateur "cargo-buildable" nativement : deux vérifications restantes

Le lien OpenSSL inconditionnel du build du compilateur est retiré, et `cargo build -p ocara` fonctionne maintenant seul (sans passer par `make build` au préalable) — voir git log pour le détail.

## Reste à faire : vérifier la contrainte `-j1` documentée pour Cranelift

Toujours documentée comme nécessaire (Cranelift saturerait la mémoire en parallèle), mais le `Makefile` utilise en réalité `-j4` pour la compilation de `ocara` lui-même dans sa cible `build` actuelle — soit cette contrainte ne s'applique plus/pas à cette machine, soit elle a été assouplie sans mise à jour du commentaire correspondant. À vérifier sur une machine où l'OOM avait réellement été observé, puis corriger la documentation en conséquence.

## Reste à faire : vérifier le round-trip complet "zéro `.a` → binaire fonctionnel"

`build.rs` compile maintenant lui-même chaque `.a` runtime manquant à la volée, vérifié partiellement (déclenchement de chaque recompilation imbriquée confirmé individuellement). Un aller-retour minuté complet en une seule commande (`rm -rf target/release/*.a && cargo build --release -p ocara`) n'a pas pu être mené à son terme faute de budget machine suffisant dans la session où ce mécanisme a été écrit (chaque dépendance C vendored — OpenSSL, SQLite — prend plusieurs dizaines de secondes à se recompiler depuis les sources). À confirmer sur une machine disposant de plus de temps.

## Fichiers clés

`build.rs` (`ensure_runtime_lib`), `Makefile`.
