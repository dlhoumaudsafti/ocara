# `ocara.HTML` — Classe builtin

> Classe de rendu HTML avec support des composants personnalisés.  
> Toutes les méthodes sont **statiques** : elles s'appellent via `HTML::<méthode>(args)`.

---

## Import

```ocara
import ocara.HTML        // importe uniquement HTML
import ocara.*           // importe toutes les classes builtins
```

---

## Référence des méthodes

### `HTML::render(template)` → `string`

Parse un template HTML et remplace les balises custom enregistrées par leur HTML généré.

| Paramètre  | Type     | Description                                 |
|------------|----------|---------------------------------------------|
| `template` | `string` | Template HTML pouvant contenir des composants personnalisés |

```ocara
var html:string = HTML::render(`<section>
  <card title="Bienvenue" body="Ocara est un langage compilé natif.">
  <badge label="stable" color="green">
</section>`)
IO::writeln(html)
// → <section>
//     <div class="card"><h2>Bienvenue</h2><p>Ocara est un langage compilé natif.</p></div>
//     <span class="badge" style="background:green">stable</span>
//   </section>
```

**Comportement :**

- Les balises HTML5 standard (`div`, `span`, `p`, `section`, `h1`…) sont conservées telles quelles.
- Les balises correspondant à un composant enregistré via `HTMLComponent::register` sont remplacées par le HTML généré par leur handler.
- Le rendu est **récursif** : si le HTML produit par un composant contient lui-même des balises custom, elles sont également résolues.
- La profondeur de récursion est limitée à **20 niveaux** pour éviter les boucles infinies.
- Les commentaires HTML `<!-- ... -->` sont préservés sans traitement.

**Attributs passés au handler :**

Chaque attribut de la balise est parsé et transmis dans un `map<string, mixed>` :

| Syntaxe d'attribut        | Type Ocara résultant | Exemple                       |
|---------------------------|----------------------|-------------------------------|
| `attr="valeur"`           | `string`             | `title="Bienvenue"`           |
| `attr='valeur'`           | `string`             | `title='Bienvenue'`           |
| `attr=42`                 | `int`                | `count=5`                     |
| `attr=true` / `attr=false`| `bool`               | `disabled=true`               |
| `attr` (sans valeur)      | `bool` (`true`)      | `disabled`                    |
| `attr={key:val,k2:v2}`    | `map<string,string>` | `links={home:'/',about:'/about'}` |

**Slot par défaut (contenu entre balises) :**

Si un composant est utilisé avec une balise ouvrante et une balise fermante, le contenu entre les deux est pré-rendu puis transmis dans `attrs["__slot__"]` :

```ocara
HTML::render(`<panel title="Mon titre">
  <badge label="new" color="orange">
  <p>Contenu riche ici.</p>
</panel>`)
```

Le contenu entre `<panel ...>` et `</panel>` est rendu récursivement (les composants imbriqués sont résolus), puis transmis au handler de `panel` via `attrs["__slot__"]`.

**Slots nommés :**

Pour distribuer le contenu dans plusieurs zones, utiliser la balise `<slot name="...">...</slot>` à l'intérieur du corps du composant :

```ocara
HTML::render(`<page>
  <slot name="header"><h1>Titre</h1></slot>
  <slot name="content"><p>Corps.</p></slot>
  <slot name="footer"><p>© 2026</p></slot>
</page>`)
```

Chaque slot nommé est pré-rendu récursivement et transmis au handler via `attrs["__slot_<name>__"]` :

| Balise dans le template          | Clé dans `attrs`            |
|----------------------------------|-----------------------------|
| `<slot name="header">...</slot>` | `attrs["__slot_header__"]`  |
| `<slot name="content">...</slot>`| `attrs["__slot_content__"]` |
| `<slot name="footer">...</slot>` | `attrs["__slot_footer__"]`  |

Le contenu hors de tout `<slot name=...>` est transmis dans `attrs["__slot__"]` (slot par défaut).

```ocara
var page:HTMLComponent = use HTMLComponent("page")
page.register(nameless(attrs:map<string, mixed>): string {
    scoped header:string  = attrs["__slot_header__"]
    scoped content:string = attrs["__slot_content__"]
    scoped footer:string  = attrs["__slot_footer__"]
    return `<div class="page"><header>${header}</header><main>${content}</main><footer>${footer}</footer></div>`
})
```

