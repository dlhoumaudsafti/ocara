// Tables du schéma `.oc.ui` (docs/roadmap.d/langage-builtin-ui-multiplateforme.md,
// §3 et §4) : types de nœuds et propriétés permises, avec le type de chaque
// valeur. Ajouter un composant = une entrée de `node_props`.

/// Type attendu d'une valeur.
#[derive(Debug, Clone, Copy)]
pub enum Ty {
    /// Texte affiché : tout scalaire, normalisé en chaîne.
    Text,
    Bool,
    /// Entier dans `[min, max]`.
    Int(i64, i64),
    /// Nombre dans `[min, max]`.
    Number(f64, f64),
    /// Longueur : nombre ≥ 0, `fill` ou `wrap`.
    Length,
    /// Marge : nombre ≥ 0, ou liste de 2 (vertical, horizontal) ou 4 nombres.
    Spacing,
    /// `#rrggbb` ou `#rrggbbaa`.
    Color,
    Enum(&'static [&'static str]),
    /// Identifiant de nœud (`id`), unique dans le fichier.
    Id,
    /// Nom d'une icône du jeu commun (icons.rs).
    Icon,
    /// Nom d'action, lié côté Ocara par `ui.on`.
    Action,
    /// Nom d'un style déclaré sous `styles:`.
    StyleRef,
    /// Champ d'un élément de liste (`bind`, gabarit `item:` seulement).
    Field,
    /// Un nœud.
    Node,
    /// Liste de nœuds.
    Children,
    /// Liste de chaînes.
    Strings,
    /// Choix de `radio`/`select` : chaînes ou `{ value, text }`.
    Options,
    /// Entrées de menu (`text`, `action`, `shortcut`, `enabled`, `checked`,
    /// `separator`, `items`).
    Menu,
    /// Table nom → action (`webHandlers`).
    ActionMap,
    /// `binds` : propriété du nœud → champ de l'élément (gabarit `item` d'une liste).
    Binds,
    /// Surcharge `tauri:`/`android:` : propriétés du même nœud.
    Override,
}

pub const ALIGN: &[&str] = &["start", "center", "end", "stretch"];
pub const THEMES: &[&str] = &["auto", "light", "dark"];
pub const PLATFORMS: &[&str] = &["tauri", "android"];

/// Propriétés de présentation, communes aux nœuds et aux `styles:`.
pub const STYLE_PROPS: &[(&str, Ty)] = &[
    ("fontSize", Ty::Number(1.0, 400.0)),
    ("bold", Ty::Bool),
    ("italic", Ty::Bool),
    ("color", Ty::Color),
    ("background", Ty::Color),
    ("padding", Ty::Spacing),
    ("margin", Ty::Spacing),
    ("radius", Ty::Number(0.0, 10_000.0)),
    ("border", Ty::Color),
    ("elevation", Ty::Number(0.0, 100.0)),
];

/// Propriétés de tout nœud (en plus de `STYLE_PROPS`).
pub const COMMON_PROPS: &[(&str, Ty)] = &[
    ("type", Ty::Text),
    ("id", Ty::Id),
    ("style", Ty::StyleRef),
    ("visible", Ty::Bool),
    ("enabled", Ty::Bool),
    ("width", Ty::Length),
    ("height", Ty::Length),
    ("weight", Ty::Number(0.0, 1000.0)),
    ("align", Ty::Enum(ALIGN)),
    ("tooltip", Ty::Text),
    ("label", Ty::Text),
    ("contextMenu", Ty::Menu),
    ("tauri", Ty::Override),
    ("android", Ty::Override),
    ("binds", Ty::Binds),
];

