# Système de Composants HTML pour Ocara

## 🎯 Vision

Créer un système de composants HTML personnalisés intégré nativement dans Ocara, permettant de définir des balises HTML custom qui génèrent du HTML valide à partir de fonctions nameless. C'est un concept innovant qui combine la simplicité d'écriture HTML avec la puissance de la programmation.

## 📋 API Finale

### Enregistrement d'un composant

```ocara
import ocara.HTMLComponent

var breadcrumb:HTMLComponent = use HTMLComponent("breadcrumb")
breadcrumb.register(nameless(attributes:map<string, mixed>): string {
    scoped links:map<string, string> = attributes["links"]
    scoped path:string = ""
    for key => link in links {
        path = path + `<a href="${link}">${key}</a> / `
    }
    return `<div class="bread">${path}</div>`
})
```

### Utilisation dans un template

```ocara
import ocara.HTML

var html:string = HTML::render(`<html>
    <body>
        <breadcrumb links={'home':'/', 'user': '/user'}>
    </body>
</html>`)
```

## 🏗️ Architecture d'implémentation

### 1. Classe HTMLComponent (Builtin)

**Fichier** : `src/builtins/htmlcomponent.rs`

- **Stack globale** : `HashMap<String, i64>` stocke les composants enregistrés
  - Clé : nom du composant (ex: "breadcrumb")
  - Valeur : pointeur vers la fonction nameless
- **Méthode d'instance `register`** : 
  - Signature : `register(handler: Function<string>) → void`
  - Enregistre le nameless dans la stack globale
  - Le composant devient disponible pour `HTML::render()`

### 2. Classe HTML (Builtin)

**Fichier** : `src/builtins/html.rs`

- **Méthode statique `render`** :
  - Signature : `render(template: string) → string`
  - Parse le template HTML
  - Détecte les balises custom
  - Remplace par le HTML généré
  - Retourne le HTML final

### 3. Parser HTML Custom

**Fichier** : `runtime/src/htmlparser.rs`

Le parser doit :

1. **Parcourir le template** caractère par caractère
2. **Détecter les balises** `<nom ...>`
3. **Vérifier si c'est un composant** en cherchant `nom` dans la stack globale
4. **Si composant custom** :
   - Parser les attributs
   - Évaluer les expressions Ocara dans les valeurs
   - Appeler le nameless avec les attributs
   - Remplacer la balise par le HTML généré
5. **Si balise HTML standard** : garder telle quelle
6. **Traitement récursif** : le HTML généré peut contenir d'autres composants

#### Pseudo-code du parser

```rust
pub fn parse_and_render(template: &str, components: &HashMap<String, i64>) -> String {
    let mut result = String::new();
    let mut i = 0;
    
    while i < template.len() {
        if template[i..].starts_with('<') {
            // Parse tag name
            let tag_start = i + 1;
            let tag_end = find_char(&template[tag_start..], ' ', '>');
            let tag_name = &template[tag_start..tag_start + tag_end];
            
            // Est-ce un composant custom ?
            if components.contains_key(tag_name) {
                // Parse attributes jusqu'à '>'
                let attrs_str = parse_until(&template[tag_start + tag_end..], '>');
                let attributes = parse_attributes(attrs_str);
                
                // Appeler le nameless du composant
                let func_ptr = components.get(tag_name).unwrap();
                let html = call_component(*func_ptr, attributes);
                
                // Récursif : render le HTML généré
                result.push_str(&parse_and_render(&html, components));
                
                // Avancer après '>'
                i = tag_start + tag_end + attrs_str.len() + 1;
            } else {
                // Balise HTML standard, garder tel quel
                result.push('<');
                i += 1;
            }
        } else {
            result.push(template.chars().nth(i).unwrap());
            i += 1;
        }
    }
    
    result
}
```

### 4. Parser d'attributs

**Challenge** : Parser et évaluer des expressions Ocara dans les attributs HTML

```html
<breadcrumb links={'home':'/', 'user': '/user'} class="nav" count=42>
```

