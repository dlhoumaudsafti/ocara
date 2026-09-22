# Builtin `ocara.UI` (GUI native unifiée : Tauri natif futur ↔ GUI native Android)

## Idée

Un builtin Ocara pour la GUI **native** (ex. `ocara.UI`), dont l'implémentation réelle change **selon la cible de compilation** :

- Compilé sans `--target` (hôte desktop) → route vers le futur support GUI natif complet de `ocara.Tauri` (voir "point d'attention" ci-dessous — **David a indiqué qu'à terme Tauri aura un support GUI complet**, au-delà de ce que le builtin `ocara.Tauri` actuel couvre).
- Compilé avec `--target aarch64-linux-android` (ou autre triple Android, voir [packaging-android](packaging-android.md)) → route vers le futur backend GUI natif Android (voir [packaging-android-gui-native](packaging-android-gui-native.md), pas encore commencé).

Distinct de [`ocara.UIHybrid`](langage-builtin-ui-hybride.md) (WebView unifiée, Tauri+WebKit ↔ WebView Android) — **scission décidée par David** plutôt qu'un unique builtin universel : deux builtins, chacun unifiant UN SEUL paradigme (natif ↔ natif ici ; web ↔ web pour `UIHybrid`), au lieu d'un seul builtin essayant de faire tenir deux paradigmes très différents dans la même API.

## Point d'attention — à clarifier avant de commencer

Tauri (le framework upstream) est architecturalement **une fenêtre native + un moteur web embarqué** — il n'existe pas, à ce jour, de mode "rendu de widgets natifs sans WebView" dans Tauri lui-même. Ce que [builtins-tauri](builtins-tauri.md) appelle "vrai support Tauri" ("finaliser l'intégration") porte sur les API OS natives AUTOUR de la fenêtre (menus, dialogues natifs, tray, notifications système — actuellement simulés en mémoire, voir `docs/builtins/Tauri.md`), pas sur un rendu non-web à l'intérieur de la fenêtre.

Deux lectures possibles de "Tauri aura un support GUI complet", à trancher avec David avant de concevoir quoi que ce soit ici :
1. "Complet" = chrome natif complet (menus/dialogues/tray/notifications réels) autour d'un contenu qui reste une WebView — dans ce cas, ce builtin `ocara.UI` (natif) et [`ocara.UIHybrid`](langage-builtin-ui-hybride.md) se recouvrent largement côté desktop (les deux finiraient par passer par Tauri), et la distinction natif/web ne se jouerait vraiment que côté Android.
2. Un rendu de widgets vraiment natif (non-web) sur desktop est envisagé, hors de Tauri lui-même ou via un mécanisme que Tauri n'offre pas aujourd'hui — dans ce cas, la partie "desktop" de ce ticket dépend d'un chantier qui n'existe nulle part encore, distinct de `ocara.Tauri`/`runtime_tauri`.

## Pourquoi ce n'est pas juste un routage mécanique

Même en supposant la lecture (1) ci-dessus, Tauri (web) et une GUI Android native restent deux paradigmes différents :

- **Tauri** : une fenêtre héberge un moteur web (WebKit/GTK sur Linux) ; l'UI est décrite en HTML/CSS/JS (ou généré, voir `ocara.HTML`/`HTMLComponent`), le code Ocara communique avec cette page via des handlers (voir `docs/builtins/Tauri.md`).
- **GUI Android native** (voie non tranchée par [packaging-android-gui-native](packaging-android-gui-native.md)) : soit un rendu bas niveau (`ANativeWindow`, dans l'esprit de SDL), soit des Views/Compose Java/Kotlin pilotées par pont JNI — dans les deux cas, un arbre de widgets impératif, pas une page web.

Concevoir UNE API `ocara.UI` qui ait un sens dans les deux mondes (quels concepts communs ? fenêtre, bouton, texte, image, événement clic — probablement un plus petit dénominateur commun assez restreint) est un vrai travail de conception, pas de la plomberie. Risque explicite à garder en tête : appauvrir l'un des deux backends pour faire rentrer les deux dans la même API.

## Ce qui est déjà en place, ce qui manque

- **En place** : `--target` (sélection de cible à la compilation, voir [packaging-android](packaging-android.md) sous-chantier 1) — le mécanisme de sélection à la compilation que ce builtin exploiterait existe déjà, pour un usage différent (codegen) mais le même principe.
- **Manque tout le reste** : aucune GUI native Android n'existe encore ([packaging-android-gui-native](packaging-android-gui-native.md), non commencé). Le "support GUI complet" de Tauri évoqué par David n'est pas non plus scopé nulle part (voir "point d'attention" ci-dessus — [builtins-tauri](builtins-tauri.md) ne couvre que le chrome natif OS, pas un rendu non-web). Aucune conception de l'API commune `ocara.UI` n'a été commencée. Aucune réflexion sur comment le compilateur choisit l'implémentation réelle d'un builtin selon `--target` (aujourd'hui, `--target` influence seulement le codegen Cranelift bas niveau, jamais la résolution des builtins eux-mêmes — un mécanisme nouveau, potentiellement dans `src/codegen/runtime.rs`/la table des builtins, partagé avec [`ocara.UIHybrid`](langage-builtin-ui-hybride.md)).

## Priorité / Complexité

**Priorité Très Basse** — spéculatif, dépend de deux prérequis non commencés et non tranchés ([packaging-android-gui-native](packaging-android-gui-native.md), et le "support GUI complet" de Tauri à clarifier avec David). **Complexité non évaluée, probablement Massive** — nouvelle abstraction de langage (résolution de builtin conditionnée par la cible de compilation, jamais fait dans Ocara), plus la conception d'une API UI commune à deux paradigmes très différents.

## Fichiers clés

Aucun — chantier pas commencé, dépend de prérequis eux-mêmes non commencés ou non clarifiés. [packaging-android-gui-native](packaging-android-gui-native.md) (prérequis direct, pas fait), [builtins-tauri](builtins-tauri.md) (portée actuelle du "vrai support Tauri", à comparer avec l'intention de David), `docs/builtins/Tauri.md` (référence du builtin desktop existant), [langage-builtin-ui-hybride](langage-builtin-ui-hybride.md) (ticket frère, WebView unifiée), `src/codegen/runtime.rs`/table des builtins (mécanisme de résolution des builtins, à étendre pour dépendre de la cible).
