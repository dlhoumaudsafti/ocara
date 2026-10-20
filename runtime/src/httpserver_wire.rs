// ─────────────────────────────────────────────────────────────────────────────
// ocara.HTTPServer — format sur le fil : décodage URL, en-têtes
//
// Fonctions pures utilisées par `httpserver.rs` (voir
// docs/roadmap.d/stdlib-httpserver-robustesse.md) :
//   - `percent_decode` / `form_decode` : `%XX` décodés en OCTETS puis
//     interprétés en UTF-8 (`%C3%A9` → `é`, jamais `Ã©`) ; `+` n'est un espace
//     que dans une query string ou un corps urlencoded, jamais dans un chemin ;
//   - `is_valid_header_name` / `is_valid_header_value` : refusent CR, LF et
//     les caractères de contrôle (injection d'en-têtes par `respondHeader`) ;
//   - `insert_request_header` : un en-tête répété est fusionné, jamais écrasé.
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;

/// Décode les séquences `%XX` (octets) puis interprète le résultat en UTF-8,
/// avec remplacement des séquences invalides. Un `%` non suivi de deux
/// chiffres hexadécimaux reste tel quel.
pub(crate) fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let (Some(h), Some(l)) = (hex_value(bytes.get(i + 1)), hex_value(bytes.get(i + 2))) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_value(byte: Option<&u8>) -> Option<u8> {
    match byte? {
        b @ b'0'..=b'9' => Some(b - b'0'),
        b @ b'a'..=b'f' => Some(b - b'a' + 10),
        b @ b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Décodage `application/x-www-form-urlencoded` (query string, corps de
/// formulaire) : `+` vaut un espace, puis `percent_decode`.
pub(crate) fn form_decode(s: &str) -> String {
    percent_decode(&s.replace('+', " "))
}

pub(crate) fn parse_query(raw: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in raw.split('&') {
        let (key, val) = pair.split_once('=').unwrap_or((pair, ""));
        if !key.is_empty() {
            map.insert(form_decode(key), form_decode(val));
        }
    }
    map
}

/// Chemin et query string d'une cible de requête, fragment `#…` retiré
/// (un client ne doit pas l'envoyer, mais il ne doit jamais finir dans la
/// query).
pub(crate) fn split_target(url: &str) -> (&str, &str) {
    let without_fragment = url.split_once('#').map_or(url, |(before, _)| before);
    without_fragment.split_once('?').unwrap_or((without_fragment, ""))
}

/// Nom d'en-tête : un `token` HTTP (RFC 9110 §5.6.2).
pub(crate) fn is_valid_header_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}

/// Valeur d'en-tête : texte visible, espaces et tabulations ; jamais CR, LF
/// ni autre caractère de contrôle (sinon un en-tête ou un cookie peut être
/// injecté dans la réponse).
pub(crate) fn is_valid_header_value(value: &str) -> bool {
    value.bytes().all(|b| b == b'\t' || (b >= 0x20 && b != 0x7f))
}

/// Range un en-tête de requête : un nom déjà présent (casse ignorée) voit
/// ses valeurs fusionnées (`; ` pour `Cookie`, `, ` sinon — RFC 9110 §5.3).
pub(crate) fn insert_request_header(headers: &mut HashMap<String, String>, name: &str, value: &str) {
    match headers.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(name)) {
        Some((_, existing)) => {
            let separator = if name.eq_ignore_ascii_case("Cookie") { "; " } else { ", " };
            existing.push_str(separator);
            existing.push_str(value);
        }
        None => {
            headers.insert(name.to_string(), value.to_string());
        }
    }
}
