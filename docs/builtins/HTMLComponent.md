# `ocara.HTMLComponent` — Classe builtin

> Classe de définition de composants HTML personnalisés.  
> `HTMLComponent` est une **classe d'instance** : chaque composant est un objet créé avec `use HTMLComponent("nom")`.  
> Les composants enregistrés sont ensuite utilisés dans les templates via `HTML::render`.

---

## Import

```ocara
import ocara.HTMLComponent        // importe uniquement HTMLComponent
import ocara.*                    // importe toutes les classes builtins
```

---

## Création

### `use HTMLComponent("nom")` → `HTMLComponent`

Alloue un nouveau composant identifié par `nom`. Ce nom correspond à la balise HTML custom utilisée dans les templates.

```ocara
var card:HTMLComponent = use HTMLComponent("card")
```

Le composant est inactif tant que `register` n'a pas été appelé.

---

## Méthodes d'instance

### `c.register(handler)` → `void`

Enregistre le handler du composant. Le handler est une closure `nameless` qui reçoit les attributs de la balise et retourne une chaîne HTML.

| Paramètre | Type                        | Description                                  |
|-----------|-----------------------------|----------------------------------------------|
| `handler` | `Function<string>`          | Closure `nameless(attrs:map<string,mixed>): string` |

```ocara
var card:HTMLComponent = use HTMLComponent("card")
card.register(nameless(attrs:map<string, mixed>): string {
    scoped title:string = attrs["title"]
    scoped body:string  = attrs["body"]
    return `<div class="card"><h2>${title}</h2><p>${body}</p></div>`
})
```

**Règles :**

- Un seul handler par composant. Appeler `register` deux fois sur le même composant remplace le handler précédent.
- Le handler doit retourner un `string` contenant du HTML valide.
- Le handler peut lui-même retourner des balises custom — elles seront résolues récursivement par `HTML::render`.
- Les composants sont stockés dans un registre global : un composant enregistré dans `main` est disponible dans tous les handlers.

---

## Structure des attributs

La closure reçoit un `map<string, mixed>` dont les clés sont les noms d'attributs et les valeurs sont typées selon la syntaxe utilisée dans le template :

| Syntaxe dans le template    | Valeur dans `attrs`            |
|-----------------------------|--------------------------------|
| `attr="texte"`              | `string` — `"texte"`          |
| `attr='texte'`              | `string` — `"texte"`          |
| `attr=42`                   | `int` — `42`                  |
| `attr=3.14`                 | `string` — `"3.14"` (voir note) |
| `attr=true`                 | `bool` — `true`               |
| `attr=false`                | `bool` — `false`              |
| `attr` (booléen implicite)  | `bool` — `true`               |
| `attr={k1:v1,k2:v2}`        | `map<string,string>`          |
| `attrs["__slot__"]`         | `string` — contenu HTML pré-rendu entre les balises (voir Slots) |

> **Note floats** : les flottants dans les attributs (`attr=3.14`) sont traités comme des chaînes. Utiliser `Convert::str_to_float` si une conversion est nécessaire.

---

## Slots

### Slot par défaut (`__slot__`)

Quand un composant est utilisé avec une balise ouvrante **et** une balise fermante, le contenu entre les deux est pré-rendu (les composants imbriqués sont résolus), puis transmis au handler via `attrs["__slot__"]`.

```ocara
HTML::render(`<panel title="Mon titre">
  <badge label="new" color="orange">
  <p>Du contenu riche.</p>
</panel>`)
```

```ocara
var panel:HTMLComponent = use HTMLComponent("panel")
panel.register(nameless(attrs:map<string, mixed>): string {
    scoped title:string   = attrs["title"]
    scoped content:string = attrs["__slot__"]
    return `<div class="panel"><h3>${title}</h3><div class="body">${content}</div></div>`
})
```

Si le composant est utilisé sans balise fermante (`<panel title="...">`), `attrs["__slot__"]` n'est pas présent dans la map.

### Slots nommés (`__slot_<name>__`)

Pour distribuer le contenu dans plusieurs zones distinctes, utiliser `<slot name="...">...</slot>` dans le corps du composant :

```ocara
HTML::render(`<page>
  <slot name="header"><h1>Titre</h1></slot>
  <slot name="content"><p>Corps.</p></slot>
  <slot name="footer"><p>© 2026</p></slot>
</page>`)
```

Chaque `<slot name="xxx">...</slot>` est pré-rendu récursivement, puis transmis via `attrs["__slot_xxx__"]`. Le `name` peut être n'importe quelle chaîne.

```ocara
var page:HTMLComponent = use HTMLComponent("page")
page.register(nameless(attrs:map<string, mixed>): string {
    scoped header:string  = attrs["__slot_header__"]
    scoped content:string = attrs["__slot_content__"]
    scoped footer:string  = attrs["__slot_footer__"]
    return `<div class="page"><header>${header}</header><main>${content}</main><footer>${footer}</footer></div>`
})
```

**Règles :**
- Le `name` est libre : `header`, `sidebar`, `actions`, etc.
- Le contenu de chaque slot est rendu récursivement avant d'être transmis (les composants imbriqués sont résolus).
- Le contenu hors de tout `<slot name=...>` va dans `attrs["__slot__"]`.
- Si aucun slot nommé n'est présent, tout le contenu va dans `attrs["__slot__"]` (comportement habituel).
- Si le composant est utilisé sans balise fermante, aucun slot n'est injecté.

---

## Exemple complet

```ocara
import ocara.IO
import ocara.HTML
import ocara.HTMLComponent

function main(): void {

    // ── Composant simple ─────────────────────────────────────────────────────
    var badge:HTMLComponent = use HTMLComponent("badge")
    badge.register(nameless(attrs:map<string, mixed>): string {
        scoped label:string = attrs["label"]
        scoped color:string = attrs["color"]
        return `<span class="badge" style="background:${color}">${label}</span>`
    })

    // ── Composant avec int ───────────────────────────────────────────────────
    var rating:HTMLComponent = use HTMLComponent("rating")
    rating.register(nameless(attrs:map<string, mixed>): string {
        scoped stars:int = attrs["stars"]
        var out:string = ""
        var i:int = 0
        while i < stars {
            out = out + "★"
            i = i + 1
        }
        return `<span class="rating">${out}</span>`
    })

    // ── Composant avec slot ──────────────────────────────────────────────────
    var card:HTMLComponent = use HTMLComponent("card")
    card.register(nameless(attrs:map<string, mixed>): string {
        scoped title:string   = attrs["title"]
        scoped content:string = attrs["__slot__"]
        return `<div class="card"><h2>${title}</h2><div>${content}</div></div>`
    })

    // ── Rendu ────────────────────────────────────────────────────────────────
    var html:string = HTML::render(`<section>
  <card title="Ocara">
    <badge label="stable" color="green">
    <rating stars=4>
  </card>
</section>`)
    IO::writeln(html)
}
```

---

## Voir aussi

- [HTML](HTML.md) — méthode `render` et `escape`
- [HTTPServer](HTTPServer.md) — utilisation des composants dans un serveur HTTP
