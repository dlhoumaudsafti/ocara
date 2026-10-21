// ─────────────────────────────────────────────────────────────────────────────
// Découverte des tests et génération du `main` de test
//
// Convention :
//   - toute fonction libre dont le nom commence ou se termine par `test`
//     (casse ignorée), même dans un fichier qui déclare aussi des classes ;
//   - toute méthode `public` (d'instance ou `static`) d'une `class` dont le
//     nom commence ou se termine par `test`.
// ─────────────────────────────────────────────────────────────────────────────

use crate::scan::{brace_delta, code_lines, words};

/// Une déclaration de fonction ou de méthode trouvée dans un source.
#[derive(Debug, Clone, PartialEq)]
pub struct Decl {
    /// Ligne (1-basée) de la déclaration
    pub line:       usize,
    pub name:       String,
    /// Classe propriétaire (`None` : fonction libre, ou méthode d'un
    /// module/interface/generic — jamais instanciés comme tests)
    pub class:      Option<String>,
    /// Méthode d'un type (classe, module, interface…) plutôt que fonction libre
    pub is_method:  bool,
    pub is_public:  bool,
    pub is_static:  bool,
}

const CONTAINERS: [&str; 6] = ["class", "generic", "module", "interface", "struct", "enum"];

/// Déclaration de conteneur (`class X`, `module Y`…) en tête de ligne :
/// `Some(classe)` où `classe` vaut `None` pour un conteneur non instanciable.
fn container(w: &[&str]) -> Option<Option<String>> {
    let kind = *w.first()?;
    if !CONTAINERS.contains(&kind) { return None; }
    Some((kind == "class").then(|| w.get(1).map(|n| n.to_string())).flatten())
}

/// `[async] function nom` → nom.
fn free_function(w: &[&str]) -> Option<String> {
    let w = if w.first() == Some(&"async") { &w[1..] } else { w };
    (w.first() == Some(&"function")).then(|| w.get(1).map(|n| n.to_string())).flatten()
}

/// `visibilité [static] [async] method nom` → (nom, public, static).
fn method(w: &[&str]) -> Option<(String, bool, bool)> {
    let vis = *w.first()?;
    if !["public", "protected", "private"].contains(&vis) { return None; }
    let mut rest = &w[1..];
    let is_static = rest.first() == Some(&"static");
    if is_static { rest = &rest[1..]; }
    if rest.first() == Some(&"async") { rest = &rest[1..]; }
    if rest.first() != Some(&"method") { return None; }
    Some((rest.get(1)?.to_string(), vis == "public", is_static))
}

/// Toutes les fonctions libres (profondeur 0) et méthodes (profondeur 1
/// d'un conteneur), dans l'ordre du fichier.
pub fn declarations(source: &str) -> Vec<Decl> {
    let mut decls = Vec::new();
    let mut depth = 0i32;
    // Conteneur courant : Some(classe instanciable ou None)
    let mut owner: Option<Option<String>> = None;
    for (idx, line) in code_lines(source).iter().enumerate() {
        let w = words(line);
        let at = |name: String, class, is_method, is_public, is_static| Decl {
            line: idx + 1, name, class, is_method, is_public, is_static,
        };
        if depth == 0 {
            if let Some(c) = container(&w) {
                owner = Some(c);
            } else if let Some(name) = free_function(&w) {
                decls.push(at(name, None, false, true, false));
            }
        } else if depth == 1 {
            if let (Some(class), Some((name, public, stat))) = (&owner, method(&w)) {
                decls.push(at(name, class.clone(), true, public, stat));
            }
        }
        let before = depth;
        depth = (depth + brace_delta(line)).max(0);
        if before > 0 && depth == 0 {
            owner = None;
        }
    }
    decls
}

/// Vrai si `ident` désigne un test : préfixe ou suffixe `test`, casse
/// ignorée (`fooTest`, `testFoo`, `foo_test`…). La casse relève du linter
/// (ocaracs), pas du runner.
pub fn is_test_name(ident: &str) -> bool {
    let lower = ident.to_ascii_lowercase();
    lower.starts_with("test") || lower.ends_with("test")
}

/// Un test découvert : fonction libre (`class: None`) ou méthode publique
/// d'une classe, d'instance ou statique.
#[derive(Debug, Clone, PartialEq)]
pub struct TestEntry {
    pub class:     Option<String>,
    pub name:      String,
    pub is_static: bool,
}

/// Extrait les tests selon la convention ocaraunit (voir en-tête).
pub fn extract_tests(source: &str) -> Vec<TestEntry> {
    declarations(source).into_iter()
        .filter(|d| is_test_name(&d.name))
        .filter(|d| !d.is_method || (d.is_public && d.class.is_some()))
        .map(|d| TestEntry { class: d.class, name: d.name, is_static: d.is_static })
        .collect()
}

/// Vrai si le fichier déclare déjà une fonction libre `main` : le `main`
/// généré serait en double (le compilateur garderait le premier).
pub fn declares_main(source: &str) -> bool {
    declarations(source).iter().any(|d| !d.is_method && d.name == "main")
}

/// Appel d'un test isolé dans un `try` : une assertion en échec
/// (`UnitTestException`) est rapportée en ligne `FAIL` et les tests suivants
/// du fichier s'exécutent quand même.
fn guarded_call(call: &str, label: &str) -> String {
    format!(
        "    try {{\n        {}\n    }} on _ocaraunit_e is UnitTestException {{\n        \
         IO::writeln(`FAIL [{}] ${{_ocaraunit_e.message}}`)\n    }}\n",
        call, label
    )
}

/// Appels de tous les tests du fichier : fonctions libres appelées
/// directement ; pour chaque classe contenant des tests, une instance
/// (constructeur sans argument) puis chaque méthode, les méthodes statiques
/// étant appelées sur la classe.
pub fn test_calls(tests: &[TestEntry]) -> String {
    let mut code = String::new();
    let mut instances: Vec<(String, String)> = Vec::new();
    for test in tests {
        let (call, label) = match (&test.class, test.is_static) {
            (None, _) => (format!("{}()", test.name), test.name.clone()),
            (Some(class), true) => (format!("{}::{}()", class, test.name), format!("{}::{}", class, test.name)),
            (Some(class), false) => {
                let var = match instances.iter().find(|(c, _)| c == class) {
                    Some((_, v)) => v.clone(),
                    None => {
                        let v = format!("_t{}", instances.len());
                        code.push_str(&format!("    scoped {}:{} = use {}()\n", v, class, class));
                        instances.push((class.clone(), v.clone()));
                        v
                    }
                };
                (format!("{}.{}()", var, test.name), format!("{}.{}", class, test.name))
            }
        };
        code.push_str(&guarded_call(&call, &label));
    }
    code
}

/// Génère un `function main()` qui appelle tous les tests du fichier (voir
/// `test_calls`). Ajouté en fin de fichier : les numéros de ligne du test
/// restent ceux du fichier d'origine.
pub fn generate_main(tests: &[TestEntry]) -> String {
    format!("\nimport ocara.IO\n\nfunction main(): int {{\n{}    return 0\n}}\n", test_calls(tests))
}
