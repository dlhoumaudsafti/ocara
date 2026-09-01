# Guide : Ajouter un nouveau type de variable à Ocara

Ce guide explique comment créer un nouveau type de variable dans le système de types d'Ocara. Les types de variables définissent la syntaxe, le parsing, la vérification de types, et le lowering vers l'IR.

## Vue d'ensemble

Un type de variable Ocara se compose de :
- **Définition AST** (`src/parsing/ast.rs`) : variante dans l'enum `Type`
- **Parsing** (`src/parsing/parser.d/types_parsing.rs`) : reconnaissance de la syntaxe
- **Lexer** (`src/parsing/token.rs` et `lexer.d/tokenizer.d/keywords.rs`) : tokens pour mots-clés
- **Type checking** (`src/sema/typecheck.rs`) : vérification de compatibilité
- **Lowering IR** (`src/ir/types.rs`) : conversion vers types Cranelift
- **Métadonnées** (`src/lower/builder.d/types.rs`) : tracking des informations de type
- **Tests** (`src/parsing/parser.d/tests.rs`) : tests unitaires de parsing

## Exemple : Type `array<T>`

Nous allons détailler l'implémentation du type `array<T>` comme exemple concret.

---

## Étape 1 : Définir le type dans l'AST

**Fichier : `src/parsing/ast.rs`**

Ajouter une variante à l'enum `Type` :

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    // Types primitifs
    Int,
    Float,
    String,
    Bool,
    Mixed,
    Void,
    Null,
    
    // Types composites
    Array(Box<Type>),           // array<T>
    Map(Box<Type>, Box<Type>),  // map<K,V>
    
    // Types nommés
    Named(String),              // MyClass
    Qualified(Vec<String>),     // module.MyClass
    
    // Types génériques
    Generic {                   // List<T>
        name: String,
        args: Vec<Type>,
    },
    
    // Types fonction
    Function {                  // Function<ReturnType(ParamType, ...)>
        ret_ty: Box<Type>,
        param_tys: Vec<Type>,
    },
    
    // Types union
    Union(Vec<Type>),          // T | U | V
}
```

**Points clés :**
- Utiliser `Box<Type>` pour les types récursifs (tableaux, maps)
- Les types génériques utilisent un `Vec<Type>` pour les arguments
- Les types union permettent `int | null` par exemple

---

## Étape 2 : Ajouter le token au lexer

**Fichier : `src/parsing/token.rs`**

Dans l'enum `TokenKind`, ajouter le token pour le mot-clé :

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // ... autres tokens
    
    // ── Types primitifs ───────────────────────────────────────────────────────
    TInt,
    TFloat,
    TString,
    TBool,
    TMixed,
    TArray,    // <-- Nouveau token pour "array"
    TMap,
    TVoid,
    
    // ... suite
}
```

**Fichier : `src/parsing/lexer.d/tokenizer.d/keywords.rs`**

Mapper le mot-clé au token :

```rust
pub(in crate::parsing::lexer) fn keyword_or_ident(s: &str) -> TokenKind {
    match s {
        // ... autres mots-clés
        
        // Types primitifs
        "int"        => TokenKind::TInt,
        "float"      => TokenKind::TFloat,
        "string"     => TokenKind::TString,
        "bool"       => TokenKind::TBool,
        "mixed"      => TokenKind::TMixed,
        "array"      => TokenKind::TArray,  // <-- Nouveau
        "map"        => TokenKind::TMap,
        "void"       => TokenKind::TVoid,
        
        // ... suite
        _            => TokenKind::Ident(s.to_string()),
    }
}
```

**Fichier : `src/parsing/lexer.d/tests.rs`**

Ajouter un test pour le nouveau token :

```rust
#[test]
fn test_type_keywords() {
    let tks = kinds("int float string bool mixed array map void");
    assert_eq!(
        tks,
        vec![
            TokenKind::TInt, TokenKind::TFloat, TokenKind::TString,
            TokenKind::TBool, TokenKind::TMixed, TokenKind::TArray, 
            TokenKind::TMap, TokenKind::TVoid,
        ]
    );
}
```

---

## Étape 3 : Implémenter le parsing

**Fichier : `src/parsing/parser.d/types_parsing.rs`**

Ajouter le cas dans `parse_type_base()` :

