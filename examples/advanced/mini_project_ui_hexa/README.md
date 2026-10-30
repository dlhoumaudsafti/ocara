# Garage Ocara — interface native

Même application que [`mini_project_hexa`](../mini_project_hexa) (flotte de
voitures, entretiens, photos, recherche, français et anglais), avec une
interface native décrite en `.oc.ui` au lieu d'un serveur web : fenêtre
Tauri sur desktop, Compose (Material 3) sur Android.

## Lancer

```sh
make desktop      # fenêtre desktop
make preview      # aperçu HTML des écrans (preview.html)
make android      # APK de debug (packaging/android)
make tests        # tests ocaraunit
```

## Architecture

Hexagonale, comme l'original (`main.oc`) : chaque contexte (`home`, `car`,
`search`) a ses couches `domain`, `app` et `infra`, et `shared` porte ce
qui est commun.

| Dossier | Rôle |
|---------|------|
| `ui/garage.oc.ui` | interface par défaut (téléphone, fenêtre étroite) ; écrans inclus depuis `ui/ui.d/` |
| `ui/garage.md.oc.ui` | variante à deux colonnes à partir de 840 px (`ui/ui.d/md/`) |
| `languages/{fr,en}/ui.yaml` | textes, passés en variables `${nom}` à `use UI` |
| `configs/App.oc` | crée l'interface et lie les actions (`configs/actions/`) |
| `context/*/app/port/*ViewPort.oc` | ports des vues : `present(dto)` rend l'état à poser sur les nœuds |
| `context/*/infra/view/` | adaptateurs : DTO → `{ id: { prop: valeur } }` |
| `context/*/infra/controllers/` | lisent les champs (`ui.get`), appellent les cas d'usage, appliquent la vue (`Screen::apply`) |
| `context/shared/infra/ui/` | `Screen` (état, messages), `NavController` (navigation, langue) |

Les contextes ne s'importent pas entre eux : la navigation de l'un vers
l'autre passe par les noms d'action, liés dans `configs/actions/`.

## Ce qui remplace le web

| Web (`mini_project_hexa`) | Natif |
|---------------------------|-------|
| routes HTTP | actions `ui.on(nom, Classe::méthode)` |
| gabarits HTML | écrans `.oc.ui`, états posés par `ui.set` / `ui.setItems` |
| formulaires POST | champs lus par `ui.get` |
| redirection + message flash | `ui.navigate` + `ui.toast("${flash_…}")` |
| upload multipart | `ui.pickFile` puis copie dans `uploads/` |
| confirmation JavaScript | `ui.confirm` |
| feuille de style responsive | variantes `garage.md.oc.ui`, `garage.lg.oc.ui` (flotte en grille, photo des voitures : chargée seulement si la variante l'affiche, `ui.has("car_photo")`) |
| langue par cookie | `ui.setVars(Language::vars(code))` |
