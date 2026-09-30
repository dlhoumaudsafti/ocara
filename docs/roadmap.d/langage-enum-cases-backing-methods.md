# Enums enrichis — syntaxe `case`, type de backing typé, méthodes d'instance

## Demande (David)

Trois formes, cumulatives :

```ocara
enum OrderStatus {
    case Pending
    case Paid
    case Shipped
    case Cancelled
}
```

```ocara
enum Status: string
{
    case Pending = 'pending'
    case Paid = 'paid'
    case Cancelled = 'cancelled'
}
```

```ocara
enum OrderStatus: string
{
    case Pending = 'pending'
    case Paid = 'paid'
    case Shipped = 'shipped'
    case Cancelled = 'cancelled'

    public method isFinal(): bool
    {
        return match (self) {
            self::Shipped,
            self::Cancelled => true,

            self::Pending,
            self::Paid => false,
        }
    }
}

var status:Enum<OrderStatus> = OrderStatus::Shipped
status.isFinal(); // true
```

Modèle proche des enums PHP 8.1 (`case`, backing type, méthodes) / des enums Swift.

## État actuel (vérifié dans le code)

L'enum existant est volontairement minimal :

- `EnumDecl`/`EnumVariant` (`src/parsing/ast.d/enums.rs`) : une variante n'a qu'un `name` et une `value: Option<i64>` — **la valeur brute est câblée en dur sur `i64`**, aucun champ pour un backing type déclaré, aucun champ pour une liste de méthodes.
- Syntaxe actuelle : virgules, pas de mot-clé `case` — `EnumDecl ::= "enum" Identifier "{" EnumVariant ("," EnumVariant)* ","? "}"`, `EnumVariant ::= Identifier ("=" Integer)?` (`docs/EBNF.md` §21).
- Une variante a le type `int` et est utilisable partout où un `int` est attendu (`docs/EBNF.md` §21) — pas de backing `string`, pas de backing générique.
- Un enum n'est **pas instanciable via `use`** et n'a **aucune méthode** — c'est un pur groupe de constantes entières, pas une « chose » avec table de méthodes comme une `class`.
- Accès aux variantes : `EnumName::VariantName`, syntaxe `StaticConst` déjà existante (`src/sema/typecheck.rs`, `src/sema/symbols.d/registers.rs`).

Le `match` a lui aussi des limites qui bloquent l'exemple avec méthode :

- `MatchArm` (`src/parsing/ast.d/patterns.rs`) a `pattern: Option<MatchPattern>` — **un seul pattern par bras**, pas de liste. Le pattern `self::Shipped, self::Cancelled => true,` (deux patterns partageant un même corps, séparés par une virgule) n'est pas représentable aujourd'hui.
- `MatchPattern` (même fichier) n'a que `Literal(Literal)` et `IsType(Type)` — **aucune variante pour référencer un cas d'enum** (`self::Shipped`, ou `OrderStatus::Shipped` en dehors d'un contexte `self`) comme motif de `match`.
- `docs/EBNF.md` §26 confirme : `MatchArm ::= MatchPattern "=>" Expression` (un seul pattern, pas de virgule finale montrée dans les exemples existants).

## Sous-chantiers (cumulatifs, à trancher avant d'implémenter)

1. **Syntaxe `case`** — remplace ou coexiste avec la syntaxe actuelle à virgules (`Name = Integer`) ? Un changement de syntaxe pur cassant tout le corpus existant utilisant la forme à virgules serait à lister explicitement (`grep -rn "^enum " examples/` pour mesurer l'empreinte réelle).
2. **Backing type déclaré** (`enum Status: T`) — au minimum `int` (défaut actuel implicite) et `string` (demandé). Liste complète des types de backing autorisés à trancher (`float`/`bool` ont-ils un sens ici ?). Valeur brute par cas devient typée selon `T`, plus seulement `i64` en dur dans `EnumVariant`.
3. **Méthodes d'instance sur un enum** — nécessite qu'un enum devienne une vraie déclaration avec table de méthodes (comme une `class`/`interface`), `self` désignant le cas courant à l'intérieur d'une méthode. Interaction à définir avec tout ce qui suppose aujourd'hui qu'un enum est un pur groupe de constantes (ex. `is_async` propagé pour class/module/generic mais jamais pour un enum, cf. correctifs de cette session sur les modificateurs d'interface).
4. **Pattern de `match` sur un cas d'enum** (`self::Case` / `EnumType::Case` comme `MatchPattern`) et **patterns multiples séparés par une virgule dans un même bras** (`self::A, self::B => expr`) — deux extensions indépendantes du pattern matching existant, nécessaires toutes les deux pour l'exemple `isFinal()`. Exhaustivité à trancher : un `match (self)` sur un enum sans cas `default` doit-il exiger de couvrir tous les cas (vérifié à la compilation), comme en Swift/Rust ?
5. **Annotation de type : `OrderStatus` directement, PAS de wrapper `Enum<T>`** (tranché — David, par le même raisonnement appliqué à
   [langage-struct-value-type](langage-struct-value-type.md)) : `OrderStatus::Shipped` produit directement une valeur de type `OrderStatus`, exactement comme aujourd'hui (« une variante a le type `int` directement, sans wrapper »). Contrairement à `Resolvable<T>` (voir [langage-async-non-int-return-type-check](langage-async-non-int-return-type-check.md)), qui enveloppe un HANDLE n'étant pas encore la valeur réelle, un cas d'enum EST directement la valeur — un wrapper n'apporterait rien et casserait la cohérence avec le fonctionnement actuel.

## Priorité / Complexité

**Haute** (demande explicite) — **Massive** : nouveau système d'enum quasi complet (syntaxe, backing type typé générique, table de méthodes, `self` dans ce contexte) cumulé à une extension du pattern matching (référence à un cas d'enum comme motif, motifs multiples par bras) — pas une extension incrémentale de l'existant.

## Fichiers clés

`src/parsing/ast.d/enums.rs` (`EnumDecl`/`EnumVariant`), `src/parsing/ast.d/patterns.rs` (`MatchArm`/`MatchPattern`), `src/parsing/parser.d/declarations.rs`, `src/sema/symbols.d/registers.rs`, `src/sema/typecheck.rs`, `docs/EBNF.md` (§21 Enums, §26 Match).
