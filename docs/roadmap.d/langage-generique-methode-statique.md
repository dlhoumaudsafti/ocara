# Méthode statique d'un `generic` inappelable

Statut : **à corriger** — constaté le 2026-10-13 (vérification automatique des exemples de la documentation, `ci/doc_examples.sh`).

## Constat

L'EBNF (§20, types `Result<Value, Error>` et `Option<T>`) présente des méthodes statiques de fabrique sur un `generic` :

```ocara
generic Box<T> {
    private property value:T
    init(v:T) { self.value = v }
    public static method of(v:T): Box<T> { return use Box<T>(v) }
    public method get(): T { return self.value }
}

function main(): int {
    var b:Box<int> = Box::of(4)
    return 0
}
```

- Jusqu'au 2026-10-13, la sema refusait le corps (`expected return type 'Box<mixed>', found 'Box<T>'`) : corrigé (`types_compat` compare désormais les arguments d'un générique un à un).
- Reste : l'appel `Box::of(4)` vise un générique **non instancié** ; aucune fonction `Box_of` n'est générée (seules les instances monomorphisées `Box_int_…` existent). Le codegen refuse l'appel (« appel vers une fonction inconnue : Box_of ») — avant le garde-fou des appels inconnus, il rendait `null` en silence. La syntaxe `Box<int>::of(4)` n'est pas acceptée par le parseur (`'<' has been removed`).

## Correction attendue (à trancher)

- soit inférer l'instanciation depuis le type attendu (`var b:Box<int> = Box::of(4)`) ou les arguments, et appeler `Box_int_of` ;
- soit accepter une syntaxe explicite `Box<int>::of(4)` ;
- soit refuser en sema une méthode statique de `generic` qui utilise un paramètre de type (diagnostic clair) et corriger les exemples de l'EBNF (§20).

## Fichiers concernés

`src/core/monomorph.rs` (instanciation), `src/sema/typecheck.rs` (`Expr::StaticCall`), `src/parsing/parser.d/expressions.rs`, `docs/EBNF.md` §20.

## Validation

Test `examples/tests/` : fabrique statique d'un générique utilisée avec deux instanciations ; exemples `Result`/`Option` de l'EBNF compilés et exécutés.

## Priorité / Complexité

**Haute** (programme présenté comme valide par la documentation, refusé) — **Structurel**.
