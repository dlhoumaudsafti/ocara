# `try` / `on` et générateurs : `break`, `continue`, `return` et locales perdus

Audit du 2026-10-19. Reproduit sur le binaire courant.

## Constat

Le corps d'un `try` et ses handlers sont abaissés comme des fonctions
séparées (`src/lower/stmt.d/statements.d/exceptions.rs` ~l.164, 277) :

1. **`break` / `continue` dans un `try` ou un `on` ignorés sans erreur.**
   ```ocara
   for i in 0 until 5 {
       try {
           if i equal 1 { continue }
           if i equal 3 { break }
           s = s + `${i}`
       } on e { }
   }                                 // s = "01234" au lieu de "02"
   ```
   `while true { try { if n equal 3 { break } } on e {} }` ne s'arrête pas
   (`loop_stack` vide dans la fonction séparée).
2. **`return` dans le corps d'un `try`** : `try { return "fromtry" } on e {…}`
   → `codegen error: VerifierError … arguments of return must match function
   signature` (`body_ret_ty = IrType::Void`). En `void`, le `return` ne sort
   que du corps du try (la suite s'exécute). `return` dans `on` fonctionne.

Générateurs :

3. **Après un `raise` rattrapé dans le générateur, les locales comptées sont
   perdues** : `var xs:array<int> = [7, 8, 9]`, `try { raise "boom" } on e {}`,
   puis `emit xs.len()` → `0` (attendu `3`) ; une chaîne → `null`.
4. **Fermeture créée dans un générateur qui capture un paramètre du
   générateur** : lit `0` (`genD(9)` émet `0`). Copier le paramètre dans une
   locale contourne.
5. **Fermeture émise par un générateur, appelée après son épuisement** : ses
   captures string/tableau valent `null`/`0` (le frame relâche ce que la
   cellule devrait garder).

## Piste

Propager la pile de boucles et le type de retour au corps du `try` (codes de
sortie : normal / break / continue / return + valeur, testés après l'appel),
ou abaisser le `try` en ligne comme dans un générateur. Pour les générateurs :
recharger les locales depuis le frame après un `longjmp`, partager les
paramètres capturés via `gen_cells`, retenir les valeurs capturées au lieu de
les rendre à la destruction du frame.

## Priorité / Complexité

**Haute** (valeurs fausses, boucles infinies) — **Structurel**.

## Fichiers clés

`src/lower/stmt.d/statements.d/exceptions.rs`,
`src/lower/stmt.d/statements.d/loops.rs`, `src/lower/builder.d/message_gen.rs`.
