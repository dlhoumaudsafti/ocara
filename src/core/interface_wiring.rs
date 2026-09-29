/// `wiring` — résolution import/alias/nom-nu pour la liaison
/// interface↔implémentation à la compilation (voir
/// docs/roadmap.d/langage-interface-wiring.md pour la spécification
/// complète, déjà tranchée).
///
/// Trois responsabilités regroupées ici :
///
/// 1. `resolve_import_file_path` — la MÊME résolution de chemin que la
///    boucle principale d'imports (`src/main.rs`), factorisée pour être
///    réutilisée par le pré-scan ci-dessous SANS dupliquer cette logique
///    délicate (chemins relatifs `./`/`../`, résolution par namespace
///    courant avec repli sur la racine) — un correctif futur appliqué à
///    l'une des deux copies mais oublié sur l'autre serait exactement la
///    classe de risque déjà documentée ailleurs dans ce compilateur pour ce
///    genre de duplication.
///
/// 2. `collect_all_interfaces` — un PRÉ-SCAN qui parcourt tout le graphe
///    d'imports transitif AVANT la boucle principale de chargement,
///    uniquement pour connaître, pour CHAQUE interface atteignable depuis le
///    fichier d'entrée, ses `wiring` déclarés. Nécessaire car un alias
///    (`import Interface as X`) doit être résolu (`core::alias_resolve`)
///    AVANT que le fichier qui l'a écrit ne soit fusionné dans le programme
///    — ce qui peut arriver AVANT que l'interface elle-même (et donc ses
///    `wiring`) n'ait été chargée par la boucle principale, qui traite les
///    imports dans l'ordre de la file `imports_to_process`, pas dans un
///    ordre de dépendance. Sans connaître les `wiring` par avance, cette
///    résolution d'alias serait un jeu de hasard selon l'ordre de
///    découverte des fichiers (confirmé en y réfléchissant sur le cas le
///    plus simple : le FICHIER PRINCIPAL lui-même alias une interface —
///    `compute_aliases`/`resolve_aliases` s'exécutent pour lui AVANT même
///    que la boucle de chargement des imports ne démarre).
///
/// 3. `resolve_bare_interface_names` — une passe qui tourne APRÈS la fusion
///    complète du programme (tous les `wiring` connus, aucun problème
///    d'ordre à ce stade) : réécrit tout `Expr::New`/`Expr::StaticCall` dont
///    le nom est encore le nom RÉEL (jamais aliasé — un alias est déjà
///    résolu vers la classe wired à ce stade, voir `core::alias_resolve`)
///    d'une interface avec au moins un `wiring`, vers son PREMIER `wiring`
///    déclaré (ordre textuel) — jamais un `Type::Named` (annotation de
///    type), qui doit rester le type abstrait (règle explicitement tranchée
///    par le ticket : la substitution ne s'applique qu'aux positions
///    aujourd'hui dénuées de sens pour une interface nue).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use crate::parsing::ast::*;
use crate::parsing::{lexer::Lexer, parser::Parser};

// ─────────────────────────────────────────────────────────────────────────────
// 1. Résolution de chemin (factorisée avec src/main.rs)
// ─────────────────────────────────────────────────────────────────────────────

pub fn resolve_import_file_path(
    imp: &ImportDecl,
    parent_dir: &Path,
    parent_namespace: &Option<String>,
    source_dir: &Path,
) -> PathBuf {
    let file_path_str = imp.file_path.as_deref().unwrap_or_default();
    let clean_path = file_path_str.trim_end_matches(".oc");

    if clean_path.starts_with("../") || clean_path.starts_with("./") {
        let mut file_path = parent_dir.join(clean_path);
        if file_path.extension().is_none() {
            file_path.set_extension("oc");
        }
        return file_path;
    }

    if parent_namespace.is_some() && parent_namespace.as_deref() != Some(".") {
        let ns = parent_namespace.as_ref().unwrap().replace('.', "/");
        let in_namespace = source_dir.join(&ns).join(clean_path).with_extension("oc");
        if in_namespace.exists() {
            return in_namespace;
        }
        return source_dir.join(clean_path).with_extension("oc");
    }

    source_dir.join(clean_path).with_extension("oc")
}

