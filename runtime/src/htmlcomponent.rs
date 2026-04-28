// ─────────────────────────────────────────────────────────────────────────────
// ocara.HTMLComponent + ocara.HTML — système de composants HTML
//
// Fonctions exportées :
//
//   HTMLComponent_init(self_ptr)              → void  constructeur
//   HTMLComponent_register(self_ptr, fat_ptr) → void  enregistre le handler
//   HTML_render(template_ptr)                 → i64   rend le template HTML
//   HTML_escape(s_ptr)                        → i64   échappe les caractères HTML
//
// Modèle :
//   Un HTMLComponent stocke son nom et un handler Ocara
//   (closure nameless(attrs: map<string, mixed>): string).
//   HTML_render() scanne le template, détecte les balises custom, construit
//   un map<string, mixed> d'attributs et appelle le handler correspondant.
//
// Format fat pointer :
//   { func_ptr: i64, env_ptr: i64 }  (16 octets)
//   Appel : let f: fn(env_ptr: i64, attrs_ptr: i64) -> i64 = transmute(func_ptr)
//           f(env_ptr, attrs_ptr)
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use std::sync::Mutex;

use crate::{alloc_str, ptr_to_str, new_map, __map_set};

// ─────────────────────────────────────────────────────────────────────────────
// Registre global des composants
// ─────────────────────────────────────────────────────────────────────────────

struct ComponentEntry {
    func_ptr: i64,
    env_ptr:  i64,
}
unsafe impl Send for ComponentEntry {}

static REGISTRY: Mutex<Option<HashMap<String, ComponentEntry>>> = Mutex::new(None);

fn with_registry<F, R>(f: F) -> R
where F: FnOnce(&mut HashMap<String, ComponentEntry>) -> R {
    let mut guard = REGISTRY.lock().unwrap();
    if guard.is_none() {
        *guard = Some(HashMap::new());
    }
    f(guard.as_mut().unwrap())
}

// ─────────────────────────────────────────────────────────────────────────────
// Struct interne de l'instance HTMLComponent
// ─────────────────────────────────────────────────────────────────────────────

struct OcaraHtmlComponent {
    name: String,
}

#[inline]
unsafe fn component_from_slot(self_ptr: i64) -> &'static mut OcaraHtmlComponent {
    let slot  = self_ptr as *const i64;
    let inner = *slot as *mut OcaraHtmlComponent;
    &mut *inner
}

// ─────────────────────────────────────────────────────────────────────────────
// API publique — HTMLComponent
// ─────────────────────────────────────────────────────────────────────────────

/// Constructeur : lit le nom passé en dernier argument (convention init).
/// En Ocara : `use HTMLComponent("breadcrumb")` → self_ptr reçoit le slot,
/// le constructeur doit lire son propre paramètre depuis la pile d'appels.
/// Convention : self_ptr est le slot alloué, name_ptr est passé après.
#[no_mangle]
pub extern "C" fn HTMLComponent_init(self_ptr: i64, name_ptr: i64) {
    let name = unsafe { ptr_to_str(name_ptr).to_string() };
    let c    = Box::new(OcaraHtmlComponent { name });
    let raw  = Box::into_raw(c) as i64;
    unsafe { *(self_ptr as *mut i64) = raw; }
}

