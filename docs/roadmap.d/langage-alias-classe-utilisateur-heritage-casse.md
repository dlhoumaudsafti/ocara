# `import Classe as Alias` (classe utilisateur) casse la résolution de méthode héritée dès qu'un AUTRE fichier référence la classe par son vrai nom

## Constat

Découvert en diagnostiquant pourquoi `examples/advanced/tauri_httpserver` ne démarrait pas réellement (le serveur écoutait, mais aucune route n'était jamais enregistrée) — **sans rapport avec les bugs déjà corrigés dans [langage-use-chaine-valeur-retour-perdue](langage-use-chaine-valeur-retour-perdue.md)**.

`main.oc` faisait :

```ocara
import configs.Server as HTTP
...
const SERVER:HTTP = use HTTP()
SERVER.routes()   // ← n'enregistre plus aucune route, silencieusement
```

Repro minimal, sans rapport avec HTTPServer, juste une méthode héritée :

```ocara
// base_ns/Base.oc
namespace base_ns
class Base {
    public method greet(): void {
        IO::writeln("greet from Base")
    }
}

// child_ns/Child.oc
namespace child_ns
import base_ns.Base
class Child extends Base {
}

// caller_ns/Caller.oc — référence Child PAR SON VRAI NOM
namespace caller_ns
import child_ns.Child
class Caller {
    init(c:Child) {
        c.greet()   // ← n'affiche RIEN quand Child est importé sous alias ailleurs
    }
}

// fichier principal — importe Child SOUS ALIAS
import child_ns.Child as MyChild
import caller_ns.Caller

init {
    const c:MyChild = use MyChild()
    use Caller(c)   // "greet from Base" ne s'affiche JAMAIS
}
```

Compile sans la moindre erreur ni avertissement — résultat silencieusement faux (méthode jamais appelée), même famille de gravité que les bugs de [langage-use-chaine-valeur-retour-perdue](langage-use-chaine-valeur-retour-perdue.md).

## Cause racine

`src/main.rs` (résolution des imports, autour de la ligne 313) : pour `import Classe as Alias`, la classe importée est **renommée** avant fusion :

```rust
if let Some(mut cls) = mod_prog.classes.iter().find(|c| c.name == requested_name).cloned() {
    cls.name = final_name.clone();   // ex. "Server" → "HTTP"
    ...
    program.classes.push(cls);
}
```

`cls.name` devient l'alias — la classe n'existe plus DU TOUT sous son nom d'origine dans `program.classes`. C'est correct et suffisant si SEUL le fichier qui pose l'alias référence cette classe. Mais si un AUTRE fichier du programme référence la même classe par son **vrai nom** (typiquement : un paramètre de méthode typé `server:Server`, jamais renommé puisqu'il vit dans un fichier distinct, jamais retouché par cet import précis) :

- `class_parents` (peuplé dans le lowering en itérant `program.classes`, `src/lower/builder.d/program.rs`) ne contient une entrée que pour le nom **final** ("HTTP" → "HTTPServer"), jamais pour le nom d'origine ("Server" → ... n'existe pas).
- Toute résolution de méthode HÉRITÉE (pas déclarée directement sur la classe) qui remonte la chaîne via `class_parents.get(cls)` — voir `src/lower/expr.d/lower.rs`, la boucle `let mut current = cls.as_str(); loop { class_parents.get(current)... }` — échoue dès que `cls` est le nom D'ORIGINE ("Server"), puisque `class_parents` ne connaît que "HTTP".
- `func_mangled` reste alors `"Server_route"` (jamais généré) au lieu de remonter vers `"HTTPServer_route"` (le vrai builtin hérité) — symbole inexistant, ignoré silencieusement par le codegen (même mécanisme que les bugs déjà corrigés dans le ticket voisin).

Le type du PARAMÈTRE (`server:Server` dans un fichier qui n'a jamais vu l'alias) n'est JAMAIS retouché par le renommage, qui n'affecte que la classe elle-même dans `program.classes` — d'où l'incohérence : deux "vues" du même objet runtime, l'une valide ("HTTP", utilisée par le fichier qui pose l'alias), l'autre orpheline ("Server", utilisée par tout le reste du programme, plus jamais résolue nulle part).

## Piste (non tranchée)

Le renommage pur et simple de `cls.name` est probablement la mauvaise approche pour une classe UTILISATEUR partagée entre plusieurs fichiers (contrairement à un builtin `ocara.*`, fabriqué à la demande sans référence croisée préexistante). Deux directions possibles, aucune évaluée en détail :

1. **Ne jamais renommer la classe** : garder `cls.name` inchangé dans `program.classes`, et à la place résoudre l'alias comme un pur synonyme LOCAL AU FICHIER IMPORTATEUR — remplacer chaque occurrence de l'alias dans l'AST de CE fichier (types, `Expr::New`, etc.) par le vrai nom avant la fusion. Plus proche de la sémantique attendue d'un alias, mais demande une passe de substitution AST ciblée.
2. **Enregistrer la classe sous les DEUX noms** (le vrai nom ET l'alias, mêmes membres) dans `program.classes`/`class_parents`/`class_field_types`. Plus simple mécaniquement, mais crée deux identités RUNTIME distinctes (`class_ids` différents) pour la même classe — casserait potentiellement `is Classe`/le dispatch polymorphique si l'une des deux instances est comparée/dispatchée dynamiquement contre l'autre nom.

## Contournement actuel

Ne pas aliaser l'import d'une classe UTILISATEUR si un AUTRE fichier du programme la référence par son vrai nom (typage de paramètre, `extends`, etc.) — utiliser le nom réel partout. Appliqué dans `examples/advanced/tauri_httpserver/main.oc` (`import configs.Server as HTTP` → `import configs.Server`).

## Priorité / Complexité

**Priorité Haute** — résultat silencieusement faux, aucun rejet à la compilation (même motif de priorisation que le ticket voisin). Non bloquant pour l'exemple `tauri_httpserver` grâce au contournement, mais reste un piège pour tout futur code qui alias une classe utilisateur partagée.
**Complexité : Structurel** — touche la fusion multi-fichiers des imports (`src/main.rs`) et potentiellement l'identité runtime des classes (`class_ids`) ; à trancher avant d'implémenter, voir les deux pistes ci-dessus.

## Fichiers clés

`src/main.rs` (résolution/fusion des imports sélectifs, ~ligne 289-327), `src/lower/builder.d/program.rs` (peuplement de `class_parents`/`class_field_types` depuis `program.classes`), `src/lower/expr.d/lower.rs` (remontée de la chaîne d'héritage pour une méthode non déclarée localement), `examples/advanced/tauri_httpserver/main.oc` (contournement appliqué), [langage-use-chaine-valeur-retour-perdue](langage-use-chaine-valeur-retour-perdue.md) (bug voisin, même symptôme de fond : symbole mangled inexistant ignoré silencieusement par le codegen).
