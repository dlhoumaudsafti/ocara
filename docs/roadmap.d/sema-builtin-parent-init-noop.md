# Constructeur des exceptions builtin sans effet — corrigé

## Constat

`parent::init(message, code)` dans une sous-classe d'`Exception` compilait
mais n'affectait rien (`err.message` valait `null`). En creusant, le bug était
plus large : `use Exception("boom", 3)` et `raise use FileException("x", 2)`
donnaient eux aussi `message = null` et `code = 0`. Ces appels produisaient
un `Exception_init` qui n'existe nulle part, et le codegen ignore sans rien
dire un appel vers une fonction inconnue. Le corpus contournait le problème
en écrivant `self.message = message`.

## Correctif

- **Constructeur `(message:string, code:int = 0)`** pour `Exception` et toutes
  les exceptions builtin (`is_builtin_exception`,
  `src/builtins/exception.rs`).
- **Lowering** (`lower_builtin_exception_init`, `src/lower/expr.d/lower.rs`) :
  pas de fonction runtime, les champs `message`, `code` et `source` (`""`)
  sont écrits directement aux offsets de la disposition. Ce chemin sert à
  `use XException(...)` et à `parent::init(...)` d'une sous-classe, qui
  écrit dans `self`.
- **Sema** (`check_builtin_exception_ctor`, `src/sema/typecheck.rs`) : arité
  1 ou 2 et types `string`/`int` vérifiés. Auparavant, aucun contrôle n'était
  fait.
- Le message d'E60 recommande désormais `parent::init(message, code)`.

Tests : `examples/tests/77_builtin_exception_constructorTest.oc`,
`src/sema/tests/use_without_init.rs`. Doc : `docs/EBNF.md` §29.4.

## Limite connue

`var e:Exception = use FileException(...)` reste refusé (« expected type
'Exception', found 'FileException' ») : les exceptions builtin n'ont pas
`Exception` comme parent pour la sema (voir `BUILTIN_EXCEPTION_NAMES`), seul
`on e is Exception` les attrape. C'était déjà le cas avant ce correctif.

## Bug trouvé

`${e}` d'une chaîne levée → voir
[langage-raised-string-interpolation](langage-raised-string-interpolation.md).

## Reste à faire — audit du 2026-10-13

**La « limite connue » est devenue un bug, aggravé par E62.** Pour la sema, les exceptions builtin (`FileException`, `IOException`…) ne sont pas des sous-types de `Exception` : `var z:Exception = use FileException(...)` était déjà refusé, et depuis E62 passer l'exception attrapée à une fonction l'est aussi :

```ocara
function logErr(e:Exception): void { IO::writeln(e.message) }
try { … } on e is FileException { logErr(e) }
// error: argument 'e' of 'logErr': expected type 'Exception', found 'FileException'
```

Contraire à `docs/EBNF.md` (~l.3770 : elles « héritent toutes implicitement de `Exception` »). Correction : faire de `Exception` l'ancêtre de chaque exception builtin pour `types_compat`/`class_matches` (`BUILTIN_EXCEPTION_NAMES`). **Haute (programme valide refusé) — Légère.** Test sema + `examples/tests/77_builtin_exception_constructorTest.oc`.

**Corrigé le 2026-10-13** : `class_matches` (`src/sema/symbols.d/lookups.rs`) considère chaque exception builtin comme fille de `Exception`. Test : `builtin_exception_is_accepted_for_exception` (`src/sema/tests/arg_types.rs`).