```rust
impl Parser {
    pub(super) fn parse_type(&mut self) -> ParseResult<Type> {
        let first = self.parse_type_base()?;

        // Type union : `T | U | ...`
        if self.check_exact(&TokenKind::Pipe) {
            let mut variants = vec![first];
            while self.check_exact(&TokenKind::Pipe) {
                self.advance();
                variants.push(self.parse_type_base()?);
            }
            return Ok(Type::Union(variants));
        }

        Ok(first)
    }

    fn parse_type_base(&mut self) -> ParseResult<Type> {
        let base = match self.peek_kind().clone() {
            // Types primitifs
            TokenKind::TInt    => { self.advance(); Type::Int    }
            TokenKind::TFloat  => { self.advance(); Type::Float  }
            TokenKind::TString => { self.advance(); Type::String }
            TokenKind::TBool   => { self.advance(); Type::Bool   }
            TokenKind::TMixed  => { self.advance(); Type::Mixed  }
            TokenKind::TVoid   => { self.advance(); Type::Void   }
            TokenKind::LitNull => { self.advance(); Type::Null   }

            // Type map : map<K, V>
            TokenKind::TMap => {
                self.advance();
                self.eat(&TokenKind::Lt)?;
                let k = self.parse_type()?;
                self.eat(&TokenKind::Comma)?;
                let v = self.parse_type()?;
                self.eat(&TokenKind::Gt)?;
                Type::Map(Box::new(k), Box::new(v))
            }

            // Type array : array<T>
            TokenKind::TArray => {
                self.advance();
                self.eat(&TokenKind::Lt)?;
                let elem_ty = self.parse_type()?;
                self.eat(&TokenKind::Gt)?;
                Type::Array(Box::new(elem_ty))
            }

            // Types nommés et génériques
            TokenKind::Ident(name) => {
                self.advance();
                
                // Function<ReturnType(ParamType, ...)>
                if name == "Function" {
                    self.eat(&TokenKind::Lt)?;
                    let ret_ty = self.parse_type()?;
                    self.eat(&TokenKind::LParen)?;
                    let mut param_tys = Vec::new();
                    if !self.check_exact(&TokenKind::RParen) {
                        param_tys.push(self.parse_type()?);
                        while self.check_exact(&TokenKind::Comma) {
                            self.advance();
                            param_tys.push(self.parse_type()?);
                        }
                    }
                    self.eat(&TokenKind::RParen)?;
                    self.eat(&TokenKind::Gt)?;
                    return Ok(Type::Function {
                        ret_ty: Box::new(ret_ty),
                        param_tys,
                    });
                }
                
                // Type qualifié : module.Class
                if self.check_exact(&TokenKind::Dot) {
                    let mut parts = vec![name];
                    while self.check_exact(&TokenKind::Dot) {
                        self.advance();
                        parts.push(self.eat_ident()?.0);
                    }
                    Type::Qualified(parts)
                } 
                // Type générique : List<T>, Cache<K, V>
                else if self.check_exact(&TokenKind::Lt) {
                    self.advance();
                    let mut args = Vec::new();
                    args.push(self.parse_type()?);
                    while self.check_exact(&TokenKind::Comma) {
                        self.advance();
                        args.push(self.parse_type()?);
                    }
                    self.eat(&TokenKind::Gt)?;
                    Type::Generic { name, args }
                } 
                // Type nommé simple
                else {
                    Type::Named(name)
                }
            }

            other => {
                return Err(ParseError::new(
                    format!("expected type, found {:?}", other),
                    self.span(),
                ))
            }
        };

        Ok(base)
    }
}
```

**Points clés :**
- Pour un type générique avec angle brackets : `self.eat(&TokenKind::Lt)?` pour `<`
- Utiliser `self.parse_type()?` récursivement pour les arguments de type
- `self.eat(&TokenKind::Gt)?` pour `>`
- Retourner le type AST correspondant (`Type::Array(Box::new(elem_ty))`)

---

## Étape 4 : Ajouter le test de parsing

**Fichier : `src/parsing/parser.d/tests.rs`**

Ajouter un test unitaire :

```rust
#[test]
fn test_type_array() {
    let p = parse("function foo(a:array<int>): int {}");
    assert_eq!(p.functions[0].params[0].ty, Type::Array(Box::new(Type::Int)));
}

#[test]
fn test_type_array_nested() {
    let p = parse("function foo(m:array<array<string>>): void {}");
    assert_eq!(
        p.functions[0].params[0].ty,
        Type::Array(Box::new(Type::Array(Box::new(Type::String))))
    );
}

#[test]
fn test_type_array_map() {
    let p = parse("function foo(data:array<map<string, int>>): void {}");
    assert_eq!(
        p.functions[0].params[0].ty,
        Type::Array(Box::new(Type::Map(Box::new(Type::String), Box::new(Type::Int))))
    );
}
```