Doit extraire :
- `links` = Map avec 2 entrées
- `class` = String "nav"
- `count` = Int 42

#### Évaluateur d'expressions

**Option A - Mini-évaluateur dédié** (recommandé) :
- Parser simple qui reconnaît :
  - Strings : `"text"` ou `'text'`
  - Nombres : `123`, `45.67`
  - Booleans : `true`, `false`
  - Maps : `{'key': 'value', 'k2': 42}`
  - Arrays : `[1, 2, 3]`
- Pas besoin du compilateur complet
- Suffisant pour 95% des cas d'usage

**Option B - Réutiliser le lexer/parser Ocara** :
- Plus complexe
- Plus flexible
- Overhead plus important

```rust
fn parse_attributes(attrs_str: &str) -> HashMap<String, Value> {
    let mut attributes = HashMap::new();
    let mut i = 0;
    
    while i < attrs_str.len() {
        // Skip whitespace
        while i < attrs_str.len() && attrs_str[i].is_whitespace() {
            i += 1;
        }
        
        // Parse attribute name
        let name_start = i;
        while i < attrs_str.len() && attrs_str[i] != '=' {
            i += 1;
        }
        let attr_name = &attrs_str[name_start..i].trim();
        
        i += 1; // skip '='
        
        // Parse attribute value
        let value = parse_value(&attrs_str[i..], &mut i);
        attributes.insert(attr_name.to_string(), value);
    }
    
    attributes
}

fn parse_value(s: &str, offset: &mut usize) -> Value {
    match s.chars().next() {
        Some('"') => parse_string(s, offset),
        Some('\'') => parse_string(s, offset),
        Some('{') => parse_map(s, offset),
        Some('[') => parse_array(s, offset),
        Some(c) if c.is_digit(10) => parse_number(s, offset),
        Some('t') | Some('f') => parse_bool(s, offset),
        _ => Value::Null,
    }
}

fn parse_map(s: &str, offset: &mut usize) -> Value {
    // Parse {key:value, key2:value2}
    // Retourne Value::Map
}

fn parse_array(s: &str, offset: &mut usize) -> Value {
    // Parse [val1, val2, val3]
    // Retourne Value::Array
}
```

## 📁 Structure des fichiers

```
src/builtins/
  htmlcomponent.rs   - Définition classe HTMLComponent + stack globale
  html.rs            - Classe HTML avec méthode render + parser
  mod.rs             - Ajout des exports

runtime/src/
  htmlcomponent.rs   - Implémentation register (stockage dans HashMap)
  htmlparser.rs      - Parser HTML + évaluateur d'expressions
  lib.rs             - Exports + fonctions C

docs/builtins/
  HTMLComponent.md   - Documentation builtin HTMLComponent
  HTML.md            - Documentation builtin HTML

examples/builtins/
  html.oc            - Exemple complet
```

## 💡 Exemple complet d'utilisation

### Avec HTTPServer

```ocara
import ocara.HTMLComponent
import ocara.HTML
import ocara.HTTPServer

// ── Composant Layout ─────────────────────────────────────────────────────
var layout:HTMLComponent = use HTMLComponent("layout")
layout.register(nameless(attrs:map<string, mixed>): string {
    scoped title:string = attrs["title"]
    scoped content:string = attrs["content"]
    return `<!DOCTYPE html>
<html>
    <head>
        <title>${title}</title>
        <style>
            body { font-family: Arial, sans-serif; margin: 0; padding: 20px; }
            .breadcrumb a { margin-right: 10px; }
        </style>
    </head>
    <body>${content}</body>
