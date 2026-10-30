// Valeurs scalaires du schéma `.oc.ui` : résolution des scalaires YAML sans
// guillemets (booléen, nombre, null) et contrôle de chaque `Ty` simple.

use serde_json::{json, Value as Json};

use super::schema::Ty;
use super::yaml::{Kind, Node};
use super::Checker;

/// Valeur d'un scalaire sans guillemets, selon le schéma « core » de YAML 1.2.
pub fn resolve_plain(text: &str) -> Json {
    match text {
        "" | "~" | "null" | "Null" | "NULL" => Json::Null,
        "true" | "True" | "TRUE" => Json::Bool(true),
        "false" | "False" | "FALSE" => Json::Bool(false),
        _ => {
            if let Ok(n) = text.parse::<i64>() {
                return json!(n);
            }
            let numeric = text.trim_start_matches(['-', '+']).starts_with(|c: char| c.is_ascii_digit() || c == '.');
            match text.parse::<f64>() {
                Ok(f) if numeric && f.is_finite() => json!(f),
                _ => Json::String(text.to_string()),
            }
        }
    }
}

/// Valeur JSON d'un scalaire (`None` pour une liste ou une table).
pub fn scalar(node: &Node) -> Option<Json> {
    match &node.kind {
        Kind::Scalar { text, plain: true } => Some(resolve_plain(text)),
        Kind::Scalar { text, plain: false } => Some(Json::String(text.clone())),
        _ => None,
    }
}

/// Texte d'un scalaire chaîne (clé de table, nom).
pub fn text(node: &Node) -> Option<&str> {
    match &node.kind {
        Kind::Scalar { text, .. } => Some(text),
        _ => None,
    }
}

pub fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

impl Checker {
    /// Contrôle `node` contre un type simple et rend sa valeur normalisée ;
    /// `what` nomme la valeur dans les messages (`propriété 'width' de 'row'`).
    pub(super) fn simple(&mut self, node: &Node, ty: Ty, what: &str) -> Option<Json> {
        let value = scalar(node);
        let number = value.as_ref().and_then(Json::as_f64).filter(|_| value.as_ref().is_some_and(Json::is_number));
        let string = value.as_ref().and_then(Json::as_str).map(str::to_string);
        match ty {
            Ty::Text => match value {
                Some(Json::String(s)) => {
                    self.scan_vars(node, &s, what)?;
                    Some(Json::String(s))
                }
                Some(Json::Null) | None => self.fail(node, format!("{what} : texte attendu")),
                Some(other) => Some(Json::String(other.to_string())),
            },
            Ty::Bool => match value {
                Some(Json::Bool(b)) => Some(Json::Bool(b)),
                _ => self.fail(node, format!("{what} : booléen attendu (true ou false)")),
            },
            Ty::Int(min, max) => match value.as_ref().and_then(Json::as_i64) {
                Some(n) if (min..=max).contains(&n) => Some(json!(n)),
                Some(n) => self.fail(node, format!("{what} : {n} hors de l'intervalle [{min}, {max}]")),
                None => self.fail(node, format!("{what} : entier attendu")),
            },
            Ty::Number(min, max) => match number {
                Some(n) if n >= min && n <= max => value,
                Some(n) => self.fail(node, format!("{what} : {n} hors de l'intervalle [{}, {}]", bound(min), bound(max))),
                None => self.fail(node, format!("{what} : nombre attendu")),
            },
            Ty::Length => match (number, string.as_deref()) {
                (Some(n), _) if n >= 0.0 => value,
                (_, Some("fill" | "wrap")) => value,
                _ => self.fail(node, format!("{what} : nombre positif, 'fill' ou 'wrap' attendu")),
            },
            Ty::Spacing => self.spacing(node, what),
            Ty::Color => match string.as_deref().and_then(super::colors::normalize) {
                Some(color) => Some(Json::String(color)),
                _ if value == Some(Json::Null) => self.fail(node, format!(
                    "{what} : couleur attendue, entre guillemets (\"#rrggbb\") : '#' commence un commentaire YAML")),
                _ => self.fail(node, format!("{what} : couleur attendue (\"#rrggbb\", \"#rrggbbaa\", \"#rgb\", \"rgb(r, g, b)\" ou \"rgba(r, g, b, a)\")")),
            },
            Ty::Enum(allowed) => match string {
                Some(s) if allowed.contains(&s.as_str()) => Some(Json::String(s)),
                Some(s) => self.fail(node, format!("{what} : valeur '{s}' inconnue, attendu : {}{}", allowed.join(", "), suggest(&s, allowed))),
                None => self.fail(node, format!("{what} : attendu : {}", allowed.join(", "))),
            },
            Ty::Icon => {
                let names = super::icons::names();
                match string {
                    Some(s) if names.contains(&s.as_str()) => {
                        self.icons.insert(s.clone());
                        Some(Json::String(s))
                    }
                    Some(s) => self.fail(node, format!("{what} : icône '{s}' inconnue, attendu : {}{}", names.join(", "), suggest(&s, &names))),
                    None => self.fail(node, format!("{what} : nom d'icône attendu ({})", names.join(", "))),
                }
            }
            // `composant.action` : action propre d'un composant (`@action`).
            Ty::Action => match string {
                Some(s) if s.split('.').all(is_ident) && s.matches('.').count() <= 1 => {
                    self.actions.insert(s.clone());
                    Some(Json::String(s))
                }
                _ => self.fail(node, format!("{what} : nom d'action attendu (lettres, chiffres, '_')")),
            },
            Ty::StyleRef => match string {
                Some(s) if self.styles.contains(&s) => Some(Json::String(s)),
                Some(s) => {
                    let known: Vec<&str> = self.styles.iter().map(String::as_str).collect();
                    self.fail(node, format!("{what} : style '{s}' non déclaré sous 'styles'{}", suggest(&s, &known)))
                }
                None => self.fail(node, format!("{what} : nom de style attendu")),
            },
            Ty::Field => match string {
                Some(s) if s.split('.').all(is_ident) => {
                    if self.in_item == 0 {
                        return self.fail(node, format!("{what} : 'bind' n'est permis que dans le gabarit 'item' d'une liste"));
                    }
                    Some(Json::String(s))
                }
                _ => self.fail(node, format!("{what} : nom de champ attendu")),
            },
            Ty::Strings => match &node.kind {
                Kind::Seq(items) => {
                    let items: Vec<Json> = items.iter().filter_map(|item| self.simple(item, Ty::Text, what)).collect();
                    Some(Json::Array(items))
                }
                _ => self.fail(node, format!("{what} : liste de textes attendue")),
            },
            _ => unreachable!("type composé contrôlé par nodes.rs"),
        }
    }

