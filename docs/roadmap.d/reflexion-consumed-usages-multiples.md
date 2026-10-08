# Réflexion : `consumed` à usages multiples

Statut : **non tranché** — idée proposée le 2026-10-08.

## Idée

Garder `consumed` tel quel (référence rendue juste après le premier usage)
et permettre de déclarer un nombre d'usages autorisés :

```ocara
consumed truc:string = "salut"     // 1 usage (inchangé)
consumed+1 truc:string = "salut"   // forme « incrément » : 1 + 1 = 2 usages
consumed<2> truc:string = "salut"  // forme « total » : 2 usages
```

Formes candidates pour le total : `consumed<2>`, `consumed{2}`, `consumed[2]`.

## Points à trancher

- **Incrément ou total** : `consumed+1` se lit comme un ajout au cas de base,
  mais oblige à calculer ; un total (`consumed<2>`) dit directement combien
  d'usages sont permis. Un seul des deux devrait exister.
- **Syntaxe** : `<N>` rappelle les génériques (`array<T>`), `[N]` l'indexation,
  `{N}` les blocs ; aucune n'est libre de toute ambiguïté visuelle.
- **Ce qu'est « un usage »** : aujourd'hui, une lecture par instruction
  (deux lectures dans la même instruction, ou deux `${x}` dans un même
  gabarit `renderFile`, sont refusées). Faut-il compter par lecture ou par
  instruction ?
- **Boucles** : un usage dans une boucle est répété ; refuser `consumed<N>`
  dans une boucle plus profonde que sa déclaration, comme aujourd'hui pour
  la libération immédiate ?
- **Intérêt réel avec le comptage de références** : depuis
  [memoire-refcount.md](memoire-refcount.md), `scoped` rend sa référence en
  fin de bloc et toute sortie est sûre. `consumed<N>` n'apporte qu'une
  libération plus précoce (après le N-ième usage) et une garantie
  d'usage borné vérifiée à la compilation.

## Mise en œuvre si retenue

- Parseur : borne optionnelle sur `consumed`.
- Sema : compteur d'usages par binding au lieu du booléen actuel (E17).
- Lowering : `rc::release_consumed_used_in` relâche au N-ième usage
  (`rc_consumed` porte le reste d'usages).