---

## Étape 5 : Conversion vers IR

**Fichier : `src/ir/types.rs`**

Implémenter la conversion de `Type` AST vers `IrType` :

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum IrType {
    I64,   // int, bool, pointeurs
    F64,   // float
    Ptr,   // string, array, map, objets
    Void,
}

impl IrType {
    /// Convertit un Type AST en IrType
    pub fn from_ast(ty: &Type) -> Self {
        match ty {
            Type::Int      => IrType::I64,
            Type::Float    => IrType::F64,
            Type::String   => IrType::Ptr,
            Type::Bool     => IrType::I64,
            Type::Mixed    => IrType::I64,  // Boxed value
            Type::Void     => IrType::Void,
            Type::Null     => IrType::I64,
            
            // Types composites → toujours Ptr
            Type::Array(_) => IrType::Ptr,
            Type::Map(_, _) => IrType::Ptr,
            
            // Types nommés (classes) → Ptr
            Type::Named(_) => IrType::Ptr,
            Type::Qualified(_) => IrType::Ptr,
            Type::Generic { .. } => IrType::Ptr,
            
            // Types fonction → Ptr
            Type::Function { .. } => IrType::Ptr,
            
            // Union → choisir le type le plus large
            Type::Union(variants) => {
                if variants.iter().any(|v| matches!(v, Type::String | Type::Array(_) | Type::Map(_, _))) {
                    IrType::Ptr
                } else if variants.iter().any(|v| matches!(v, Type::Float)) {
                    IrType::F64
                } else {
                    IrType::I64
                }
            }
        }
    }
}
```

**Points clés :**
- Les types composites (array, map) sont toujours représentés par des pointeurs (`IrType::Ptr`)
- Les types primitifs mappent directement (int → I64, float → F64)
- Les unions doivent choisir le type le plus large qui peut représenter toutes les variantes

---

## Étape 6 : Métadonnées du builder

**Fichier : `src/lower/builder.d/types.rs`**

Ajouter des champs pour tracker les métadonnées de type :

```rust
pub struct LowerBuilder<'m> {
    // ... autres champs
    
    /// Type des éléments pour les variables tableau (ex: jours:string[] → Ptr)
    pub elem_types: HashMap<String, IrType>,
    
    /// Type AST des éléments pour les variables tableau (pour métadonnées complètes)
    pub elem_ast_types: HashMap<String, Type>,
    
    /// Noms des variables déclarées comme map<K,V> (pour Expr::Index → __map_get)
    pub map_vars: HashSet<String>,
    
    /// Mapping nom_variable → nom_classe (pour résoudre les appels de méthode)
    pub var_class: HashMap<String, String>,
    
    // ... autres champs
}