---

### `HTML::renderCached(template, cache_key)` → `string`

Identique à `HTML::render`, mais met le résultat en cache sous la clé `cache_key`. Les appels suivants avec la même clé retournent immédiatement le résultat mis en cache sans re-parser le template.

| Paramètre   | Type     | Description                                      |
|-------------|----------|--------------------------------------------------|
| `template`  | `string` | Template HTML pouvant contenir des composants    |
| `cache_key` | `string` | Clé unique identifiant ce rendu dans le cache    |

```ocara
scoped tpl:string = `<badge label="stable" color="green">`

// Premier appel : parse et met en cache sous "badge_stable"
scoped r1:string = HTML::renderCached(tpl, "badge_stable")

// Second appel : retourne directement depuis le cache
scoped r2:string = HTML::renderCached(tpl, "badge_stable")
```

**Règles :**

- Le cache est global et persiste pour toute la durée du programme.
- ⚠️ Si deux templates **différents** utilisent la même clé, le second template ne sera jamais rendu — c'est le résultat du premier qui sera retourné. Utiliser des clés uniques par template.
- Utiliser des clés descriptives et uniques par template.
- ⚠️ Ne pas utiliser `render_cached` pour des templates dont le rendu dépend de données dynamiques qui changent entre les appels.

---

### `HTML::cacheDelete(cache_key)` → `void`

Supprime une entrée du cache par sa clé. Sans effet si la clé n'existe pas.

| Paramètre   | Type     | Description                        |
|-------------|----------|------------------------------------||
| `cache_key` | `string` | Clé du rendu à supprimer du cache  |

```ocara
HTML::renderCached(`<badge label="stable" color="green">`, "my_badge")
HTML::cacheDelete("my_badge")  // entrée supprimée
// prochain render_cached avec "my_badge" re-parsera le template
```

---

### `HTML::cacheClear()` → `void`

Purge toutes les entrées du cache de rendu.

```ocara
HTML::cacheClear()  // le cache est vide, tous les prochains render_cached re-parseront
```

---

### `HTML::escape(s)` → `string`

Échappe les caractères HTML spéciaux dans une chaîne. Utile pour prévenir les injections XSS.

| Paramètre | Type     | Description         |
|-----------|----------|---------------------|
| `s`       | `string` | Chaîne à échapper   |

| Caractère | Entité HTML |
|-----------|-------------|
| `&`       | `&amp;`     |
| `<`       | `&lt;`      |
| `>`       | `&gt;`      |
| `"`       | `&quot;`    |
| `'`       | `&#39;`     |

```ocara
scoped raw:string = "<script>alert(\"xss\")</script>"
scoped safe:string = HTML::escape(raw)
IO::writeln(safe)
// → &lt;script&gt;alert(&quot;xss&quot;)&lt;/script&gt;
```

---

## Exemple complet

```ocara
import ocara.IO
import ocara.HTML
import ocara.HTMLComponent

function main(): void {

    // Enregistrement des composants
    var card:HTMLComponent = use HTMLComponent("card")
    card.register(nameless(attrs:map<string, mixed>): string {
        scoped title:string = attrs["title"]
        scoped body:string  = attrs["body"]
        return `<div class="card"><h2>${title}</h2><p>${body}</p></div>`
    })

    var panel:HTMLComponent = use HTMLComponent("panel")
    panel.register(nameless(attrs:map<string, mixed>): string {
        scoped title:string   = attrs["title"]
        scoped content:string = attrs["__slot__"]
        return `<div class="panel"><h3>${title}</h3><div class="body">${content}</div></div>`
    })

    // Rendu simple
    var html:string = HTML::render(`<card title="Bonjour" body="Bienvenue sur Ocara.">`)
    IO::writeln(html)

    // Rendu avec slot
    var page:string = HTML::render(`<panel title="Résumé">
  <card title="Point 1" body="Premier élément.">
  <p>Texte libre entre les composants.</p>
</panel>`)
    IO::writeln(page)

    // Échappement XSS
    IO::writeln(HTML::escape("<script>alert(1)</script>"))
}
```

---

## Voir aussi

- [HTMLComponent](HTMLComponent.md) — enregistrement des composants personnalisés
- [HTTPServer](HTTPServer.md) — utilisation avec un serveur HTTP