const CONTAINER: &[(&str, Ty)] = &[
    ("children", Ty::Children),
    ("spacing", Ty::Number(0.0, 10_000.0)),
    ("scroll", Ty::Bool),
];
const GRID: &[(&str, Ty)] = &[
    ("children", Ty::Children),
    ("spacing", Ty::Number(0.0, 10_000.0)),
    ("scroll", Ty::Bool),
    ("columns", Ty::Int(1, 64)),
];
const CARD: &[(&str, Ty)] = &[
    ("children", Ty::Children),
    ("spacing", Ty::Number(0.0, 10_000.0)),
    ("onClick", Ty::Action),
];
const NONE: &[(&str, Ty)] = &[];
const TEXT: &[(&str, Ty)] = &[("value", Ty::Text), ("bind", Ty::Field)];
const HEADING: &[(&str, Ty)] = &[("value", Ty::Text), ("bind", Ty::Field), ("level", Ty::Int(1, 3))];
const IMAGE: &[(&str, Ty)] = &[
    ("src", Ty::Text),
    ("bind", Ty::Field),
    ("fit", Ty::Enum(&["contain", "cover", "fill", "none"])),
];
const ICON: &[(&str, Ty)] = &[("name", Ty::Icon)];
const PROGRESS: &[(&str, Ty)] = &[("value", Ty::Number(0.0, 1.0))];
/// Ordre des cases d'une liste en grille : de gauche à droite (défaut), de
/// droite à gauche, ou en serpentin (chaque ligne repart du côté où la
/// précédente s'arrête).
pub const LIST_DIRECTIONS: &[&str] = &["ltr", "rtl", "ltr-snake", "rtl-snake"];
const LIST: &[(&str, Ty)] = &[
    ("item", Ty::Node),
    ("display", Ty::Enum(&["list", "grid"])),
    ("itemWidth", Ty::Number(1.0, 10_000.0)),
    ("minColumns", Ty::Int(1, 64)),
    ("maxColumns", Ty::Int(1, 64)),
    ("direction", Ty::Enum(LIST_DIRECTIONS)),
    ("spacing", Ty::Number(0.0, 10_000.0)),
    ("onSelect", Ty::Action),
];
const WEBVIEW: &[(&str, Ty)] = &[
    ("url", Ty::Text),
    ("file", Ty::Text),
    ("html", Ty::Text),
    ("javascript", Ty::Bool),
    ("storage", Ty::Bool),
    ("allow", Ty::Strings),
    ("zoom", Ty::Number(0.1, 10.0)),
    ("userAgent", Ty::Text),
    ("webHandlers", Ty::ActionMap),
    ("onLoad", Ty::Action),
    ("onError", Ty::Action),
    ("onMessage", Ty::Action),
];
const INPUT: &[(&str, Ty)] = &[
    ("value", Ty::Text),
    ("bind", Ty::Field),
    ("placeholder", Ty::Text),
    ("kind", Ty::Enum(&["text", "password", "email", "number", "multiline"])),
    ("readonly", Ty::Bool),
    ("onChange", Ty::Action),
    ("onSubmit", Ty::Action),
    ("onFocus", Ty::Action),
    ("onBlur", Ty::Action),
];
const CHECK: &[(&str, Ty)] = &[
    ("checked", Ty::Bool),
    ("bind", Ty::Field),
    ("onChange", Ty::Action),
];
const CHOICE: &[(&str, Ty)] = &[
    ("options", Ty::Options),
    ("value", Ty::Text),
    ("onChange", Ty::Action),
];
const SLIDER: &[(&str, Ty)] = &[
    ("min", Ty::Number(f64::MIN, f64::MAX)),
    ("max", Ty::Number(f64::MIN, f64::MAX)),
    ("step", Ty::Number(0.0, f64::MAX)),
    ("value", Ty::Number(f64::MIN, f64::MAX)),
    ("onChange", Ty::Action),
];
const DATE: &[(&str, Ty)] = &[("value", Ty::Text), ("placeholder", Ty::Text), ("onChange", Ty::Action)];
const BUTTON: &[(&str, Ty)] = &[
    ("text", Ty::Text),
    ("icon", Ty::Icon),
    ("variant", Ty::Enum(&["primary", "secondary", "text"])),
    ("onClick", Ty::Action),
];
const ICON_BUTTON: &[(&str, Ty)] = &[("icon", Ty::Icon), ("onClick", Ty::Action)];

// Composants Material propres à Android (préfixe `android.`), ignorés sur
// desktop : bouton flottant de l'écran, panneau inférieur et tiroir de
// navigation, ouverts par `ui.set(id, "open", true)`.
const ANDROID_FAB: &[(&str, Ty)] = &[("icon", Ty::Icon), ("text", Ty::Text), ("onClick", Ty::Action)];
const ANDROID_PANEL: &[(&str, Ty)] = &[
    ("children", Ty::Children),
    ("spacing", Ty::Number(0.0, 10_000.0)),
    ("open", Ty::Bool),
    ("onDismiss", Ty::Action),
];