impl<'m> LowerBuilder<'m> {
    pub fn new(
        module: &'m mut IrModule,
        name: String,
        params: Vec<IrParam>,
        ret_ty: IrType,
    ) -> Self {
        let func = IrFunction::new(name, params, ret_ty);
        Self {
            // ... autres initialisations
            elem_types: HashMap::new(),
            elem_ast_types: HashMap::new(),
            map_vars: HashSet::new(),
            var_class: HashMap::new(),
            // ... autres initialisations
        }
    }
}
```

**Points clés :**
- `elem_types` : type IR des éléments (pour le codegen)
- `elem_ast_types` : type AST complet (pour préserver métadonnées comme map<K,V>)
- `map_vars` : variables qui sont des maps (pour générer __map_get au lieu de __array_get)
- `var_class` : mapping variable → classe (pour résoudre méthodes d'instance)

---

## Étape 7 : Enregistrer les métadonnées lors des déclarations

**Fichier : `src/lower/stmt.d/statements.d/variables.rs`**

Lors de la déclaration d'une variable, enregistrer ses métadonnées :

```rust
pub fn lower_var(
    builder: &mut LowerBuilder,
    name: &str,
    ty: &Type,
    value: &Expr,
    mutable: bool,
) {
    let ir_ty = IrType::from_ast(ty);
    
    // Si c'est un tableau, enregistrer le type des éléments
    if let Type::Array(inner) = ty {
        builder.elem_types.insert(name.to_string(), IrType::from_ast(inner));
        builder.elem_ast_types.insert(name.to_string(), (**inner).clone());
    }
    
    // Si c'est une map, marquer la variable pour Expr::Index → __map_get
    // et enregistrer le type des valeurs dans elem_types
    if let Type::Map(_, val_ty) = ty {
        builder.map_vars.insert(name.to_string());
        builder.elem_types.insert(name.to_string(), IrType::from_ast(val_ty));
    }
    
    // Si c'est un type de classe, enregistrer le mapping var → classe
    if let Type::Named(class_name) = ty {
        builder.var_class.insert(name.to_string(), class_name.clone());
    }
    
    // Les variables array ont automatiquement accès aux méthodes de Array
    if let Type::Array(_) = ty {
        builder.var_class.insert(name.to_string(), "Array".to_string());
    }
    
    // Les variables map ont automatiquement accès aux méthodes de Map
    if let Type::Map(_, _) = ty {
        builder.var_class.insert(name.to_string(), "Map".to_string());
    }
    
    // ... suite de l'implémentation
}
```

**Même chose pour les paramètres de fonction :**

**Fichier : `src/lower/builder.d/functions.rs`**

```rust
// Dans la fonction qui traite les paramètres
for (idx, param) in func.params.iter().enumerate() {
    // ... autres traitements
    
    // Si le paramètre est un tableau, enregistrer le type d'élément
    if let Type::Array(inner) = &param.ty {
        let elem_ty = IrType::from_ast(inner);
        builder.elem_types.insert(param.name.clone(), elem_ty);
        builder.elem_ast_types.insert(param.name.clone(), (**inner).clone());
    }
    
    // Marquer les paramètres de type map<>
    if let Type::Map(_, _) = &param.ty {
        builder.map_vars.insert(param.name.clone());
    }
    
    // ... suite
}
```

---

## Étape 8 : Utiliser les métadonnées dans les boucles

**Fichier : `src/lower/stmt.d/statements.d/loops.rs`**

Exemple : boucle `for item in array` doit propager les métadonnées à la variable d'itération :

```rust
pub fn lower_for_in(
    builder: &mut LowerBuilder,
    var: &str,
    iter: &Expr,
    body: &Block,
) {
    // ... code de génération de la boucle
    
    // Créer la variable d'itération
    let elem_slot = builder.declare_local(var, elem_ty.clone(), false);
    builder.emit(Inst::Store { ptr: elem_slot, src: elem });
    
    // Si l'itérateur est une variable avec un type d'élément map, 
    // enregistrer les métadonnées pour la variable d'itération
    if let Expr::Ident(iter_name, _) = iter {
        if let Some(elem_ast_ty) = builder.elem_ast_types.get(iter_name.as_str()) {
            // Si l'élément est un map, propager les métadonnées
            if let Type::Map(_, val_ty) = elem_ast_ty {
                builder.map_vars.insert(var.to_string());
                builder.elem_types.insert(var.to_string(), IrType::from_ast(val_ty));
                builder.var_class.insert(var.to_string(), "Map".to_string());
            }
            
            // Si l'élément est un tableau, propager
            if let Type::Array(inner) = elem_ast_ty {
                builder.elem_types.insert(var.to_string(), IrType::from_ast(inner));
                builder.elem_ast_types.insert(var.to_string(), (**inner).clone());
                builder.var_class.insert(var.to_string(), "Array".to_string());
            }
            
            // Si l'élément est une classe, propager
            if let Type::Named(class_name) = elem_ast_ty {
                builder.var_class.insert(var.to_string(), class_name.clone());
            }
        }
    }
    
    // ... suite de la boucle
}
```

**Points clés :**
- Les variables créées dans les boucles doivent hériter des métadonnées de leur source
- Utiliser `elem_ast_types` pour récupérer le type complet (pas juste IrType::Ptr)
- Propager toutes les métadonnées nécessaires (map_vars, var_class, etc.)

---

## Étape 9 : Type checking

**Fichier : `src/sema/typecheck.rs`**

Implémenter la vérification de compatibilité entre types :

```rust
impl TypeChecker {
    fn check_type_compat(&self, expected: &Type, actual: &Type, span: &Span) -> CheckResult<()> {
        match (expected, actual) {
            // Identiques
            (Type::Int, Type::Int) => Ok(()),
            (Type::Float, Type::Float) => Ok(()),
            (Type::String, Type::String) => Ok(()),
            (Type::Bool, Type::Bool) => Ok(()),
            (Type::Void, Type::Void) => Ok(()),
            
            // Arrays : vérifier récursivement les types d'éléments
            (Type::Array(e1), Type::Array(e2)) => {
                self.check_type_compat(e1, e2, span)
            }
            
            // Maps : vérifier les clés et valeurs
            (Type::Map(k1, v1), Type::Map(k2, v2)) => {
                self.check_type_compat(k1, k2, span)?;
                self.check_type_compat(v1, v2, span)
            }
            
            // Mixed accepte tout
            (Type::Mixed, _) | (_, Type::Mixed) => Ok(()),
            
            // Null compatible avec les types référence
            (Type::Named(_), Type::Null) => Ok(()),
            (Type::Array(_), Type::Null) => Ok(()),
            (Type::Map(_, _), Type::Null) => Ok(()),
            
            // Union : l'actual doit être compatible avec au moins une variante
            (Type::Union(variants), actual_ty) => {
                for variant in variants {
                    if self.check_type_compat(variant, actual_ty, span).is_ok() {
                        return Ok(());
                    }
                }
                Err(CheckError::new(
                    format!("type mismatch: expected one of union variants, got {:?}", actual_ty),
                    span.clone(),
                ))
            }
            
            // ... autres cas
            
            _ => Err(CheckError::new(
                format!("type mismatch: expected {:?}, got {:?}", expected, actual),
                span.clone(),
            ))
        }
    }
}
```

---

## Étape 10 : Documentation

**Fichier : `docs/EBNF.md`**

Documenter la syntaxe du nouveau type :

```ebnf
type_expr ::=
    | type_base ('|' type_base)*              # Union : int | null

