# `import Classe as Alias` (classe utilisateur) casse la résolution de méthode héritée dès qu'un AUTRE fichier référence la classe par son vrai nom

## Terminé — option 1 retenue et implémentée (substitution AST, jamais de renommage)

Voir §"Ce qui a été fait" plus bas.

## Constat (avant correctif)

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

## Ce qui a été fait

Des deux pistes envisagées initialement, l'option 1 (« ne jamais renommer la classe, résoudre l'alias comme un pur synonyme local au fichier importateur ») a été retenue — l'option 2 (enregistrer sous les deux noms) a été écartée après analyse : elle aurait créé deux CLASSES INDÉPENDANTES au sens du système de types (aucune relation déclarée entre elles), donc `use Alias()` produirait une valeur de type "Alias" incompatible avec un paramètre typé par le vrai nom — un rejet de compilation à l'endroit même qu'on cherche à corriger, pas juste un `is`/dispatch polymorphique dégradé comme supposé au départ.

1. **Nouveau module `src/core/alias_resolve.rs`** : `compute_aliases(imports) -> HashMap<alias, vrai_nom>` (à partir des SEULES déclarations d'import d'UN fichier — un alias n'est jamais visible en dehors du fichier qui l'écrit) et `resolve_aliases(program, aliases)`, une passe de substitution AST exhaustive qui réécrit chaque occurrence de l'alias vers le vrai nom, PARTOUT où un nom de symbole peut apparaître : `Type::Named` (récursif dans `Array`/`Map`/`Generic`/`Union`/`Function`/`Message`), `Expr::New`/`StaticCall`/`StaticConst` (jamais le nom de MÉTHODE, seulement la classe), `extends`/`implements`/`modules` d'une classe ou d'un `generic`, filtre `on e is X`, signatures d'interface, corps de fonction/méthode/constructeur récursivement (tous les `Stmt`/`Expr`).
2. **`src/main.rs`** : appelée deux fois — une fois sur le fichier PRINCIPAL (dès après son parsing, sur ses propres imports), une fois sur CHAQUE fichier importé (`mod_prog`, juste après son chargement, sur SES PROPRES imports) — AVANT toute extraction/fusion dans `program`. Les 5 lignes qui renommaient le symbole sélectionné (`cls.name = final_name.clone()` et ses 4 équivalents pour generic/interface/module/fonction) sont supprimées : le symbole GARDE désormais toujours son vrai nom dans `program`, quel que soit l'alias utilisé pour l'importer.

### Vérifications

- 11 nouveaux tests Rust unitaires (`src/core/tests.rs`) : `compute_aliases` (mapping vers le dernier segment du chemin, alias absent, alias identique au vrai nom ignoré) et `resolve_aliases` (types de paramètre non concernés laissés intacts, `extends`, `implements`, `Expr::New`, `Expr::StaticCall` — classe réécrite, méthode jamais touchée —, récursion dans les arguments imbriqués, filtre `on e is X`, no-op sur une table vide).
- Nouveau test de bout en bout multi-fichiers `examples/project/tests/AliasClassInheritanceTest.oc` (+ 3 nouvelles classes `examples/project/classes/AliasGreet{Base,Child,Caller}.oc`) : `AliasGreetChild extends AliasGreetBase`, importée sous alias dans le fichier de test, référencée par son VRAI NOM dans un troisième fichier séparé (`AliasGreetCaller`) — vérifie que `greet()` (héritée) se résout correctement malgré l'alias.
- `make tests` : 98 passed. `make regression` (cache vidé) : 684 + 50 PASS, 0 FAIL, 0 ERREUR. `make build` : 0 warning.
- `examples/advanced/tauri_httpserver/main.oc` : le contournement (alias retiré) annulé, `import configs.Server as HTTP` restauré tel que l'utilisateur le voulait à l'origine — vérifié en conditions réelles (`curl` externe) : `/health` → 200, `/` → 200, `/style.css` (fichier statique) → 200.

## Fichiers clés

`src/core/alias_resolve.rs` (nouveau), `src/core/tests.rs` (nouveau), `src/main.rs` (les deux points d'appel + suppression des 5 renommages), `examples/project/classes/AliasGreetBase.oc`, `AliasGreetChild.oc`, `AliasGreetCaller.oc` (nouveaux), `examples/project/tests/AliasClassInheritanceTest.oc` (nouveau), `examples/advanced/tauri_httpserver/main.oc` (alias restauré), [langage-use-chaine-valeur-retour-perdue](langage-use-chaine-valeur-retour-perdue.md) (bug voisin, même symptôme de fond : symbole mangled inexistant ignoré silencieusement par le codegen).