    /// Variables `${nom}` d'un texte (remplacées à l'exécution par les
    /// valeurs de `use UI(path, vars)`) ; `$${` écrit un `${` littéral.
    fn scan_vars(&mut self, node: &Node, text: &str, what: &str) -> Option<()> {
        let mut rest = text;
        while let Some(at) = rest.find("${") {
            if rest[..at].ends_with('$') {
                rest = &rest[at + 2..];
                continue;
            }
            let after = &rest[at + 2..];
            let Some(end) = after.find('}') else {
                return self.fail(node, format!("{what} : '${{' sans '}}' fermant (écrire '$${{' pour un '${{' littéral)"));
            };
            let name = &after[..end];
            if !is_ident(name) {
                return self.fail(node, format!("{what} : variable '${{{name}}}' invalide (lettres, chiffres, '_')"));
            }
            self.vars.insert(name.to_string());
            rest = &after[end + 1..];
        }
        Some(())
    }

    fn spacing(&mut self, node: &Node, what: &str) -> Option<Json> {
        let positive = |n: &Node| scalar(n).filter(|v| v.as_f64().is_some_and(|f| f >= 0.0) && v.is_number());
        match &node.kind {
            Kind::Seq(items) if items.len() == 2 || items.len() == 4 => {
                let values: Option<Vec<Json>> = items.iter().map(positive).collect();
                match values {
                    Some(values) => Some(Json::Array(values)),
                    None => self.fail(node, format!("{what} : nombres positifs attendus")),
                }
            }
            Kind::Seq(_) => self.fail(node, format!("{what} : 2 valeurs (vertical, horizontal) ou 4 (haut, droite, bas, gauche) attendues")),
            _ => match positive(node) {
                Some(value) => Some(value),
                None => self.fail(node, format!("{what} : nombre positif attendu")),
            },
        }
    }
}

fn bound(n: f64) -> String {
    if n == f64::MIN { "-∞".into() } else if n == f64::MAX { "+∞".into() } else { n.to_string() }
}

fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let current = row[j + 1];
            row[j + 1] = (previous + usize::from(ca != cb)).min(row[j] + 1).min(current + 1);
            previous = current;
        }
    }
    row[b.len()]
}

/// ` (vouliez-vous dire 'x' ?)` si un candidat est proche de `word`.
pub fn suggest(word: &str, candidates: &[&str]) -> String {
    let lower = word.to_lowercase();
    candidates.iter()
        .map(|c| (distance(&lower, &c.to_lowercase()), *c))
        .filter(|(d, c)| *d <= 2.max(c.len() / 3))
        .min()
        .map(|(_, c)| format!(" (vouliez-vous dire '{c}' ?)"))
        .unwrap_or_default()
}