type_base ::=
    | 'int' | 'float' | 'string' | 'bool' | 'mixed' | 'void' | 'null'
    | 'array' '<' type_expr '>'               # array<T>
    | 'map' '<' type_expr ',' type_expr '>'   # map<K,V>
    | IDENT                                   # MyClass
    | IDENT ('.' IDENT)+                      # module.MyClass
    | IDENT '<' type_args '>'                 # Generic<T>
    | 'Function' '<' function_type '>'        # Function<RetType(Params)>

type_args ::= type_expr (',' type_expr)*

function_type ::= type_expr '(' (type_expr (',' type_expr)*)? ')'
```

**Fichier : Documentation utilisateur (ex: `docs/types.md`)**

```markdown
## Arrays

Les arrays sont des collections ordonnées d'éléments du même type.

### Syntaxe

\```ocara
var numbers:array<int> = [1, 2, 3, 4, 5]
var names:array<string> = ["Alice", "Bob", "Charlie"]
var matrix:array<array<int>> = [[1, 2], [3, 4]]
\```

### Arrays de types composites

\```ocara
// Array de maps
var users:array<map<string, mixed>> = [
    {"name": "Alice", "age": 30},
    {"name": "Bob", "age": 25}
]

// Itération
for user in users {
    IO::writeln(\`Name: \${user["name"]}\`)
}
\```

### Méthodes

Les arrays ont accès aux méthodes de la classe builtin `Array` :

\```ocara
var arr:array<int> = [3, 1, 4, 1, 5]
var len:int = Array::len(arr)          // 5
var sorted:array<int> = Array::sort(arr)  // [1, 1, 3, 4, 5]
\```
```

---

## Checklist

Lors de l'ajout d'un nouveau type, vérifier :

- [ ] ✅ Variante ajoutée à `Type` dans `src/parsing/ast.rs`
- [ ] ✅ Token ajouté à `TokenKind` dans `src/parsing/token.rs`
- [ ] ✅ Mapping mot-clé → token dans `src/parsing/lexer.d/tokenizer.d/keywords.rs`
- [ ] ✅ Test lexer dans `src/parsing/lexer.d/tests.rs`
- [ ] ✅ Parsing implémenté dans `src/parsing/parser.d/types_parsing.rs`
- [ ] ✅ Tests de parsing dans `src/parsing/parser.d/tests.rs`
- [ ] ✅ Conversion `Type → IrType` dans `src/ir/types.rs`
- [ ] ✅ Métadonnées dans `LowerBuilder` (`src/lower/builder.d/types.rs`)
- [ ] ✅ Enregistrement métadonnées pour variables (`src/lower/stmt.d/statements.d/variables.rs`)
- [ ] ✅ Enregistrement métadonnées pour paramètres (`src/lower/builder.d/functions.rs`)
- [ ] ✅ Propagation métadonnées dans boucles (`src/lower/stmt.d/statements.d/loops.rs`)
- [ ] ✅ Type checking dans `src/sema/typecheck.rs`
- [ ] ✅ Documentation EBNF dans `docs/EBNF.md`
- [ ] ✅ Documentation utilisateur
- [ ] ✅ Exemples de code