/// Types de nœuds, dans l'ordre des tableaux du §3.
pub const NODE_TYPES: &[&str] = &[
    "column", "row", "stack", "grid", "card", "divider", "spacer",
    "text", "heading", "image", "icon", "progress", "list", "webview",
    "input", "checkbox", "switch", "radio", "select", "slider", "date", "time",
    "button", "iconButton",
    "android.fab", "android.bottomSheet", "android.drawer",
];

/// Propriétés propres au type `ty` ; `None` pour un type inconnu.
pub fn node_props(ty: &str) -> Option<&'static [(&'static str, Ty)]> {
    Some(match ty {
        "column" | "row" | "stack" => CONTAINER,
        "grid" => GRID,
        "card" => CARD,
        "divider" | "spacer" => NONE,
        "text" => TEXT,
        "heading" => HEADING,
        "image" => IMAGE,
        "icon" => ICON,
        "progress" => PROGRESS,
        "list" => LIST,
        "webview" => WEBVIEW,
        "input" => INPUT,
        "checkbox" | "switch" => CHECK,
        "radio" | "select" => CHOICE,
        "slider" => SLIDER,
        "date" | "time" => DATE,
        "button" => BUTTON,
        "iconButton" => ICON_BUTTON,
        "android.fab" => ANDROID_FAB,
        "android.bottomSheet" | "android.drawer" => ANDROID_PANEL,
        _ => return None,
    })
}

/// Propriétés obligatoires d'un type.
pub fn required_props(ty: &str) -> &'static [&'static str] {
    match ty {
        "image" => &["src|bind", "label"],
        "icon" => &["name"],
        "iconButton" => &["icon", "label"],
        "list" => &["item"],
        "radio" | "select" => &["options"],
        // L'hôte désigne la vue web par son `id` (ui.postMessage, ui.eval…).
        "webview" => &["id"],
        "android.fab" => &["icon", "label"],
        "android.bottomSheet" | "android.drawer" => &["id"],
        _ => &[],
    }
}

/// Type d'une propriété du nœud `ty` (propres, communes, de présentation).
pub fn prop_type(ty: &str, prop: &str) -> Option<Ty> {
    node_props(ty)?.iter()
        .chain(COMMON_PROPS)
        .chain(STYLE_PROPS)
        .find(|(name, _)| *name == prop)
        .map(|(_, t)| *t)
}

/// Clés de `window:`.
pub const WINDOW_PROPS: &[(&str, Ty)] = &[
    ("title", Ty::Text),
    ("width", Ty::Int(1, 100_000)),
    ("height", Ty::Int(1, 100_000)),
    ("minWidth", Ty::Int(1, 100_000)),
    ("minHeight", Ty::Int(1, 100_000)),
    ("resizable", Ty::Bool),
    ("theme", Ty::Enum(THEMES)),
    ("primary", Ty::Color),
    ("tauri", Ty::Override),
    ("android", Ty::Override),
];

/// Options de fenêtre propres au desktop (`window.tauri`).
pub const WINDOW_TAURI: &[(&str, Ty)] = &[
    ("decorations", Ty::Bool),
    ("alwaysOnTop", Ty::Bool),
    ("transparent", Ty::Bool),
    ("center", Ty::Bool),
    ("devtools", Ty::Bool),
];

/// Options propres à Android (`window.android`).
pub const WINDOW_ANDROID: &[(&str, Ty)] = &[
    ("orientation", Ty::Enum(&["portrait", "landscape", "auto"])),
    ("fullscreen", Ty::Bool),
    ("keepScreenOn", Ty::Bool),
    ("systemBars", Ty::Enum(&["light", "dark"])),
];

/// Clés d'un écran (`screens.<nom>`).
pub const SCREEN_PROPS: &[(&str, Ty)] = &[("title", Ty::Text), ("layout", Ty::Node), ("onShow", Ty::Action)];

/// Clés d'une entrée de menu.
pub const MENU_PROPS: &[(&str, Ty)] = &[
    ("text", Ty::Text),
    ("action", Ty::Action),
    ("shortcut", Ty::Text),
    ("enabled", Ty::Bool),
    ("checked", Ty::Bool),
    ("separator", Ty::Bool),
    ("items", Ty::Menu),
];

/// Clés de `styles:` qui ne sont pas des styles : surcharges par thème
/// (`dark: { nom: { … } }`), appliquées par-dessus le style de même nom.
pub const THEME_OVERRIDES: &[&str] = &["dark", "light"];

/// Clés à la racine du fichier.
pub const ROOT_KEYS: &[&str] = &["window", "styles", "start", "screens", "layout", "menu"];
