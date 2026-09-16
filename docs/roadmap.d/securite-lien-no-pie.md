# `-no-pie` au lien final — ASLR désactivé sans justification documentée

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

**Priorité Moyenne** — n'affecte pas la correction fonctionnelle du langage (aucun bug utilisateur connu n'y est rattaché), mais c'est un affaiblissement de sécurité qui touche tous les binaires produits par le compilateur, sans trace de justification. **Complexité : Simple** — un essai de retrait + `make regression` suffit à trancher.

## Fichiers clés

`src/codegen/link.rs`.