---

## Bonnes pratiques

### 1. Cohérence syntaxique

Garder une syntaxe cohérente avec les types existants :
- Types génériques : `name<args>` (array<T>, map<K,V>)
- Types qualifiés : `module.Class`
- Types union : `T | U | V`

### 2. Tests exhaustifs

Tester tous les cas d'usage :
- Type simple : `array<int>`
- Type imbriqué : `array<array<string>>`
- Type composite : `array<map<string, int>>`
- Type union : `array<int | null>`

### 3. Métadonnées complètes

Toujours enregistrer à la fois :
- `elem_types` : type IR pour le codegen
- `elem_ast_types` : type AST pour les métadonnées complètes

### 4. Propagation dans les boucles

Les variables créées dans les boucles doivent hériter des métadonnées :
```rust
if let Some(elem_ast_ty) = builder.elem_ast_types.get(iter_name) {
    // Propager les métadonnées à la variable d'itération
}
```

### 5. Type checking récursif

Pour les types composites, vérifier récursivement :
```rust
(Type::Array(e1), Type::Array(e2)) => {
    self.check_type_compat(e1, e2, span)
}
```

---

## Erreurs courantes

### ❌ Oublier elem_ast_types

**Problème :** Enregistrer seulement `elem_types` (IrType::Ptr)

**Impact :** Perte des métadonnées comme `map<string, int>` → variable d'itération ne sait pas qu'elle est un map

**Solution :**
```rust
builder.elem_types.insert(name.to_string(), IrType::from_ast(inner));
builder.elem_ast_types.insert(name.to_string(), (**inner).clone());
```

### ❌ Ne pas propager dans les boucles

**Problème :** Variable d'itération sans métadonnées

**Impact :** `for user in users` → `user["name"]` ne fonctionne pas

**Solution :** Vérifier et propager dans `lower_for_in()` :
```rust
if let Type::Map(_, val_ty) = elem_ast_ty {
    builder.map_vars.insert(var.to_string());
    builder.elem_types.insert(var.to_string(), IrType::from_ast(val_ty));
}
```

### ❌ Oublier les paramètres de fonction

**Problème :** Métadonnées enregistrées pour variables mais pas pour paramètres

**Impact :** Fonction `fn process(items:array<map<string, int>>)` ne peut pas itérer sur items

**Solution :** Traiter les paramètres dans `src/lower/builder.d/functions.rs`

### ❌ Type checking incomplet

**Problème :** Ne pas vérifier récursivement les types imbriqués

**Impact :** `array<int>` accepte `array<string>`

**Solution :** Implémenter vérification récursive dans `check_type_compat()`

---

## Debugging

### Afficher les types AST

Ajouter du debug logging :
```rust
eprintln!("Type: {:?}", ty);
eprintln!("IrType: {:?}", IrType::from_ast(ty));
```

### Vérifier les métadonnées

Dans le lowering :
```rust
eprintln!("elem_types: {:?}", builder.elem_types);
eprintln!("elem_ast_types: {:?}", builder.elem_ast_types);
eprintln!("map_vars: {:?}", builder.map_vars);
```

### Tester avec un exemple minimal

Créer un fichier de test simple :
```ocara
main {
    var arr:array<int> = [1, 2, 3]
    for x in arr {
        IO::writeln(x)
    }
}
```

Compiler avec verbose :
```bash
./target/release/ocara test.oc
```

---

## Conclusion

L'ajout d'un nouveau type nécessite :
1. **AST** : définition du type
2. **Lexer** : reconnaissance des mots-clés
3. **Parser** : syntaxe et construction de l'AST
4. **IR** : conversion vers types machine
5. **Métadonnées** : tracking des informations de type
6. **Type checking** : vérification de compatibilité
7. **Tests** : validation complète
8. **Documentation** : guide utilisateur

En suivant ces étapes méthodiquement, vous pouvez étendre le système de types d'Ocara de manière robuste et maintenable.
