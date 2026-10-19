# Sema : programmes invalides acceptés, qui plantent ou rendent des valeurs fausses

Audit du 2026-10-19. Reproduit sur le binaire courant.

## Cause racine principale

`types_compat` (`src/sema/typecheck.rs` ~l.2539) :

- une union en position « trouvée » est acceptée dès qu'**une** variante
  convient (~l.2545) — il faudrait que **toutes** conviennent ;
- `null` est accepté pour toute string, classe, tableau ou map (~l.2553) ;
- tableaux et maps covariants (~l.2570, 2576) alors qu'ils sont mutables ;
- paramètres de `Function` vérifiés dans le mauvais sens (~l.2597-2599,
  l'EBNF §6.5 annonce la contravariance).

## Constat

| # | Programme accepté | Résultat |
|---|---|---|
| 1 | `function mk(): A { return null }` puis `mk().v` ; `var a:A = null` | SIGSEGV (au lieu d'un rejet, ou au moins de « Cannot access 'v' on null ») |
| 2 | `var y:int = x` avec `x:int|string = "hello"` ; `var aa:A = u` avec `u:A|string` | adresse affichée (`10031121`), champ parasite |
| 3 | `var animals:array<Animal> = dogs` puis `animals.push(use Animal())` et `dogs[1].tricks` | SIGSEGV (même faille pour `map<K, Dog>`) |
| 4 | `var f:Function<int(Animal)> = nameless(d:Dog): int {…}` puis `f(use Animal())` | SIGSEGV |
| 5 | `B extends A` redéfinit `v()` avec un autre type de retour ; `a.v().tags[0]` | SIGSEGV ; paramètres différents → erreur interne Cranelift |
| 6 | `bu.v = "notauser"` sur `bu:Box<U>` (champ public d'un générique) ; `b.v = "zz"` sur `Box<int>` | SIGSEGV ; adresse affichée |
| 7 | `var u:U = match n { 5 => "notU"  default => use U() }` (type = dernière branche, ~l.2277) | SIGSEGV |
| 8 | `var bad:U = arr[1]` où l'élément `mixed` est une chaîne | SIGSEGV (`mixed` vers classe sans contrôle) |
| 9 | `-"abc"`, `-b` (bool), `-a` (tableau) (`UnaryOp::Neg => ty`, ~l.2214) ; `-mf` sur `mixed` = négation des bits | adresse, `true`, SIGSEGV ; `-mixed(2.5)` → `-444883753` |
| 10 | `var c:string = s[1]` (indexation d'une chaîne, interdite par l'EBNF §7.4) | SIGSEGV |
| 11 | `function g(x:int): string { if x greater 0 { return "pos" } }` (chemin sans `return`, interdit EBNF ~l.1606), aussi pour `nameless` | `null` / `0` |
| 12 | `match` sans `default` ni branche correspondante | `null` dans une `string` |
| 13 | deux `function main()`, deux `function foo()`, deux méthodes `m()`, deux `class A` dans un fichier | aucun diagnostic, la première gagne (seules les locales en double sont signalées) |
| 14 | `on e is Enfant` placé après `on e is Parent` | handler mort sans avertissement (E24 ne couvre que `on e` générique) |

## Piste

Réécrire `types_compat` (variance correcte, `null` réservé aux types
nullables, union → toutes les variantes), contrôler la conformité des
redéfinitions comme pour les interfaces, analyse « tous les chemins
retournent », `match` non exhaustif sans `default`, détection des
déclarations en double, contrôle de type à l'affectation d'un champ générique
monomorphisé. Vérifier ensuite les exemples : le resserrement peut révéler du
code qui s'appuyait sur ces trous.

## Priorité / Complexité

**Haute** (crashes) — **Structurel**.

## Fichiers clés

`src/sema/typecheck.rs`, `src/sema/symbols.d/`, `src/core/monomorph.rs`.
