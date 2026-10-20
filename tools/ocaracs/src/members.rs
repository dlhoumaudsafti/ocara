// ─────────────────────────────────────────────────────────────────────────────
// Receveurs des accès membres (`x.nom`, `T::nom`) : un membre renommé ne l'est
// que si son receveur peut être un type du projet. `win.isOpen()` sur un
// `SDL` garde son nom ; un receveur dont le type est inconnu rend le
// renommage ambigu (il est alors ignoré et signalé).
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::text::masked_code;

/// Mots qui déclarent un type du projet.
const TYPE_KEYWORDS: [&str; 7] = ["class ", "struct ", "interface ", "module ", "generic ", "enum ", "wiring "];

/// Décision pour un accès membre.
#[derive(Debug, PartialEq)]
pub enum Verdict {
    /// Le receveur est (ou peut seulement être) un type du projet.
    Rename,
    /// Le receveur est un builtin : le membre n'est pas celui du projet.
    Keep,
    /// Type du receveur inconnu ou mêlé : impossible de trancher.
    Ambiguous,
}

/// Accès membre : receveur (`None` pour une expression : `)`, `]`…), `::`
/// ou `.`, et ligne (0-based) de l'accès.
#[derive(Debug, PartialEq)]
pub struct MemberAccess {
    pub receiver: Option<String>,
    pub path:     bool,
    pub line:     usize,
}

/// Déclarations `nom:Type` d'un fichier : nom → (ligne, type), dans l'ordre.
type FileTypes = HashMap<String, Vec<(usize, String)>>;

#[derive(Default)]
pub struct Receivers {
    /// Déclarations de chaque fichier.
    files: HashMap<PathBuf, FileTypes>,
    /// Nom → types rencontrés dans tout le projet (receveur déclaré ailleurs,
    /// comme une propriété d'une autre classe).
    types: HashMap<String, HashSet<String>>,
    /// Types déclarés dans le projet.
    user_types: HashSet<String>,
}

impl Receivers {
    pub fn collect(files: &[(PathBuf, String)]) -> Self {
        let mut out = Receivers::default();
        for (path, content) in files {
            let mut in_bt = false;
            let mut local = FileTypes::new();
            for (i, line) in content.lines().enumerate() {
                let code = masked_code(line, &mut in_bt);
                out.scan_type_decl(code.trim_start());
                for (name, ty) in typed_names(&code) {
                    out.types.entry(name.clone()).or_default().insert(ty.clone());
                    local.entry(name).or_default().push((i, ty));
                }
            }
            out.files.insert(path.clone(), local);
        }
        out
    }

    fn scan_type_decl(&mut self, code: &str) {
        let code = crate::text::strip_visibility(code);
        if let Some(kw) = TYPE_KEYWORDS.iter().find(|kw| code.starts_with(*kw)) {
            let name = crate::text::leading_name(&code[kw.len()..]);
            if !name.is_empty() { self.user_types.insert(name.to_string()); }
        }
    }

    /// Types possibles du receveur `name` à la ligne `line` de `file` : la
    /// dernière déclaration qui précède dans le fichier, sinon toutes celles
    /// du fichier, sinon celles du projet.
    fn receiver_types(&self, file: &Path, name: &str, line: usize) -> Option<HashSet<String>> {
        if let Some(decls) = self.files.get(file).and_then(|f| f.get(name)) {
            let before = decls.iter().rev().find(|(l, _)| *l <= line);
            return Some(match before {
                Some((_, ty)) => HashSet::from([ty.clone()]),
                None => decls.iter().map(|(_, ty)| ty.clone()).collect(),
            });
        }
        self.types.get(name).cloned()
    }

    /// Verdict pour l'accès membre `access` du fichier `file`.
    pub fn verdict(&self, file: &Path, access: &MemberAccess) -> Verdict {
        let Some(receiver) = access.receiver.as_deref() else { return Verdict::Ambiguous };
        if receiver == "self" || receiver == "parent" { return Verdict::Rename; }
        if self.user_types.contains(receiver) { return Verdict::Rename; }
        if access.path {
            let builtin = receiver.starts_with(|c: char| c.is_ascii_uppercase());
            return if builtin { Verdict::Keep } else { Verdict::Ambiguous };
        }
        let Some(types) = self.receiver_types(file, receiver, access.line) else { return Verdict::Ambiguous };
        let user = types.iter().filter(|t| self.user_types.contains(*t)).count();
        match user {
            0 => Verdict::Keep,
            n if n == types.len() => Verdict::Rename,
            _ => Verdict::Ambiguous,
        }
    }
}

