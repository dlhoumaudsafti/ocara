# Appel à une fonction ou une classe inconnue accepté — `null` silencieux

Statut : **corrigé** le 2026-10-13 (constaté le même jour par l'audit de la roadmap).

## Constat

```ocara
IO::writeln(foo())               // foo jamais déclarée : « compilation réussie », affiche null
var x:int = Nope::label()        // classe Nope inexistante : compile ; x + 1 affiche 1
```

Cas particulier, les homonymes : un fichier qui appelle `Helper::label()` ou
`label()` défini dans un namespace qu'il n'importe pas obtient `null` sans
diagnostic (et non « le premier chargé » comme l'écrivait
[langage-classes-homonymes-fusionnees](langage-classes-homonymes-fusionnees.md)).

Contraire à `docs/diagnostics.md` (E01 : « fonction utilisée sans avoir été
déclarée »).

Point connexe : `ocara --check` accepte `String::trim(...)` sans
`import ocara.String`, alors que la compilation le refuse.

## Cause

- sema, `src/sema/typecheck.rs` : retombée finale sur `Type::Mixed` sans
  erreur dans `Expr::Call` (~l.1676) et `Expr::StaticCall` (~l.1857) ; E01
  n'est levée que si la classe est connue ;
- codegen, `src/codegen/emit.d/instructions.d/calls.rs` (~l.69) : appel inconnu
  ignoré.

## Correction attendue

E01 pour toute fonction libre ou classe inconnue (hors classes opaques
d'imports non résolus, déjà traitées à part) ; erreur interne au codegen pour
un appel qui ne correspond à aucun symbole ; même résultat en `--check` qu'en
compilation.

## Validation

Tests sema (fonction inconnue, classe inconnue, homonyme non importé) ;
`make regression` (vérifier qu'aucun exemple ne reposait sur ce silence).

## Priorité / Complexité

**Haute** (programme invalide accepté, résultat faux silencieux) — **Légère**.

## Correctif

- Sema : E01 pour une fonction libre inconnue (ni locale, ni fonction, ni constante) et pour un appel statique sur une classe inconnue (ni classe, interface, générique, module, enum, ni builtin) ; `Classe()` sans `use` est signalé « not callable » (`write 'use Classe(...)'`).
- Codegen (`calls.rs`) : un appel vers un symbole inexistant est une erreur interne (seul un `Classe_init` absent — classe sans constructeur — reste sans effet).
- Le garde-fou a révélé des wrappers `__fn_wrap_*` générés pour des méthodes d'interface et des générateurs (appels vers des symboles inexistants) : ils ne sont plus générés.
- `examples/tests/31_default_paramsTest.oc` appelait `Calculator()` sans `use` (instance jamais créée) : corrigé.

Tests : `src/sema/tests/unknown_symbols.rs`. Reste : l'incohérence `--check` / compilation sur `String::trim` sans import.