/// Convertit un import "ancien format" (`import a.b.C`, sans `file_path`) en
/// import virtuel "from" — même conversion que `src/main.rs` pratique déjà
/// pour `module_imports` et pour les imports d'un fichier chargé.
fn as_file_import(imp: &ImportDecl) -> ImportDecl {
    if imp.file_path.is_some() {
        return imp.clone();
    }
    let file_path_str = imp.path.join("/");
    let symbol_name = imp.path.last().cloned().unwrap_or_default();
    ImportDecl {
        path: vec![symbol_name],
        alias: imp.alias.clone(),
        file_path: Some(file_path_str),
        span: imp.span.clone(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Pré-scan : toutes les interfaces (avec leurs wiring) du graphe transitif
// ─────────────────────────────────────────────────────────────────────────────

/// Parcourt tout le graphe d'imports atteignable depuis `entry_program` et
/// retourne CHAQUE interface rencontrée, indexée par son nom réel — voir la
/// doc de module pour la raison de ce pré-scan séparé de la boucle
/// principale de `src/main.rs`.
///
/// Tolérant aux erreurs (fichier illisible/invalide) : ce pré-scan n'a
/// jamais vocation à être la source de vérité des diagnostics de chargement
/// — la boucle principale (`src/main.rs`) revisite de toute façon les mêmes
/// fichiers ensuite et lève l'erreur appropriée avec le bon contexte si un
/// fichier est réellement invalide. Un fichier illisible ici est simplement
/// ignoré (ses interfaces, s'il en a, resteront absentes du résultat — sans
/// conséquence : soit ce fichier n'est de toute façon pas valide et la
/// boucle principale va bientôt échouer dessus avec un message clair, soit
/// il n'est atteignable par aucun `import` réel et n'a jamais eu besoin
/// d'être visité).
pub fn collect_all_interfaces(entry_program: &Program, source_dir: &Path) -> HashMap<String, InterfaceDecl> {
    let mut all_interfaces: HashMap<String, InterfaceDecl> = HashMap::new();
    for iface in &entry_program.interfaces {
        all_interfaces.entry(iface.name.clone()).or_insert_with(|| iface.clone());
    }

    let mut visited: HashSet<PathBuf> = HashSet::new();
    let mut cache: HashMap<PathBuf, Program> = HashMap::new();
    let mut queue: Vec<(ImportDecl, PathBuf, Option<String>)> = Vec::new();

    let main_namespace = entry_program.namespace.clone();
    for imp in &entry_program.imports {
        if imp.path.first().map(|s| s.as_str()) == Some("ocara") {
            continue; // builtin : jamais un fichier .oc à parser
        }
        queue.push((as_file_import(imp), source_dir.to_path_buf(), main_namespace.clone()));
    }

    while let Some((imp, parent_dir, parent_namespace)) = queue.pop() {
        let file_path = resolve_import_file_path(&imp, &parent_dir, &parent_namespace, source_dir);
        let canonical = file_path.canonicalize().unwrap_or_else(|_| file_path.clone());
        if !visited.insert(canonical.clone()) {
            continue;
        }

        let mod_prog = if let Some(p) = cache.get(&canonical) {
            p.clone()
        } else {
            let Ok(src) = std::fs::read_to_string(&file_path) else { continue };
            let Ok(tokens) = Lexer::new(&src).tokenize() else { continue };
            let Ok(parsed) = Parser::new(tokens).parse_program() else { continue };
            cache.insert(canonical.clone(), parsed.clone());
            parsed
        };

        for iface in &mod_prog.interfaces {
            all_interfaces.entry(iface.name.clone()).or_insert_with(|| iface.clone());
        }

        let current_file_dir = file_path.parent().unwrap_or(&parent_dir).to_path_buf();
        let loaded_namespace = mod_prog.namespace.clone();
        for new_imp in &mod_prog.imports {
            if new_imp.path.first().map(|s| s.as_str()) == Some("ocara") {
                continue;
            }
            queue.push((as_file_import(new_imp), current_file_dir.clone(), loaded_namespace.clone()));
        }
    }

    all_interfaces
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Résolution des noms nus (sans alias) en position construction/appel
//    statique — vers le PREMIER wiring déclaré
// ─────────────────────────────────────────────────────────────────────────────

/// Réécrit, dans TOUT le programme déjà fusionné, chaque `Expr::New`/
/// `Expr::StaticCall` dont le nom est encore celui d'une interface avec au
/// moins un `wiring`, vers le nom réel de son PREMIER `wiring` (ordre
/// textuel de déclaration). Ne touche JAMAIS `Type::Named` (annotation de
/// type) — voir la doc de module. Une interface sans aucun `wiring` n'est
/// jamais modifiée ici (elle reste rejetée par ailleurs — voir
/// `SemaError::InterfaceNoWiring` (E38) — au moment du typecheck, pas ici).
pub fn resolve_bare_interface_names(program: &mut Program, all_interfaces: &HashMap<String, InterfaceDecl>) {
    // Aucune interface `wiring`-ée : rien à faire, évite de parcourir tout
    // le programme pour rien (cas de très loin le plus fréquent).
    if all_interfaces.values().all(|i| i.wirings.is_empty()) {
        return;
    }
    let first_wiring = |name: &str| -> Option<String> {
        all_interfaces.get(name).and_then(|i| i.wirings.first()).map(|w| w.simple_name().to_string())
    };

    for c in &mut program.classes {
        for member in &mut c.members {
            resolve_class_member(member, &first_wiring);
        }
    }
    for g in &mut program.generics {
        for member in &mut g.members {
            resolve_class_member(member, &first_wiring);
        }
    }
    for m in &mut program.modules {
        for member in &mut m.members {
            resolve_class_member(member, &first_wiring);
        }
    }
    for f in &mut program.functions {
        resolve_block(&mut f.body, &first_wiring);
    }
    for c in &mut program.consts {
        resolve_expr(&mut c.value, &first_wiring);
    }
    for rb in &mut program.runtime_blocks {
        for s in &mut rb.statements {
            resolve_stmt(s, &first_wiring);
        }
    }
}

fn resolve_class_member(member: &mut ClassMember, first_wiring: &impl Fn(&str) -> Option<String>) {
    match member {
        ClassMember::Method { decl, .. } => resolve_block(&mut decl.body, first_wiring),
        ClassMember::Constructor { body, .. } => resolve_block(body, first_wiring),
        // `const NAME:T = <expr>` — l'expression peut contenir un
        // `use Interface(...)`/`Interface::method()` nu (ex: constante de
        // classe initialisée par une fabrique statique) : même traitement
        // que `core::alias_resolve::resolve_class_member`, qui résout déjà
        // cette valeur pour les alias — un oubli ici laisserait ce SEUL cas
        // pointer encore vers l'interface plutôt que le premier `wiring`.
        ClassMember::Const { value, .. } => resolve_expr(value, first_wiring),
        ClassMember::Field { .. } => {}
    }
}

fn resolve_block(block: &mut Block, first_wiring: &impl Fn(&str) -> Option<String>) {
    for s in &mut block.stmts {
        resolve_stmt(s, first_wiring);
    }
}

fn resolve_stmt(stmt: &mut Stmt, first_wiring: &impl Fn(&str) -> Option<String>) {
    match stmt {
        Stmt::Var { value, .. } | Stmt::Const { value, .. } => resolve_expr(value, first_wiring),
        Stmt::Expr(e) => resolve_expr(e, first_wiring),
        Stmt::If { condition, then_block, elseif, else_block, .. } => {
            resolve_expr(condition, first_wiring);
            resolve_block(then_block, first_wiring);
            for (cond, blk) in elseif {
                resolve_expr(cond, first_wiring);
                resolve_block(blk, first_wiring);
            }
            if let Some(b) = else_block {
                resolve_block(b, first_wiring);
            }
        }
        Stmt::Switch { subject, cases, default, .. } => {
            resolve_expr(subject, first_wiring);
            for c in cases {
                resolve_block(&mut c.body, first_wiring);
            }
            if let Some(d) = default {
                resolve_block(d, first_wiring);
            }
        }
        Stmt::While { condition, body, .. } => {
            resolve_expr(condition, first_wiring);
            resolve_block(body, first_wiring);
        }
        Stmt::ForIn { iter, body, .. } => {
            resolve_expr(iter, first_wiring);
            resolve_block(body, first_wiring);
        }
        Stmt::ForMap { iter, body, .. } => {
            resolve_expr(iter, first_wiring);
            resolve_block(body, first_wiring);
        }
        Stmt::Return { value, .. } | Stmt::Result { value, .. } => {
            if let Some(v) = value {
                resolve_expr(v, first_wiring);
            }
        }
        Stmt::Break { .. } | Stmt::Continue { .. } => {}
        Stmt::Try { body, handlers, .. } => {
            resolve_block(body, first_wiring);
            for h in handlers {
                resolve_block(&mut h.body, first_wiring);
            }
        }
        Stmt::Raise { value, .. } => resolve_expr(value, first_wiring),
        Stmt::Emit { value, .. } => resolve_expr(value, first_wiring),
        Stmt::Assign { target, value, .. } => {
            resolve_expr(target, first_wiring);
            resolve_expr(value, first_wiring);
        }
    }
}

fn resolve_expr(expr: &mut Expr, first_wiring: &impl Fn(&str) -> Option<String>) {
    match expr {
        Expr::Literal(..) | Expr::Ident(..) | Expr::SelfExpr(_) | Expr::ParentExpr(_) => {}
        Expr::Field { object, .. } => resolve_expr(object, first_wiring),
        Expr::Call { callee, args, .. } => {
            resolve_expr(callee, first_wiring);
            for a in args {
                resolve_expr(a, first_wiring);
            }
        }
        Expr::StaticCall { class, args, .. } => {
            // Le seul cas qui compte : le nom nu (jamais aliasé — un alias
            // valide a déjà été résolu vers la classe wired par
            // core::alias_resolve) d'une interface `wiring`-ée.
            if let Some(target) = first_wiring(class) {
                *class = target;
            }
            for a in args {
                resolve_expr(a, first_wiring);
            }
        }
        Expr::StaticConst { .. } => {}
        Expr::New { class, type_args: _, args, .. } => {
            if let Some(target) = first_wiring(class) {
                *class = target;
            }
            for a in args {
                resolve_expr(a, first_wiring);
            }
        }
        Expr::Binary { left, right, .. } => {
            resolve_expr(left, first_wiring);
            resolve_expr(right, first_wiring);
        }
        Expr::Unary { operand, .. } => resolve_expr(operand, first_wiring),
        Expr::Array { elements, .. } => {
            for e in elements {
                resolve_expr(e, first_wiring);
            }
        }
        Expr::Map { entries, .. } => {
            for (k, v) in entries {
                resolve_expr(k, first_wiring);
                resolve_expr(v, first_wiring);
            }
        }
        Expr::Template { parts, .. } => {
            for p in parts {
                if let TemplatePartExpr::Expr(e) = p {
                    resolve_expr(e, first_wiring);
                }
            }
        }
        Expr::Index { object, index, .. } => {
            resolve_expr(object, first_wiring);
            resolve_expr(index, first_wiring);
        }
        Expr::Range { start, end, .. } => {
            resolve_expr(start, first_wiring);
            resolve_expr(end, first_wiring);
        }
        Expr::Match { subject, arms, .. } => {
            resolve_expr(subject, first_wiring);
            for arm in arms {
                resolve_expr(&mut arm.body, first_wiring);
            }
        }
        Expr::Nameless { body, .. } => resolve_block(body, first_wiring),
        Expr::Resolve { expr, .. } | Expr::IsCheck { expr, .. } => resolve_expr(expr, first_wiring),
        Expr::IncDec { target, .. } => resolve_expr(target, first_wiring),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsing::lexer::Lexer;
    use crate::parsing::parser::Parser;

    fn parse(src: &str) -> Program {
        let tokens = Lexer::new(src).tokenize().expect("lex");
        Parser::new(tokens).parse_program().expect("parse")
    }

    /// Bâtit la map `all_interfaces` attendue par `resolve_bare_interface_names`
    /// directement depuis les interfaces déclarées DANS le programme donné —
    /// équivalent, pour un test sans fichier importé, du pré-scan complet de
    /// `collect_all_interfaces` (qui exige un vrai système de fichiers).
    fn interfaces_of(program: &Program) -> HashMap<String, InterfaceDecl> {
        program.interfaces.iter().map(|i| (i.name.clone(), i.clone())).collect()
    }

    /// Cas nominal : `use Repo()` (nom NU, sans alias) sur une interface avec
    /// deux `wiring` doit se réécrire vers le PREMIER déclaré (ordre textuel).
    #[test]
    fn resolve_bare_interface_names_rewrites_new_to_first_wiring() {
        let mut program = parse(
            "interface Repo {\n    wiring infra.PostgresRepo\n    wiring infra.InMemoryRepo\n}\n\
             function main(): void {\n    var r:Repo = use Repo()\n}\n",
        );
        let all = interfaces_of(&program);
        resolve_bare_interface_names(&mut program, &all);

        let Stmt::Var { value, .. } = &program.functions[0].body.stmts[0] else { panic!("expected var") };
        let Expr::New { class, .. } = value else { panic!("expected Expr::New") };
        assert_eq!(class, "PostgresRepo", "doit résoudre vers le PREMIER wiring déclaré, pas le second");
    }

    /// Même chose pour un appel statique nu : `Repo::create()`.
    #[test]
    fn resolve_bare_interface_names_rewrites_static_call_to_first_wiring() {
        let mut program = parse(
            "interface Repo {\n    wiring infra.PostgresRepo\n}\n\
             function main(): void {\n    Repo::create()\n}\n",
        );
        let all = interfaces_of(&program);
        resolve_bare_interface_names(&mut program, &all);

        let Stmt::Expr(Expr::StaticCall { class, .. }) = &program.functions[0].body.stmts[0] else {
            panic!("expected Expr::StaticCall")
        };
        assert_eq!(class, "PostgresRepo");
    }

    /// Une interface SANS aucun `wiring` : le nom nu ne doit JAMAIS être
    /// réécrit ici — il reste rejeté plus tard par le typecheck (E38,
    /// `SemaError::InterfaceNoWiring`), pas par cette passe.
    #[test]
    fn resolve_bare_interface_names_leaves_unwired_interface_untouched() {
        let mut program = parse(
            "interface Repo {\n    method save(): void\n}\n\
             function main(): void {\n    var r:Repo = use Repo()\n}\n",
        );
        let all = interfaces_of(&program);
        resolve_bare_interface_names(&mut program, &all);

        let Stmt::Var { value, .. } = &program.functions[0].body.stmts[0] else { panic!("expected var") };
        let Expr::New { class, .. } = value else { panic!("expected Expr::New") };
        assert_eq!(class, "Repo", "sans wiring, le nom nu doit rester intact");
    }

    /// Le type annoté (`Type::Named`) NE DOIT JAMAIS être réécrit, même dans
    /// la même déclaration où la construction (`Expr::New`) l'est — c'est
    /// la règle centrale du ticket : le polymorphisme via le type abstrait
    /// reste intact, seules les positions "construction"/"appel statique"
    /// sont concernées par la substitution.
    #[test]
    fn resolve_bare_interface_names_never_touches_type_annotations() {
        let mut program = parse(
            "interface Repo {\n    wiring infra.PostgresRepo\n}\n\
             function main(): void {\n    var r:Repo = use Repo()\n}\n",
        );
        let all = interfaces_of(&program);
        resolve_bare_interface_names(&mut program, &all);

        let Stmt::Var { ty, .. } = &program.functions[0].body.stmts[0] else { panic!("expected var") };
        assert_eq!(ty, &Type::Named("Repo".to_string()), "l'annotation de type doit rester l'interface abstraite");
    }

    /// La substitution doit s'appliquer récursivement, y compris à
    /// l'intérieur d'une constante de classe (`ClassMember::Const`) — voir
    /// le correctif ajouté à `resolve_class_member` (initialement un no-op
    /// pour `Const`, contrairement à `core::alias_resolve`).
    #[test]
    fn resolve_bare_interface_names_recurses_into_class_const_value() {
        let mut program = parse(
            "interface Repo {\n    wiring infra.PostgresRepo\n}\n\
             class Factory {\n    public const DEFAULT:Repo = use Repo()\n}\n",
        );
        let all = interfaces_of(&program);
        resolve_bare_interface_names(&mut program, &all);

        let ClassMember::Const { value, .. } = &program.classes[0].members[0] else { panic!("expected const") };
        let Expr::New { class, .. } = value else { panic!("expected Expr::New") };
        assert_eq!(class, "PostgresRepo");
    }

    /// Aucune interface `wiring`-ée dans tout le programme : la passe doit
    /// être un no-op total (court-circuit de tête), y compris sur un
    /// programme qui référence des interfaces par ailleurs.
    #[test]
    fn resolve_bare_interface_names_is_noop_when_nothing_is_wired() {
        let mut program = parse(
            "interface Repo {\n    method save(): void\n}\n\
             function main(): void {\n    var r:Repo = use Repo()\n}\n",
        );
        let before = program.clone();
        let all = interfaces_of(&program);
        resolve_bare_interface_names(&mut program, &all);
        assert_eq!(program, before);
    }
}