</html>`
})

// ── Composant Breadcrumb ─────────────────────────────────────────────────
var breadcrumb:HTMLComponent = use HTMLComponent("breadcrumb")
breadcrumb.register(nameless(attrs:map<string, mixed>): string {
    scoped links:map<string, string> = attrs["links"]
    scoped path:string = ""
    for key => link in links {
        path = path + `<a href="${link}">${key}</a> / `
    }
    return `<nav class="breadcrumb">${path}</nav>`
})

// ── Composant Card ───────────────────────────────────────────────────────
var card:HTMLComponent = use HTMLComponent("card")
card.register(nameless(attrs:map<string, mixed>): string {
    scoped title:string = attrs["title"]
    scoped body:string = attrs["body"]
    scoped color:string = attrs["color"]
    return `<div class="card" style="border-left: 4px solid ${color}; padding: 15px; margin: 10px 0;">
        <h2 style="margin-top: 0;">${title}</h2>
        <p>${body}</p>
    </div>`
})

// ── Composant UserProfile ────────────────────────────────────────────────
var userProfile:HTMLComponent = use HTMLComponent("user-profile")
userProfile.register(nameless(attrs:map<string, mixed>): string {
    scoped name:string = attrs["name"]
    scoped email:string = attrs["email"]
    scoped avatar:string = attrs["avatar"]
    return `<div class="user-profile" style="display: flex; align-items: center;">
        <img src="${avatar}" alt="${name}" style="width: 50px; height: 50px; border-radius: 50%; margin-right: 15px;">
        <div>
            <strong>${name}</strong>
            <br>
            <small>${email}</small>
        </div>
    </div>`
})

function main(): int {
    const server:HTTPServer = use HTTPServer()
    server.set_port(8080)
    
    // ── Route page d'accueil ─────────────────────────────────────────────
    server.route("/", "GET", nameless(req:int): int {
        var html:string = HTML::render(`
            <layout title="Accueil - Mon Site">
                <breadcrumb links={'Accueil':'/', 'Profil':'/profile'}>
                <h1>Bienvenue sur mon site</h1>
                <card title="Information" body="Ceci est une carte d'information" color="#3498db">
                <card title="Attention" body="Ceci est un message d'attention" color="#e74c3c">
            </layout>
        `)
        
        HTTPServer::set_resp_header(req, "Content-Type", "text/html; charset=utf-8")
        HTTPServer::respond(req, 200, html)
        return 0
    })
    
    // ── Route profil utilisateur ─────────────────────────────────────────
    server.route("/profile", "GET", nameless(req:int): int {
        var html:string = HTML::render(`
            <layout title="Mon Profil">
                <breadcrumb links={'Accueil':'/', 'Profil':'/profile'}>
                <h1>Profil Utilisateur</h1>
                <user-profile 
                    name="Alice Dupont" 
                    email="alice@example.com" 
                    avatar="https://i.pravatar.cc/150?img=1">
                <card title="Bio" body="Développeuse passionnée par Ocara" color="#2ecc71">
            </layout>
        `)
        
        HTTPServer::set_resp_header(req, "Content-Type", "text/html; charset=utf-8")
        HTTPServer::respond(req, 200, html)
        return 0
    })
    
    IO::writeln("Serveur démarré sur http://localhost:8080")
    server.run()
    return 0
}
```

## 🚀 Avantages du système

### ✅ Syntaxe naturelle
On écrit du HTML avec nos composants personnalisés, pas besoin d'apprendre une nouvelle syntaxe template.

### ✅ Réutilisabilité
Définir un composant une fois, l'utiliser partout dans l'application.

### ✅ Composition
Un composant peut utiliser d'autres composants. Le parser gère la récursivité automatiquement.

### ✅ Type-safe au runtime
Les attributs sont passés dans un `map<string, mixed>`, avec vérification des types au runtime.

### ✅ Pas de transpilation
Tout se passe au runtime lors du `HTML::render()`. Pas de build step complexe.

### ✅ Intégration HTTPServer
Parfait pour générer du HTML dynamique dans les handlers HTTP.

### ✅ Performance
Le parsing HTML est fait à la demande, pas de compilation préalable nécessaire.

## 🔮 Extensions futures

### 1. Slots pour contenu riche

Permettre de passer du contenu HTML entre les balises :

