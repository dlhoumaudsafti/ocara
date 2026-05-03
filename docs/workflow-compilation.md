# Workflow de Compilation Ocara

Ce document décrit l'architecture interne du compilateur Ocara et le processus de compilation d'un script.

## Structure des Dossiers

### `src/` - Compilateur Ocara

Le compilateur est organisé en phases successives, chacune transformant la représentation du code :

#### `src/core/`
**Structures de base communes** utilisées dans tout le compilateur :
- **ast.d/** : Définition de l'arbre syntaxique abstrait (AST) - nœuds représentant le code source parsé
- **token.d/** : Définition des tokens (mots-clés, opérateurs, littéraux) produits par le lexer
- **diagnostic.d/** : Système de diagnostic pour rapporter erreurs et warnings avec contexte
- **error.rs** : Types d'erreurs génériques du compilateur

#### `src/parsing/`
**Analyse du code source** en structures manipulables :
- **lexer.d/** : Analyse lexicale (source texte → tokens)
  - `scanner.d/` : Lecture caractère par caractère du fichier source
  - `tokenizer.d/` : Reconnaissance des tokens (nombres, strings, identifiants, opérateurs)
- **parser.d/** : Analyse syntaxique (tokens → AST)
  - Parse les déclarations (fonctions, classes, interfaces)
  - Parse les expressions et statements selon la grammaire Ocara

#### `src/sema/`
**Analyse sémantique** - vérification de la cohérence du programme :
- **symbols.d/** : Table des symboles (variables, fonctions, classes)
- **scope.rs** : Gestion des portées (scopes imbriqués)
- **typecheck.rs** : Vérification des types (compatibilité, inférence)
- **error.rs** : Erreurs sémantiques (type mismatch, symbole non trouvé, etc.)

#### `src/lower/`
**Lowering** - transformation AST → représentation intermédiaire (IR) :
- **builder.d/** : Construction de l'IR (création d'instructions, blocs, fonctions)
- **expr.d/** : Lowering des expressions (arithmétique, appels, accès membres)
- **stmt.d/** : Lowering des statements (if, while, return, assignations)
  - `statements.d/` : Statements complexes décomposés en instructions simples

#### `src/ir/`
**Représentation intermédiaire (IR)** - format interne avant génération native :
- **types.rs** : Système de types de l'IR (Int, Float, Ptr, Bool)
- **inst.rs** : Instructions IR (Add, Load, Store, Call, Branch, etc.)
- **func.rs** : Fonctions IR avec paramètres, blocs basiques, et graphe de contrôle
- **module.rs** : Module IR contenant toutes les fonctions et métadonnées

#### `src/codegen/`
**Génération de code natif** via Cranelift :
- **desc.d/** : Descripteurs de classes (layouts mémoire pour les objets)
- **emit.d/** : Émission de code machine
  - `emitter.rs` : Structure principale orchestrant la compilation Cranelift
  - `instructions.d/` : Émission par catégorie d'instructions (arithmetic, memory, calls, control flow, etc.)
  - `helpers.rs` : Utilitaires de conversion de types
- **link.rs** : Linkage des fonctions et résolution des symboles
- **runtime.rs** : Interface avec la bibliothèque runtime

#### `src/builtins/`
**Fonctions built-in** du langage (Array, String, IO, Math, HTTP, System, Thread, etc.) - implémentation en Rust appelable depuis Ocara.

### `runtime/` - Bibliothèque Runtime

Bibliothèque Rust compilée séparément et linkée au code généré :
- **httpserver.rs** : Serveur HTTP asynchrone
- **httprequest.rs** : Client HTTP
- **thread.rs** : Gestion des threads
- **lib.rs** : Point d'entrée et exports

Le runtime fournit des fonctions natives complexes (I/O, réseau, concurrence) que le code généré appelle via FFI.

---

## Workflow de Compilation

Voici les étapes successives de la compilation d'un fichier `.oc` en exécutable :

### 1️⃣ **Lexing** (Analyse Lexicale)
**Input** : Code source (`main.oc`)  
**Output** : Stream de tokens  
**Responsable** : `src/parsing/lexer.d/`

Le scanner lit le fichier caractère par caractère et le tokenizer identifie les unités lexicales :
- Mots-clés (`func`, `class`, `if`, `return`)
- Identifiants (`myVariable`, `MyClass`)
- Littéraux (`42`, `"hello"`, `true`)
- Opérateurs (`+`, `==`, `->`)
- Délimiteurs (`{`, `}`, `;`)

Les tokens incluent leur position dans le fichier (ligne, colonne) pour les diagnostics.

### 2️⃣ **Parsing** (Analyse Syntaxique)
**Input** : Stream de tokens  
**Output** : AST (Abstract Syntax Tree)  
**Responsable** : `src/parsing/parser.d/`

Le parser consomme les tokens et construit un arbre représentant la structure du programme selon la grammaire Ocara :
- Déclarations de fonctions → `FuncDecl` nodes
- Classes et interfaces → `ClassDecl`, `InterfaceDecl`
- Expressions → `BinaryExpr`, `CallExpr`, `LiteralExpr`
- Statements → `IfStmt`, `WhileStmt`, `ReturnStmt`

L'AST préserve la structure hiérarchique du code mais ne contient pas encore d'informations de types.

### 3️⃣ **Semantic Analysis** (Analyse Sémantique)
**Input** : AST non typé  
**Output** : AST typé + table des symboles  
**Responsable** : `src/sema/`

Phase de vérification et enrichissement :

**a) Construction de la table des symboles** (`symbols.d/`)
- Enregistre toutes les déclarations (variables, fonctions, classes)
- Détecte les redéfinitions
- Gère les scopes imbriqués

**b) Vérification des types** (`typecheck.rs`)
- Résout les types de toutes les expressions
- Vérifie la compatibilité des types dans les opérations
- Valide les appels de fonctions (nombre et types d'arguments)
- Vérifie l'existence des membres de classes

**c) Détection d'erreurs sémantiques**
- Variables non déclarées
- Types incompatibles
- Fonctions appelées avec mauvais arguments
- Accès à des membres inexistants

Si cette phase réussit, le programme est **sémantiquement correct**.

### 4️⃣ **Lowering** (AST → IR)
**Input** : AST typé  
**Output** : IR (Intermediate Representation)  
**Responsable** : `src/lower/`

Transformation de l'AST haut-niveau vers une représentation plus bas-niveau :

**a) Simplification** (`builder.d/`)
- Crée le module IR et ses fonctions
- Alloue des registres virtuels (SSA form)
- Décompose les expressions complexes en séquences d'instructions simples

**b) Lowering des expressions** (`expr.d/`)
- `x + y * 2` devient : `temp1 = Mul y, 2; temp2 = Add x, temp1`
- Appels de fonctions → instructions `Call` avec arguments
- Accès membres → calculs d'offsets et `Load`/`Store`

**c) Lowering des statements** (`stmt.d/`)
- `if` → `Branch` vers blocs conditionnels
- `while` → blocs avec `Jump` arrière
- `return` → instruction `Return`

L'IR ressemble à un assembleur virtuel indépendant de la plateforme, avec un nombre illimité de registres.

### 5️⃣ **Code Generation** (IR → Code Natif)
**Input** : Module IR  
**Output** : Code machine natif  
**Responsable** : `src/codegen/`

Utilise **Cranelift** comme backend JIT/AOT :

**a) Préparation** (`desc.d/`, `emitter.rs`)
- Calcule les layouts mémoire des classes (offsets des champs)
- Pré-déclare toutes les fonctions pour permettre les appels mutuels
- Prépare les constantes strings

**b) Émission** (`emit.d/instructions.d/`)
- Traduit chaque instruction IR en instructions Cranelift
- Gère les conversions de types (ex: F64 ↔ I64 pour compatibilité)
- Optimise les accès mémoire
- Génère les appels aux fonctions runtime

**c) Compilation**
- Cranelift compile l'IR Cranelift en code machine natif (x86_64, ARM, etc.)
- Applique des optimisations (register allocation, instruction selection)
- Produit un objet relocatable

### 6️⃣ **Linking**
**Input** : Code natif + runtime library  
**Output** : Exécutable final  
**Responsable** : `src/codegen/link.rs`

- Résout les symboles entre le code généré et la bibliothèque runtime
- Linke les fonctions built-in (compilées depuis `runtime/`)
- Produit un exécutable natif ELF (Linux) ou PE (Windows)

---

## Exemple de Flux

### main.oc:
```php
  import ocara.IO
  
  function main():void {
    var x:int = 42;
    IO::writeln(x);
  }
```

### Lexing (src/parsing/lexer.d/)
```json
[import, ident("ocara"), dot, ident("IO"), newline,
 function, ident("main"), lparen, rparen, colon, ident("void"), lbrace,
 var, ident("x"), colon, ident("int"), eq, int(42), semicolon,
 ident("IO"), coloncolon, ident("writeln"), lparen, ident("x"), rparen, semicolon,
 rbrace]
```

### Parsing (src/parsing/parser.d/)
```nginx
Module {
  imports: [ImportDecl { module: "ocara.IO" }],
  decls: [
    FuncDecl {
      name: "main",
      params: [],
      return_type: "void",
      body: BlockStmt [
        VarStmt { name: "x", type: "int", value: IntLit(42) },
        ExprStmt { 
          StaticCallExpr { 
            class: "IO", 
            method: "writeln", 
            args: [VarExpr("x")] 
          }
        }
      ]
    }
  ]
}
```

### Semantic Analysis (src/sema/)
```nginx
Module {
  imports: [ImportDecl { module: "ocara.IO", resolved: builtin }],
  decls: [
    FuncDecl {
      name: "main",
      type: () -> Void,
      body: [
        VarStmt { name: "x", type: Int, value: IntLit(42) },
        ExprStmt {
          StaticCallExpr {
            class: IO (builtin),
            method: writeln,
            type: (Int) -> Void,
            args: [VarExpr("x", type: Int)]
          }
        }
      ]
    }
  ]
}
```

### Lowering IR (src/lower/)
```ini
function main():void
block0:
  v0 = ConstInt 42           ; charge la constante 42
  v1 = Alloca Int            ; alloue un slot sur la stack pour x
  Store v0, v1               ; stocke 42 dans x
  v2 = Load v1               ; charge x pour l'appel
  Call @io_writeln_int(v2)   ; appelle IO::writeln avec x
  Return                     ; retourne (void)
```

### Codegen Craftlift -> Machine (src/codegen/emit.d/)
```ini
main:
  push rbp                   ; prologue
  mov rbp, rsp
  sub rsp, 16                ; alloue stack frame
  
  mov DWORD [rbp-4], 42      ; x = 42
  mov edi, DWORD [rbp-4]     ; charge x dans edi (1er argument)
  call io_writeln_int        ; appel IO::writeln
  
  add rsp, 16                ; libère stack frame
  pop rbp                    ; épilogue
  ret
```

### Linking (src/codegen/link.rs)
```
Résolution: io_writeln_int → src/builtins/io.rs::writeln_int()
Exécutable natif avec runtime intégré (libc + builtins)
```

---

## Points Clés

1. **Pipeline linéaire** : chaque phase transforme la représentation sans retour en arrière
2. **Séparation des concerns** : lexing/parsing ne gèrent pas les types, sema ne génère pas de code
3. **IR centrale** : découple le frontend (parsing/sema) du backend (codegen)
4. **Cranelift** : génération de code optimisé multi-plateforme sans écrire d'assembleur
5. **Runtime séparé** : bibliothèque Rust pour fonctionnalités complexes (async, réseau)
6. **Modularité** : chaque module `.d/` découpe les responsabilités pour la maintenabilité

Cette architecture permet d'ajouter facilement des optimisations, de nouveaux backends ou d'évoluer la syntaxe sans tout récrire.
