# Un appel de méthode chaîné sur le résultat `void` d'un appel précédent compile sans erreur, et le reste de la chaîne est silencieusement ignoré

## Constat

Découvert dans `examples/advanced/tauri_httpserver/configs/Server.oc`, en corrigeant les bugs de [langage-use-chaine-valeur-retour-perdue](langage-use-chaine-valeur-retour-perdue.md) :

```ocara
self.port(8080)
    .workers(4)
    .rootPath("./public/")
```

`HTTPServer::port`/`workers`/`rootPath` retournent tous `void` (`docs/builtins/HTTPServer.md`) — ce ne sont PAS des méthodes fluides (aucune ne retourne `self`). Ce code **compile sans la moindre erreur ni avertissement**, mais `workers(4)` et `rootPath(...)` ne sont JAMAIS appelés — confirmé en conditions réelles : le serveur démarre avec 4 workers par défaut (pas 4 volontaires, coïncidence de valeur par défaut) et **aucun `root_path` configuré** (`"[STATIC] No root_path configured"` dans les logs runtime), donc plus aucun fichier statique n'est jamais servi.

Repro minimal :

```ocara
import ocara.HTTPServer

class Server extends HTTPServer {
    init() {
        parent::init()
        self.port(8080)
            .workers(4)          // jamais exécuté
            .rootPath("./x/")    // jamais exécuté non plus
    }
}
```

Compile proprement. Séparer les trois appels (`self.port(8080)` puis `self.workers(4)` puis `self.rootPath(...)`, chacun sur sa propre ligne) fonctionne correctement — c'est le contournement, déjà appliqué dans `Server.oc`.

## Cause probable (non creusée en détail)

Cohérent avec la famille de bugs de [langage-use-chaine-valeur-retour-perdue](langage-use-chaine-valeur-retour-perdue.md) : la résolution de classe du récepteur, pour `Expr::Call{callee: Expr::Field{object: Expr::Call{...port...}, field:"workers"}}`, consulte `builder.fn_ret_types.get("HTTPServer_port")` et exige `IrType::Ptr` pour continuer avec la même classe (voir `src/lower/expr.d/lower.rs`, bloc `Expr::Call { callee: inner_callee, .. }`, cas `Expr::Field`) — `port` retourne `IrType::Void`, donc `class_name` devient `None`, `func_mangled` vaut `"_method_workers"` (symbole inexistant), ignoré silencieusement par le codegen (`emit_calls`).

Mais ici, contrairement aux cas déjà corrigés, il n'y a **aucune classe valide à retrouver** : chaîner un appel sur un `void` n'a de toute façon aucun sens sémantique. Le vrai manque n'est donc pas une résolution de classe à ajouter, mais une **vérification absente en sema** : un appel de méthode dont le récepteur est lui-même un appel à une méthode `void` devrait être rejeté à la compilation (« cannot call a method on 'void' »), pas silencieusement toléré puis no-opé au lowering.

## Priorité / Complexité

**Priorité Haute** — même motif que les tickets voisins : résultat silencieusement faux (ici, des appels entiers disparaissent), aucun rejet à la compilation.
**Complexité : non évaluée** — le correctif est probablement côté sema (rejeter un appel de méthode sur un récepteur de type `void`), pas encore localisé précisément où le sema type-check actuellement une telle chaîne sans la rejeter.

## Fichiers clés

`examples/advanced/tauri_httpserver/configs/Server.oc` (contournement appliqué : trois appels séparés), `src/lower/expr.d/lower.rs` (mécanisme de résolution de classe du récepteur, même famille que les tickets voisins), sema (typecheck d'un `Expr::Call` chaîné — point d'entrée non encore identifié précisément), [langage-use-chaine-valeur-retour-perdue](langage-use-chaine-valeur-retour-perdue.md) (bugs voisins, même symptôme de fond).