```ocara
var card:HTMLComponent = use HTMLComponent("card")
card.register(nameless(attrs:map<string, mixed>): string {
    scoped title:string = attrs["title"]
    scoped slot_content:string = attrs["__slot__"]  // contenu entre les balises
    return `<div class="card">
        <h2>${title}</h2>
        <div class="content">${slot_content}</div>
    </div>`
})

// Usage avec contenu
var html:string = HTML::render(`
    <card title="Ma Carte">
        <p>Du contenu <strong>riche</strong> ici</p>
        <breadcrumb links={'home':'/'}>
    </card>
`)
```

### 2. Slots nommés

```ocara
var page:HTMLComponent = use HTMLComponent("page")
page.register(nameless(attrs:map<string, mixed>): string {
    scoped header:string = attrs["__slot_header__"]
    scoped main:string = attrs["__slot_main__"]
    scoped footer:string = attrs["__slot_footer__"]
    return `<div class="page">
        <header>${header}</header>
        <main>${main}</main>
        <footer>${footer}</footer>
    </div>`
})

// Usage
var html:string = HTML::render(`
    <page>
        <slot name="header"><h1>Titre</h1></slot>
        <slot name="main"><p>Contenu</p></slot>
        <slot name="footer"><p>© 2026</p></slot>
    </page>
`)
```

### 3. Props conditionnelles

```ocara
var alert:HTMLComponent = use HTMLComponent("alert")
alert.register(nameless(attrs:map<string, mixed>): string {
    scoped message:string = attrs["message"]
    scoped type:string = attrs["type"]  // "info", "warning", "error"
    
    scoped color:string = ""
    if type == "info" {
        color = "#3498db"
    } else if type == "warning" {
        color = "#f39c12"
    } else {
        color = "#e74c3c"
    }
    
    return `<div class="alert" style="background: ${color}; padding: 15px; color: white;">
        ${message}
    </div>`
})

var html:string = HTML::render(`
    <alert message="Opération réussie" type="info">
    <alert message="Attention requise" type="warning">
    <alert message="Erreur critique" type="error">
`)
```

### 4. Échappement HTML

Ajouter une fonction helper pour sécuriser les inputs :

```ocara
// Dans HTML builtin
HTML::escape(input: string) → string

// Usage dans un composant
var userCard:HTMLComponent = use HTMLComponent("user-card")
userCard.register(nameless(attrs:map<string, mixed>): string {
    scoped name:string = HTML::escape(attrs["name"])  // Prévient XSS
    return `<div class="user">${name}</div>`
})
```

### 5. Cache de rendu

Pour optimiser les performances, cacher le résultat du parsing :

```ocara
HTML::render_cached(template: string, cache_key: string) → string
```

## 📊 Comparaison avec d'autres approches

| Approche | Avantages | Inconvénients |
|----------|-----------|---------------|
| **String templates** | Simple, direct | Pas de réutilisation, code dupliqué |
| **Builder pattern** | Type-safe | Verbeux, moins lisible |
| **Template engine externe** | Puissant | Dépendance externe, build step |
| **HTML Components Ocara** | ✅ Natif, ✅ Simple, ✅ Réutilisable, ✅ Composable | Parsing runtime |

## 🎯 Plan d'implémentation

### Phase 1 - Base (MVP)
1. ✅ Définir `src/builtins/htmlcomponent.rs` avec stack globale
2. ✅ Définir `src/builtins/html.rs` avec méthode `render`
3. ✅ Implémenter `runtime/src/htmlcomponent.rs` (register)
4. ✅ Implémenter parser HTML basique dans `runtime/src/htmlparser.rs`
5. ✅ Parser d'attributs simple (strings uniquement)
6. ✅ Tests basiques

### Phase 2 - Évaluateur d'expressions
1. Parser de maps : `{'key': 'value'}`
2. Parser d'arrays : `[1, 2, 3]`
3. Parser de nombres et booleans
4. Tests avec types complexes

### Phase 3 - Features avancées
1. Support des slots (`__slot__`)
2. Slots nommés
3. `HTML::escape()` pour sécurité
4. Cache optionnel