/// Enregistre un handler nameless(attrs: map<string, mixed>): string.
/// fat_ptr = { func_ptr: i64, env_ptr: i64 }.
#[no_mangle]
pub extern "C" fn HTMLComponent_register(self_ptr: i64, fat_ptr: i64) {
    let comp     = unsafe { component_from_slot(self_ptr) };
    let func_ptr = unsafe { *(fat_ptr as *const i64) };
    let env_ptr  = unsafe { *((fat_ptr as *const i64).add(1)) };
    with_registry(|reg| {
        reg.insert(comp.name.clone(), ComponentEntry { func_ptr, env_ptr });
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Parser d'attributs HTML
// ─────────────────────────────────────────────────────────────────────────────

/// Valeur d'un attribut parsé (type dynamique simplifié).
enum AttrVal {
    Str(String),
    Int(i64),
    Bool(bool),
    Map(Vec<(String, String)>),  // paires (key, value) — toutes strings pour l'instant
}

/// Convertit une AttrVal en i64 Ocara.
unsafe fn attr_to_ocara(val: AttrVal) -> i64 {
    match val {
        AttrVal::Str(s)  => alloc_str(&s),
        AttrVal::Int(n)  => n,
        AttrVal::Bool(b) => if b { 1 } else { 0 },
        AttrVal::Map(entries) => {
            let m = new_map();
            for (k, v) in entries {
                let kp = alloc_str(&k);
                let vp = alloc_str(&v);
                __map_set(m, kp, vp);
            }
            m
        }
    }
}

/// Parse la valeur d'un attribut en consommant depuis la position courante.
fn parse_attr_value(s: &[u8], i: &mut usize) -> AttrVal {
    skip_ws(s, i);
    if *i >= s.len() { return AttrVal::Str(String::new()); }

    match s[*i] {
        // Chaîne entre guillemets simples ou doubles
        b'\'' | b'"' => {
            let q = s[*i];
            *i += 1;
            let start = *i;
            while *i < s.len() && s[*i] != q {
                *i += 1;
            }
            let v = String::from_utf8_lossy(&s[start..*i]).into_owned();
            if *i < s.len() { *i += 1; } // consommer le guillemet fermant
            AttrVal::Str(v)
        }
        // Map : {key:value, key2:value2} ou {'key':'value'}
        b'{' => {
            *i += 1;
            let mut entries: Vec<(String, String)> = Vec::new();
            loop {
                skip_ws(s, i);
                if *i >= s.len() || s[*i] == b'}' { *i += 1; break; }
                // Clé
                let key = parse_bare_string(s, i, &[b':', b'}', b',']);
                skip_ws(s, i);
                if *i < s.len() && s[*i] == b':' { *i += 1; }
                skip_ws(s, i);
                // Valeur
                let val_str = match parse_attr_value(s, i) {
                    AttrVal::Str(v) => v,
                    AttrVal::Int(n) => n.to_string(),
                    AttrVal::Bool(b) => b.to_string(),
                    AttrVal::Map(_) => String::new(),
                };
                entries.push((key.trim().to_string(), val_str));
                skip_ws(s, i);
                if *i < s.len() && s[*i] == b',' { *i += 1; }
            }
            AttrVal::Map(entries)
        }
        // Nombre
        c if c.is_ascii_digit() || c == b'-' => {
            let start = *i;
            if s[*i] == b'-' { *i += 1; }
            while *i < s.len() && s[*i].is_ascii_digit() { *i += 1; }
            let tok = String::from_utf8_lossy(&s[start..*i]).into_owned();
            AttrVal::Int(tok.parse::<i64>().unwrap_or(0))
        }
        // true / false
        _ => {
            let start = *i;
            while *i < s.len() && s[*i].is_ascii_alphanumeric() { *i += 1; }
            let tok = String::from_utf8_lossy(&s[start..*i]).into_owned();
            match tok.as_str() {
                "true"  => AttrVal::Bool(true),
                "false" => AttrVal::Bool(false),
                _       => AttrVal::Str(tok),
            }
        }
    }
}

fn skip_ws(s: &[u8], i: &mut usize) {
    while *i < s.len() && (s[*i] == b' ' || s[*i] == b'\t' || s[*i] == b'\n' || s[*i] == b'\r') {
        *i += 1;
    }
}

fn parse_bare_string(s: &[u8], i: &mut usize, stops: &[u8]) -> String {
    // Accepte guillemets simples/doubles comme délimiteurs
    if *i < s.len() && (s[*i] == b'\'' || s[*i] == b'"') {
        let q = s[*i]; *i += 1;
        let start = *i;
        while *i < s.len() && s[*i] != q { *i += 1; }
        let v = String::from_utf8_lossy(&s[start..*i]).into_owned();
        if *i < s.len() { *i += 1; }
        return v;
    }
    let start = *i;
    while *i < s.len() && !stops.contains(&s[*i]) { *i += 1; }
    String::from_utf8_lossy(&s[start..*i]).into_owned()
}

/// Parse tous les attributs d'une balise et retourne un map Ocara.
/// Entrée : slice qui commence après le nom de la balise, jusqu'à '>'.
unsafe fn parse_attrs_to_ocara_map(attrs_bytes: &[u8]) -> i64 {
    let map = new_map();
    let mut i = 0;
    loop {
        skip_ws(attrs_bytes, &mut i);
        if i >= attrs_bytes.len() { break; }
        // Lire le nom de l'attribut
        let name = parse_bare_string(attrs_bytes, &mut i, &[b'=', b'>', b' ', b'\t', b'\n', b'\r']);
        let name = name.trim().to_string();
        if name.is_empty() { break; }
        skip_ws(attrs_bytes, &mut i);
        let val: AttrVal = if i < attrs_bytes.len() && attrs_bytes[i] == b'=' {
            i += 1;
            parse_attr_value(attrs_bytes, &mut i)
        } else {
            AttrVal::Bool(true) // attribut booléen sans valeur
        };
        let key_ptr = alloc_str(&name);
        let val_ptr = attr_to_ocara(val);
        __map_set(map, key_ptr, val_ptr);
    }
    map
}

// ─────────────────────────────────────────────────────────────────────────────
// Parser HTML / moteur de rendu
// ─────────────────────────────────────────────────────────────────────────────

/// Appelle un handler enregistré avec les attributs.
/// Signature du handler Ocara : fn(env_ptr: i64, attrs_ptr: i64) -> i64
unsafe fn call_component(entry: &ComponentEntry, attrs_ptr: i64) -> String {
    type HandlerFn = unsafe extern "C" fn(i64, i64) -> i64;
    let f: HandlerFn = std::mem::transmute(entry.func_ptr as usize);
    let result_ptr = f(entry.env_ptr, attrs_ptr);
    if result_ptr == 0 {
        String::new()
    } else {
        ptr_to_str(result_ptr).to_string()
    }
}

/// Vérifie si un nom de balise est un nom de composant personnalisé valide.
/// Les noms valides contiennent uniquement des lettres, chiffres et tirets,
/// et ne sont pas des balises HTML5 standard (heuristique simple).
fn is_html5_standard(tag: &str) -> bool {
    const STANDARD: &[&str] = &[
        "a","abbr","address","area","article","aside","audio",
        "b","base","bdi","bdo","blockquote","body","br","button",
        "canvas","caption","cite","code","col","colgroup",
        "data","datalist","dd","del","details","dfn","dialog","div","dl","dt",
        "em","embed",
        "fieldset","figcaption","figure","footer","form",
        "h1","h2","h3","h4","h5","h6","head","header","hgroup","hr","html",
        "i","iframe","img","input","ins",
        "kbd",
        "label","legend","li","link",
        "main","map","mark","menu","meta","meter",
        "nav","noscript",
        "object","ol","optgroup","option","output",
        "p","picture","pre","progress",
        "q",
        "rp","rt","ruby",
        "s","samp","script","search","section","select","slot","small",
        "source","span","strong","style","sub","summary","sup",
        "table","tbody","td","template","textarea","tfoot","th","thead",
        "time","title","tr","track",
        "u","ul",
        "var","video",
        "wbr",
        // SVG courants
        "svg","path","circle","rect","line","polyline","polygon","g","text","defs","use",
        // Doctype / spéciaux
        "!doctype","!--",
    ];
    let lower = tag.to_lowercase();
    STANDARD.contains(&lower.as_str())
}

/// Extrait la valeur de l'attribut `name` d'une chaîne d'attributs HTML.
/// Entrée ex. : ` name="header" class="x"` → Some("header")
fn extract_name_attr(attrs: &str) -> Option<String> {
    let bytes = attrs.as_bytes();
    let mut i = 0;
    loop {
        // Sauter les espaces
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r') { i += 1; }
        if i >= bytes.len() { break; }
        // Lire le nom de l'attribut
        let name_start = i;
        while i < bytes.len() && !matches!(bytes[i], b'=' | b' ' | b'\t' | b'\n' | b'\r' | b'>') { i += 1; }
        let attr_name = &attrs[name_start..i];
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\t') { i += 1; }
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
            while i < bytes.len() && matches!(bytes[i], b' ' | b'\t') { i += 1; }
            let val = if i < bytes.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
                let q = bytes[i]; i += 1;
                let vstart = i;
                while i < bytes.len() && bytes[i] != q { i += 1; }
                let v = attrs[vstart..i].to_string();
                if i < bytes.len() { i += 1; }
                v
            } else {
                let vstart = i;
                while i < bytes.len() && !matches!(bytes[i], b' ' | b'\t' | b'>' | b'\n' | b'\r') { i += 1; }
                attrs[vstart..i].to_string()
            };
            if attr_name == "name" { return Some(val); }
        } else {
            if i >= bytes.len() { break; }
            i += 1; // attribut booléen sans valeur
        }
    }
    None
}

/// Extrait les slots nommés `<slot name="...">...</slot>` du contenu brut.
/// Retourne (contenu_défaut, vec[(nom, contenu_brut)])
/// Tout ce qui n'est pas dans un `<slot name=...>` va dans le contenu par défaut.
fn extract_named_slots(content: &str) -> (String, Vec<(String, String)>) {
    let mut default_content = String::new();
    let mut named_slots: Vec<(String, String)> = Vec::new();
    let mut pos = 0;

    while pos < content.len() {
        match content[pos..].find("<slot") {
            None => {
                default_content.push_str(&content[pos..]);
                break;
            }
            Some(rel) => {
                let slot_pos = pos + rel;
                // Vérifier que c'est bien <slot suivi d'un espace/> (pas <slotmachine>)
                let after = &content[slot_pos + 5..]; // après "<slot"
                let is_slot_tag = after.chars().next()
                    .map(|c| c.is_ascii_whitespace() || c == '>')
                    .unwrap_or(true);

                if !is_slot_tag {
                    // Faux positif : copier jusqu'au '<' inclus et continuer
                    default_content.push_str(&content[pos..slot_pos + 1]);
                    pos = slot_pos + 1;
                    continue;
                }

                // Tout ce qui précède va au slot par défaut
                default_content.push_str(&content[pos..slot_pos]);

                // Trouver la fin du tag ouvrant '>'
                match content[slot_pos..].find('>') {
                    None => {
                        default_content.push('<');
                        pos = slot_pos + 1;
                    }
                    Some(rel_close) => {
                        let tag_end = slot_pos + rel_close; // position de '>'
                        let attrs_str = content[slot_pos + 5..tag_end].trim();
                        let slot_name = extract_name_attr(attrs_str);
                        let after_open = tag_end + 1;

                        const CLOSE: &str = "</slot>";
                        match content[after_open..].find(CLOSE) {
                            None => {
                                // Pas de </slot> : traiter comme du contenu normal
                                default_content.push_str(&content[slot_pos..after_open]);
                                pos = after_open;
                            }
                            Some(rel_end) => {
                                let slot_content = &content[after_open..after_open + rel_end];
                                if let Some(name) = slot_name {
                                    named_slots.push((name, slot_content.to_string()));
                                } else {
                                    default_content.push_str(slot_content);
                                }
                                pos = after_open + rel_end + CLOSE.len();
                            }
                        }
                    }
                }
            }
        }
    }

    (default_content, named_slots)
}

/// Rend récursivement un template HTML en remplaçant les composants custom.
/// Profondeur max pour éviter les boucles infinies.
fn render_recursive(template: &str, depth: u32) -> String {
    if depth > 20 { return template.to_string(); }
    let bytes = template.as_bytes();
    let mut result = String::with_capacity(template.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'<' {
            // Accumule les bytes non-balise en un seul slice UTF-8
            // pour préserver les caractères multi-octets (ex. é, ★, 中).
            let start = i;
            while i < bytes.len() && bytes[i] != b'<' {
                i += 1;
            }
            result.push_str(std::str::from_utf8(&bytes[start..i]).unwrap_or(
                &String::from_utf8_lossy(&bytes[start..i])
            ));
            continue;
        }

        // Début d'une balise
        let tag_start = i + 1;

        // Ignorer commentaires <!-- ... -->
        if bytes[tag_start..].starts_with(b"!--") {
            let end = template[tag_start..].find("-->").map(|p| tag_start + p + 3).unwrap_or(bytes.len());
            result.push_str(&template[i..end]);
            i = end;
            continue;
        }

        // Lire le nom de la balise
        let mut name_end = tag_start;
        while name_end < bytes.len() && bytes[name_end] != b' ' && bytes[name_end] != b'>'
            && bytes[name_end] != b'\n' && bytes[name_end] != b'\t' && bytes[name_end] != b'\r'
            && bytes[name_end] != b'/' {
            name_end += 1;
        }
        let tag_name = &template[tag_start..name_end];

        // Fermer '<' si tag_name vide ou balise de fermeture
        if tag_name.is_empty() || tag_name.starts_with('/') {
            result.push('<');
            i += 1;
            continue;
        }

        // Vérifier si c'est un composant enregistré
        let entry_data: Option<(i64, i64)> = with_registry(|reg| {
            reg.get(tag_name).map(|e| (e.func_ptr, e.env_ptr))
        });

        if entry_data.is_none() || is_html5_standard(tag_name) {
            // Balise standard : émettre telle quelle
            result.push('<');
            i += 1;
            continue;
        }

        let (func_ptr, env_ptr) = entry_data.unwrap();

        // Trouver la fin de la balise ouvrante (chercher '>')
        // en tenant compte des chaînes imbriquées
        let mut j = name_end;
        let mut in_q: Option<u8> = None;
        while j < bytes.len() {
            match (in_q, bytes[j]) {
                (None, b'"') | (None, b'\'') => { in_q = Some(bytes[j]); j += 1; }
                (Some(q), c) if c == q       => { in_q = None; j += 1; }
                (None, b'>')                  => break,
                _                             => { j += 1; }
            }
        }
        let attrs_bytes = &bytes[name_end..j];
        // Avancer après '>'
        let close_pos = if j < bytes.len() { j + 1 } else { j };

        // Construire le map d'attributs et appeler le handler
        let attrs_str = std::str::from_utf8(attrs_bytes).unwrap_or("").trim().to_string();

        // Chercher une balise fermante </tag_name> pour les slots
        let closing_tag = format!("</{}>", tag_name);
        let (slot_data, end_pos) = if let Some(rel) = template[close_pos..].find(&closing_tag) {
            let raw = &template[close_pos..close_pos + rel];
            // Extraire les slots nommés du contenu brut
            let (default_raw, named) = extract_named_slots(raw);
            // Rendu récursif du contenu de chaque slot
            let rendered_default = render_recursive(&default_raw, depth + 1);
            let rendered_named: Vec<(String, String)> = named.into_iter()
                .map(|(name, content)| (name, render_recursive(&content, depth + 1)))
                .collect();
            (Some((rendered_default, rendered_named)), close_pos + rel + closing_tag.len())
        } else {
            (None, close_pos)
        };

        let html_out = unsafe {
            let attrs_ptr = parse_attrs_to_ocara_map(attrs_str.as_bytes());
            if let Some((ref default_slot, ref named_slots)) = slot_data {
                // Slot par défaut : contenu hors <slot name=...>
                let k = alloc_str("__slot__");
                let v = alloc_str(default_slot);
                __map_set(attrs_ptr, k, v);
                // Slots nommés : attrs["__slot_<name>__"]
                for (name, content) in named_slots {
                    let k = alloc_str(&format!("__slot_{}__", name));
                    let v = alloc_str(content);
                    __map_set(attrs_ptr, k, v);
                }
            }
            let entry = ComponentEntry { func_ptr, env_ptr };
            call_component(&entry, attrs_ptr)
        };

        // Rendu récursif du HTML produit
        result.push_str(&render_recursive(&html_out, depth + 1));
        i = end_pos;
    }

    result
}

// ─────────────────────────────────────────────────────────────────────────────
// Cache de rendu
// ─────────────────────────────────────────────────────────────────────────────

static RENDER_CACHE: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

fn with_cache<F, R>(f: F) -> R
where F: FnOnce(&mut HashMap<String, String>) -> R {
    let mut guard = RENDER_CACHE.lock().unwrap();
    if guard.is_none() {
        *guard = Some(HashMap::new());
    }
    f(guard.as_mut().unwrap())
}

// ─────────────────────────────────────────────────────────────────────────────
// API publique — HTML
// ─────────────────────────────────────────────────────────────────────────────

/// Rend un template HTML en remplaçant les balises custom par leur HTML généré.
#[no_mangle]
pub extern "C" fn HTML_render(template_ptr: i64) -> i64 {
    let template = unsafe { ptr_to_str(template_ptr) };
    let rendered = render_recursive(template, 0);
    unsafe { alloc_str(&rendered) }
}

/// Rend un template HTML avec mise en cache par cache_key.
/// Si cache_key a déjà été rendu, retourne le résultat mis en cache.
/// Sinon, rend le template, le stocke dans le cache et le retourne.
#[no_mangle]
pub extern "C" fn HTML_render_cached(template_ptr: i64, cache_key_ptr: i64) -> i64 {
    let cache_key = unsafe { ptr_to_str(cache_key_ptr).to_string() };
    // Vérifier le cache
    let cached = with_cache(|c| c.get(&cache_key).cloned());
    if let Some(hit) = cached {
        return unsafe { alloc_str(&hit) };
    }
    // Cache miss : rendre et stocker
    let template = unsafe { ptr_to_str(template_ptr) };
    let rendered = render_recursive(template, 0);
    with_cache(|c| { c.insert(cache_key, rendered.clone()); });
    unsafe { alloc_str(&rendered) }
}

/// Supprime une entrée du cache par sa clé.
/// Sans effet si la clé n'existe pas.
#[no_mangle]
pub extern "C" fn HTML_cache_delete(cache_key_ptr: i64) {
    let cache_key = unsafe { ptr_to_str(cache_key_ptr).to_string() };
    with_cache(|c| { c.remove(&cache_key); });
}

/// Purge toutes les entrées du cache de rendu.
#[no_mangle]
pub extern "C" fn HTML_cache_clear() {
    with_cache(|c| { c.clear(); });
}

/// Échappe les caractères HTML spéciaux dans une chaîne
/// (prévention XSS de base).
#[no_mangle]
pub extern "C" fn HTML_escape(s_ptr: i64) -> i64 {
    let s = unsafe { ptr_to_str(s_ptr) };
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&'  => out.push_str("&amp;"),
            '<'  => out.push_str("&lt;"),
            '>'  => out.push_str("&gt;"),
            '"'  => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _    => out.push(c),
        }
    }
    unsafe { alloc_str(&out) }
}
