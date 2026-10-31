# Carfolio — interface native

Suivi d'une flotte de voitures (prix d'achat, entretiens et améliorations,
valeur de revente estimée, photos), avec une interface native décrite en
`.oc.ui` : fenêtre Tauri sur desktop, Compose (Material 3) sur Android.
Né du garage de [`mini_project_hexa`](../mini_project_hexa) (même domaine,
serveur web).

## Lancer

```sh
make desktop      # fenêtre desktop
make preview      # aperçu HTML des écrans (preview.html)
make android      # APK de debug « Carfolio » (packaging/android), icône : le logo
make tests        # tests ocaraunit
make logo         # refait assets/logo.android.png depuis assets/logo.svg (ImageMagick)
```

## Ce que montre l'exemple

| Fonction | Où |
|----------|-----|
| Formulaires liés (`ui.bind`) : ajout d'une voiture, fiche, entretien, paramètres | `configs/actions/*Actions.oc` (formulaires créés et liés), `context/*/infra/view/**/Form.oc` |
| Fiche modifiable à la volée : chaque champ quitté (ou choix fait) est enregistré, prix estimé recalculé | `CarController::save`, composant `carFields` (`onBlur`/`onChange: save_car`) |
| Entretiens ajoutés, modifiés (crayon de l'historique) et supprimés | `MaintenanceController`, composants `maintenanceForm`, `historyCard` |
| Pays d'origine, année, compteur par véhicule (km/mi), dernier contrôle et prochaine échéance avec alerte à 60 jours | `cars.origin`, `cars.year`, `cars.mileage`, `cars.distance_unit`, `cars.last_inspection_date`, migration dans `Database.oc`, `InspectionSchedule` |
| Périodicité du contrôle technique : réglages manuels (2 ans / 5 ans par défaut) ou règles par pays | contexte `settings`, `InspectionRules`; les règles pays restent explicitement indisponibles tant qu'aucune source réglementaire fiable n'est intégrée |
| Contrôle technique dans l'historique, date d'intervention et plusieurs justificatifs PDF/images par opération | `MaintenanceEntity`, `MaintenanceController`, `maintenance_attachments` (migration depuis les anciennes factures) |
| Voitures de collection signalées dans les cartes ; vignette de photo principale dans les listes | composant `carCard`, modèles de lecture `CarSummaryEntity` et `CarSearchResultEntity` |
| Suppression confirmée d'une photo individuelle ; si elle était principale, une autre photo devient principale | `CarPhotoRepository::deletePhoto`, galerie et écran photo |
| Relevé du compteur et date de chaque entretien : le dernier relevé met à jour le compteur de la voiture | `maintenances.mileage`, `maintenances.performed_on`, `MileageService` |
| Recherche dans la barre de titre, propositions dans un menu sous le champ | `bar:` de `ui/carfolio.oc.ui`, `SearchController::suggest` (`suggestions`, `onSuggestion`) |
| Paramètres : langue, devise, fuseau horaire (heure d'été Europe et États-Unis), unité par défaut des nouveaux véhicules, périodes de contrôle et thème | contexte `settings`, `shared/infra/settings/Preferences.oc`, `shared/infra/i18n/{Format,Timezone}.oc` ; les compteurs restent stockés en kilomètres et ne sont convertis qu'à l'affichage/saisie |
| Menu : Accueil, Paramètres, Recherche, À propos (tiroir, toujours affiché à partir de 1200 px) | composant `navMenu`, `NavController` |
| À propos : logo, version, contenu de la base | contexte `about` |
| Thème : jaune `#FFD700` (texte noir dessus en clair, blanc en sombre), coins de 6 px | `window` de `ui/carfolio.oc.ui` (`primary`, `onPrimary`, `radius`) |
| Ajout d'une voiture dans un panneau inférieur, champs obligatoires vérifiés avant l'action | composant `newCarSheet` (`form`, `required`, `submit`, `onInvalid`) |

## Architecture

Hexagonale (`main.oc`) : chaque contexte (`home`, `car`, `search`,
`settings`, `about`) a ses couches `domain`, `app` et `infra`, et `shared`
porte ce qui est commun. Les contextes ne s'importent pas entre eux : la
navigation de l'un vers l'autre passe par les noms d'action, liés dans
`configs/actions/`.

| Dossier | Rôle |
|---------|------|
| `ui/carfolio.oc.ui` | interface par défaut (téléphone, fenêtre étroite) : fenêtre, barre de recherche, écrans inclus depuis `ui/ui.d/` |
| `ui/carfolio.md.oc.ui`, `ui/carfolio.lg.oc.ui` | variantes à partir de 840 px (deux colonnes) et 1200 px (flotte en grille avec photos, menu toujours affiché) |
| `ui/ui.d/components.oc.ui` | composants partagés (menu, champs d'une voiture, formulaires, cartes) et styles, inclus par chaque variante |
| `languages/{fr,en}/ui.yaml` | textes, passés en variables `${nom}` à `use UI` et `ui.setVars` |
| `configs/App.oc` | crée l'interface dans la langue des paramètres et lie les actions (`configs/actions/`) |
| `context/*/app/port/*ViewPort.oc` | ports des vues : `present(…)` rend l'état à poser sur les nœuds |
| `context/*/infra/view/` | adaptateurs : DTO → `{ id: { prop: valeur } }` ; formulaires liés |
| `context/*/infra/controllers/` | lisent les formulaires, appellent les cas d'usage, appliquent la vue (`Screen::apply`) |
| `context/shared/infra/` | base (`db`), préférences (`settings`), formats et fuseaux (`i18n`), `Screen` et `NavController` (`ui`) |
| `assets/` | logo (`logo.svg` ; `logo.android.png` pour Android, qui n'affiche pas le SVG) |

## Tests

`tests/` reprend l'arborescence du projet. Les dépôts SQL sont testés sur
`./app.db` sans toucher aux données existantes (voitures `TEST-OCUNIT-*`
créées puis supprimées, préférences remises en place) ; les autres tests
remplacent les dépôts par des versions en mémoire (`tests/support/fake/`)
et agissent sur une interface créée sans être affichée.