/// Paires `nom:Type` du code masqué d'une ligne (variables, paramètres,
/// propriétés, champs) ; `Type[]` et `array<…>`/`map<…>` donnent `array`/`map`.
fn typed_names(code: &str) -> Vec<(String, String)> {
    let chars: Vec<char> = code.chars().collect();
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    let mut out = Vec::new();
    for (i, &c) in chars.iter().enumerate() {
        let colon_alone = c == ':' && chars.get(i + 1) != Some(&':') && (i == 0 || chars[i - 1] != ':');
        if !colon_alone { continue; }
        let mut end = i;
        while end > 0 && chars[end - 1] == ' ' { end -= 1; }
        let mut start = end;
        while start > 0 && is_ident(chars[start - 1]) { start -= 1; }
        let mut t = i + 1;
        while t < chars.len() && chars[t] == ' ' { t += 1; }
        let mut t_end = t;
        while t_end < chars.len() && is_ident(chars[t_end]) { t_end += 1; }
        if start == end || t == t_end || !(chars[start].is_alphabetic() || chars[start] == '_') { continue; }
        let mut ty: String = chars[t..t_end].iter().collect();
        if chars.get(t_end) == Some(&'[') { ty = "array".to_string(); }
        out.push((chars[start..end].iter().collect(), ty));
    }
    out
}

/// Identifiant qui précède l'opérateur membre finissant juste avant `op_start`
/// (`None` si c'est une expression : `)`, `]`, chaîne…).
pub fn receiver_before(chars: &[char], op_start: usize) -> Option<String> {
    let mut end = op_start;
    while end > 0 && chars[end - 1] == ' ' { end -= 1; }
    let mut start = end;
    while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') { start -= 1; }
    (start < end && !chars[start].is_ascii_digit()).then(|| chars[start..end].iter().collect())
}

#[cfg(test)]
mod tests {
    use super::{MemberAccess, Receivers, Verdict};
    use std::path::{Path, PathBuf};

    fn verdict(r: &Receivers, receiver: Option<&str>, path: bool, line: usize) -> Verdict {
        let access = MemberAccess { receiver: receiver.map(str::to_string), path, line };
        r.verdict(Path::new("a.oc"), &access)
    }

    #[test]
    fn builtin_receivers_keep_their_members() {
        let src = "class Window {\n    public property isOpen:bool\n}\nfunction f(win:SDL, w:Window, cars:Window[]): void {\n    var s:string = \"x:Window\"\n}\n";
        let r = Receivers::collect(&[(PathBuf::from("a.oc"), src.to_string())]);
        assert_eq!(verdict(&r, Some("win"), false, 4), Verdict::Keep);
        assert_eq!(verdict(&r, Some("w"), false, 4), Verdict::Rename);
        assert_eq!(verdict(&r, Some("self"), false, 1), Verdict::Rename);
        assert_eq!(verdict(&r, Some("cars"), false, 4), Verdict::Keep);
        assert_eq!(verdict(&r, Some("s"), false, 5), Verdict::Keep);
        assert_eq!(verdict(&r, Some("Window"), true, 5), Verdict::Rename);
        assert_eq!(verdict(&r, Some("SDL"), true, 5), Verdict::Keep);
        assert_eq!(verdict(&r, Some("unknown"), false, 5), Verdict::Ambiguous);
        assert_eq!(verdict(&r, None, false, 5), Verdict::Ambiguous);
    }

    #[test]
    fn the_closest_previous_declaration_types_the_receiver() {
        let files = [
            (PathBuf::from("a.oc"), "class Box {\n}\nfunction f(): void {\n    var s:string = \"\"\n}\nfunction g(): void {\n    var s:Box = use Box()\n    s.open()\n}\n".to_string()),
            (PathBuf::from("b.oc"), "function h(s:Box): void {\n}\n".to_string()),
        ];
        let r = Receivers::collect(&files);
        assert_eq!(verdict(&r, Some("s"), false, 7), Verdict::Rename);
        assert_eq!(verdict(&r, Some("s"), false, 4), Verdict::Keep);
        let other = MemberAccess { receiver: Some("s".to_string()), path: false, line: 0 };
        assert_eq!(r.verdict(Path::new("c.oc"), &other), Verdict::Ambiguous);
    }
}
