# Opérateurs d'affectation composés `+=`, `-=`, `*=`, `/=`, `%=`

## Constat

Aucun opérateur composé n'existe aujourd'hui : ni token dans le lexer, ni
règle dans `docs/EBNF.md`. Il faut écrire `total = total + n`.

## Proposition

| Opérateur | `int` / `float` | `string` |
|---|---|---|
| `x += n` | addition | concaténation : `s += " le monde"` |
| `x -= n` | soustraction | suppression de **toutes** les occurrences : `"salut le monde" -= " le monde"` → `"salut"` |
| `x *= n` | multiplication | — (erreur de typage) |
| `x /= n` | division | — |
| `x %= n` | modulo | — |

```ocara
var n:int = 10
n += 5      // 15
n %= 4      // 3

var s:string = "salut"
s += " le monde"      // "salut le monde"
s -= " le monde"      // "salut"
```

## Points à trancher

- **Sucre ou instruction propre** : réécrire `x op= e` en `x = x op e` au
  parsing (simple, mais `obj.f()[i] += 1` évaluerait la cible deux fois), ou
  évaluer la cible une seule fois au lowering.
- **Cibles admises** : variable seulement, ou aussi champ (`self.total += n`)
  et index (`m["k"] += 1`, `a[i] *= 2`) ?
- **`-=` sur string** : n'a pas d'équivalent `-` binaire entre chaînes.
  `s -= x` vaudrait `s = String::replace(s, x, "")`. Faut-il aussi un
  opérateur `-` (`s - x`) pour la cohérence, ou garder `-=` seul ?
- **Types mixtes** : `int += float` interdit (comme `int = float`), ou
  promotion ? `float %= n` (`fmod`) ? `/=` sur `int` : division entière,
  comme `/` ?
- **`mixed`** : autorisé via les opérations dynamiques existantes
  (`__dyn_add`…), ou refusé ?
- **Immutables** : `const`, paramètre, champ non mutable → même erreur que
  l'affectation simple (`SemaError::InvalidAssign`).

## À mettre à jour

Lexer (5 tokens), parseur (instruction d'affectation), sema (typage par
opérateur), lowering, `docs/EBNF.md` (affectation, §31), `docs/diagnostics.md`
si un nouveau code est créé, coloration VS Code
(`tools/highlight/vsode/syntaxes`), et R03 d'ocaracs (`decl_assign_pos`
ignore déjà `==`/`<=`/`>=`, à étendre à `+=`…).

## Priorité / Complexité

Moyenne — **Légère** (sucre syntaxique) à **Structurel** si la cible ne doit
être évaluée qu'une fois.
