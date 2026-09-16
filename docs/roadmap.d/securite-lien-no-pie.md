# `-no-pie` au lien final — ASLR désactivé sans justification documentée

## ✅ Terminé — `-no-pie` conservé, justification documentée

Audit fait : `-no-pie` retiré à l'essai, `cargo build -p ocara` + compilation d'un exemple → succès, **mais** le binaire résultant est un `DT_TEXTREL` PIE (`readelf -d` : `TEXTREL`, `ld` avertit `creating DT_TEXTREL in a PIE`) — confirmé par `file` (`pie executable`) et `readelf -l` (le chargeur doit rendre le segment de code inscriptible au démarrage pour appliquer les relocations). Root cause : Cranelift (`src/codegen/emit.d/emitter.rs`, `settings::Flags::new`) n'active jamais `is_pic` (`false` par défaut dans `cranelift-codegen` — vérifié dans `settings.rs` du crate vendored) — le `.o` généré utilise des relocations absolues, pas du code indépendant de la position.

Un PIE avec `TEXTREL` n'est **pas** un gain net : le segment de code doit rester réinscriptible au chargement pour que le linker dynamique y écrive les relocations, ce qui affaiblit la protection W^X que PIE est censé renforcer — un recul de sécurité différent, pas moins réel que l'absence d'ASLR. **Décision : `-no-pie` reste**, avec un commentaire détaillé ajouté directement dans `src/codegen/link.rs` expliquant cette root cause et pourquoi le retrait seul n'est pas une amélioration.

La vraie correction (un binaire PIE sans `TEXTREL`) demanderait d'activer `is_pic = true` côté Cranelift — un chantier bien plus large qu'un flag de lien (touche tout l'adressage émis par le codegen pour les globales/appels), hors du périmètre "Simple" de ce ticket. Pas de nouvelle fiche ouverte pour ça tant qu'aucun besoin concret ne l'exige (défense en profondeur, pas un bug fonctionnel).

## Constat

`src/codegen/link.rs:127` ajoute systématiquement `-no-pie` à la commande `cc` finale, pour **tout** binaire produit par Ocara :

```rust
.arg("-no-pie")
.arg("-Wl,--allow-multiple-definition")
```

`-no-pie` désactive l'ASLR (Address Space Layout Randomization) pour le binaire produit — régression de sécurité par rapport au PIE (Position Independent Executable) activé par défaut sur la quasi-totalité des toolchains Linux modernes (gcc/clang récents lient en PIE par défaut depuis plusieurs années). Le commentaire adjacent (`link.rs:108`) justifie `--allow-multiple-definition` (les symboles du `.o` programme priment sur ceux de l'archive runtime) mais **aucun commentaire ne justifie `-no-pie`** — impossible de distinguer, sans clarification, si c'est une nécessité technique (ex. un mécanisme du runtime dépend d'adresses non-relogées, comme les strings globales avec header à offset fixe — voir `memoire-fiabilite-runtime-bas-niveau.md`) ou un réglage hérité d'un premier essai jamais reconsidéré.

## Ce qui est demandé

Investiguer puis documenter (ou retirer) :
1. Retirer `-no-pie` et lancer `make regression` — si tout passe, c'était un réglage superflu, à retirer purement et simplement.
2. Si le retrait casse quelque chose, documenter précisément quel mécanisme du runtime dépend d'un lien non-PIE directement en commentaire à côté de `.arg("-no-pie")`, pour que ce ne soit plus une décision silencieuse.

## Priorité / Complexité

**✅ Terminé.** Était Priorité Moyenne (affaiblissement de sécurité sans justification documentée) — fermé : la justification existe maintenant (Cranelift n'émet pas de code PIC), documentée dans le code, `make regression` inchangé (637 PASS, 0 FAIL).

## Fichiers clés

`src/codegen/link.rs`.