### Phase 4 - Documentation
1. `docs/builtins/HTMLComponent.md`
2. `docs/builtins/HTML.md`
3. `examples/builtins/html.oc`
4. Tests d'intégration avec HTTPServer

## 🧪 Tests à créer

```ocara
// examples/tests/htmlComponentTest.oc

import ocara.UnitTest
import ocara.HTMLComponent
import ocara.HTML

class Test_HTMLComponent {
    public static method test_simple_component() {
        var hello:HTMLComponent = use HTMLComponent("hello")
        hello.register(nameless(attrs:map<string, mixed>): string {
            scoped name:string = attrs["name"]
            return `<p>Hello ${name}!</p>`
        })
        
        var html:string = HTML::render(`<hello name="World">`)
        UnitTest::assertEqual(html, "<p>Hello World!</p>", "Simple component render")
    }
    
    public static method test_nested_components() {
        var outer:HTMLComponent = use HTMLComponent("outer")
        outer.register(nameless(attrs:map<string, mixed>): string {
            return `<div class="outer">OUTER</div>`
        })
        
        var inner:HTMLComponent = use HTMLComponent("inner")
        inner.register(nameless(attrs:map<string, mixed>): string {
            return `<span class="inner">INNER</span>`
        })
        
        var html:string = HTML::render(`<outer><inner></outer>`)
        UnitTest::assertContains(html, "OUTER", "Outer component present")
        UnitTest::assertContains(html, "INNER", "Inner component present")
    }
    
    public static method test_map_attribute() {
        var list:HTMLComponent = use HTMLComponent("list")
        list.register(nameless(attrs:map<string, mixed>): string {
            scoped items:map<string, string> = attrs["items"]
            scoped result:string = "<ul>"
            for key => val in items {
                result = result + `<li>${key}: ${val}</li>`
            }
            return result + "</ul>"
        })
        
        var html:string = HTML::render(`<list items={'a':'1', 'b':'2'}>`)
        UnitTest::assertContains(html, "<ul>", "List contains ul")
        UnitTest::assertContains(html, "a: 1", "List contains first item")
    }
}
```

## 📝 Notes d'implémentation

### Gestion de la stack globale

La stack globale des composants doit être accessible depuis :
- `HTMLComponent::register()` (pour ajouter)
- `HTML::render()` (pour lire)

**Solution** : Variable statique Rust avec `lazy_static!` ou `OnceCell`

```rust
use std::collections::HashMap;
use std::sync::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    static ref GLOBAL_COMPONENTS: Mutex<HashMap<String, i64>> = Mutex::new(HashMap::new());
}

#[no_mangle]
pub extern "C" fn HTMLComponent_register(name_ptr: i64, func_ptr: i64) {
    let name = unsafe { from_ocara_string(name_ptr) };
    let mut components = GLOBAL_COMPONENTS.lock().unwrap();
    components.insert(name, func_ptr);
}
```

### Appel d'une fonction depuis le runtime

Pour appeler le nameless depuis le parser :

```rust
// Le func_ptr est un fat pointer Ocara (16 bytes)
// Format : [context_ptr: i64, code_ptr: i64]

pub extern "C" fn call_component(func_ptr: i64, attrs_ptr: i64) -> i64 {
    // Extraire code_ptr et context_ptr du fat pointer
    let code_ptr: extern "C" fn(i64, i64) -> i64 = unsafe { std::mem::transmute(func_ptr) };
    
    // Appeler la fonction avec le contexte et les attributs
    code_ptr(0, attrs_ptr)  // 0 = pas de contexte pour nameless
}
```

### Conversion map Rust → Ocara

```rust
fn rust_map_to_ocara(map: HashMap<String, Value>) -> i64 {
    // Allouer une map Ocara
    let map_ptr = __map_new();
    
    for (key, value) in map {
        let key_ptr = to_ocara_string(&key);
        let val_ptr = value_to_ocara(value);
        __map_insert(map_ptr, key_ptr, val_ptr);
    }
    
    map_ptr
}
```

---

**Ce document sera mis à jour au fur et à mesure de l'implémentation.**
