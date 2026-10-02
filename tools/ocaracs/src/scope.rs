// ─────────────────────────────────────────────────────────────────────────────
// Suivi des corps (R13) : fonction, méthode, constructeur `init(...)`,
// `nameless`, bloc runtime (`main { }`…) et fichier runtime entier
// (`*.runtime.oc`) — une `const` déclarée dans l'un d'eux est locale, sinon
// globale ou de classe (R09).
// Accolades comptées hors chaînes "…", hors backticks et hors commentaires.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct BodyTracker {
    depth:   usize,
    /// Profondeur d'ouverture de chaque corps en cours.
    bodies:  Vec<usize>,
    /// Déclaration vue, corps pas encore ouvert (signature sur plusieurs lignes).
    pending: Option<usize>,
    in_backtick: bool,
    /// Fichier runtime : tout son contenu est le corps d'un bloc runtime.
    whole_file: bool,
}

const RUNTIME_SUFFIXES: [&str; 3] = [".runtime.oc", ".run.oc", ".rt.oc"];
const RUNTIME_BLOCKS: [&str; 5] = ["init", "main", "error", "success", "exit"];

/// `function`/`method` en tête de ligne (visibilité, `static`, `async` admis).
pub fn is_callable_decl(trimmed: &str) -> bool {
    let mut rest = trimmed;
    for prefix in ["public ", "protected ", "private ", "static ", "async "] {
        rest = rest.strip_prefix(prefix).map(str::trim_start).unwrap_or(rest);
    }
    rest.starts_with("function ") || rest.starts_with("method ")
        || rest.strip_prefix("init").is_some_and(|r| r.trim_start().starts_with('('))
}

/// `main {`, `init {`… (RuntimeBlockKind suivi directement de son bloc).
fn is_runtime_block(trimmed: &str) -> bool {
    RUNTIME_BLOCKS.iter().any(|kind| trimmed.strip_prefix(kind).is_some_and(|r| r.trim_start().starts_with('{')))
}

impl BodyTracker {
    pub fn for_file(file_name: &str) -> Self {
        BodyTracker { whole_file: RUNTIME_SUFFIXES.iter().any(|s| file_name.ends_with(s)), ..Self::default() }
    }

    /// Vrai si la ligne courante commence à l'intérieur d'un corps.
    pub fn in_body(&self) -> bool {
        self.whole_file || self.bodies.last().is_some_and(|&open| self.depth > open)
    }

    /// Met à jour l'état avec la ligne courante (à appeler après `in_body`).
    pub fn advance(&mut self, line: &str) {
        let trimmed = line.trim_start();
        if !self.in_backtick && (is_callable_decl(trimmed) || is_runtime_block(trimmed) || trimmed.contains("nameless(")) {
            self.pending = Some(self.depth);
        }
        let mut in_str = false;
        let mut prev = '\0';
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            let escaped = prev == '\\';
            prev = c;
            if escaped { continue; }
            match c {
                '`' if !in_str => self.in_backtick = !self.in_backtick,
                '"' if !self.in_backtick => in_str = !in_str,
                '/' if !in_str && !self.in_backtick && chars.peek() == Some(&'/') => break,
                '{' if !in_str && !self.in_backtick => self.open(),
                '}' if !in_str && !self.in_backtick => self.close(),
                _ => {}
            }
        }
    }

    fn open(&mut self) {
        if self.pending == Some(self.depth) {
            self.bodies.push(self.depth);
            self.pending = None;
        }
        self.depth += 1;
    }

    fn close(&mut self) {
        self.depth = self.depth.saturating_sub(1);
        if self.bodies.last() == Some(&self.depth) {
            self.bodies.pop();
        }
        if self.pending.is_some_and(|d| self.depth < d) {
            self.pending = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BodyTracker;

    fn body_flags(src: &str) -> Vec<bool> {
        let mut tracker = BodyTracker::default();
        src.lines().map(|l| { let inside = tracker.in_body(); tracker.advance(l); inside }).collect()
    }

    #[test]
    fn class_constant_vs_method_constant() {
        let src = "const A = 1\nclass C {\n    public const B:int = 2\n    public method m(): int {\n        const c:int = 3\n        return c\n    }\n}\n";
        assert_eq!(body_flags(src), vec![false, false, false, false, true, true, true, false]);
    }

    #[test]
    fn runtime_blocks_constructors_and_runtime_files() {
        let src = "main {\n    const db:int = 1\n}\nclass C {\n    public init(x:int) {\n        const y:int = x\n    }\n}\n";
        assert_eq!(body_flags(src), vec![false, true, true, false, false, true, true, false]);
        assert!(BodyTracker::for_file("core/init.runtime.oc").in_body());
        assert!(!BodyTracker::for_file("core/main.oc").in_body());
    }

    #[test]
    fn interface_signature_does_not_open_a_body() {
        let src = "interface I {\n    method draw(): void\n}\nclass C {\n    public const X:int = 1\n}\n";
        assert_eq!(body_flags(src), vec![false; 6]);
    }

    #[test]
    fn braces_in_strings_and_comments_are_ignored() {
        let src = "function main(): int {\n    var s:string = \"}\" // }\n    var h:string = `a }\n}`\n    const x:int = 1\n    return 0\n}\nconst Y = 2\n";
        assert_eq!(body_flags(src), vec![false, true, true, true, true, true, true, false]);
    }
}
