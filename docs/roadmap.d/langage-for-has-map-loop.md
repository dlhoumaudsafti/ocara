# Boucle sur map : `for k has v in m` à la place de `for k => v in m`

## Constat

La boucle sur une map s'écrit aujourd'hui `for cle => valeur in m { }`.
`=>` est un symbole, pas un mot, ce qui va contre l'intention d'Ocara d'une
syntaxe explicite. Il sert déjà, avec un autre sens, dans les bras de
`match` (`100 => "parfait"`).

## Proposition

```ocara
for pays has capitale in capitales {
    IO::writeln(`${pays} → ${capitale}`)
}
```

## Points à trancher

- **`has` mot-clé contextuel** : `has` est aujourd'hui un identifiant
  ordinaire, utilisé comme nom de méthode builtin (`m.has(k)`,
  `sess.has("user")`, `HTTPServerSession::hasGlobal`). Il ne doit devenir
  réservé qu'entre la variable de boucle et `in`, sinon ces appels cassent.
- **Sort de `=>`** : coexistence (avec un avertissement de dépréciation), ou
  remplacement cassant (`for k => v` ne compile plus, comme l'ancien
  `req:int` de `HTTPServer`) ? 19 boucles du corpus utilisent `=>` (dont
  `examples/07_loops.oc`, `examples/09_maps.oc`, `examples/tests/07_loopsTest.oc`).
- **Migration** : `ocaracs --fix` pourrait réécrire `for a => b in` en
  `for a has b in`, avec une règle de style dédiée tant que les deux formes
  coexistent.

## À mettre à jour

Parseur (`Stmt::ForMap`), `docs/EBNF.md` (boucles, §31), corpus d'exemples
et tests, coloration et mots-clés VS Code (`syntaxes`, `src/keywords.ts`),
extension et ocaracs (`scope.rs` ne dépend pas de `=>`, à vérifier).

## Priorité / Complexité

Moyenne — **Légère**.
