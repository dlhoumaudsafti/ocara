/// Analyse d'échappement interprocédurale des paramètres de fonction/
/// méthode/constructeur UTILISATEUR — voir docs/roadmap.d/memoire-strategie-var.md
/// et docs/roadmap.d/memoire-echappement-argument.md.
///
/// Sert deux besoins aux exigences de sûreté différentes :
///   - `sema::typecheck` (diagnostic E26, ArgumentEscape) : rater un
///     échappement réel n'est pas pire qu'aujourd'hui (rien n'est vérifié du
///     tout sur un argument actuellement) — un résultat "au mieux", imprécis
///     sur des constructions non modélisées ici (ex. une valeur qui
///     s'échappe seulement à travers une branche de `match` non triviale),
///     est une amélioration nette, jamais une régression.
///   - `lower::stmt::ownership` (libération automatique d'un `var`, via
///     `var_never_escapes`) : rater un échappement réel y serait un
///     USE-AFTER-FREE NOUVEAU (aujourd'hui `var` ne libère jamais rien, donc
///     jamais de UAF côté `var`) — ce consommateur n'utilise ce module QUE
///     pour savoir si un appel à une fonction/méthode/constructeur
///     UTILISATEUR *connu* est *prouvé* sûr ; un appel non résolu (builtin,
///     ou callee qu'on ne sait pas résoudre) doit TOUJOURS être traité comme
///     échappant de ce côté-là, jamais l'inverse — cette prudence-là est
///     imposée par construction : `check_call_args` traite un callee non
///     résolu comme échappant pour CHAQUE argument, pas seulement absent de
///     vérification (voir sa doc).
///
/// Modélise explicitement : `return`/`result`, `raise`, affectation à un
/// champ (`self.x = p` ou `obj.x = p`) ou à un élément de tableau/map
/// (`arr[i] = p`), affectation à une autre variable (propage le "taint"),
/// passage en argument à un appel RÉSOLU vers une classe utilisateur
/// (constructeur ou méthode), capture par une closure (`nameless`), et
/// propagation à travers les branches d'un `match`. Toute autre forme
/// d'expression produit une valeur considérée fraîche (non traçée) — limite
/// assumée et documentée, pas un oubli.
use std::collections::{HashMap, HashSet};
use crate::parsing::ast::*;

/// Identifie une fonction libre (son nom) ou une méthode/constructeur de
/// classe (`"Classe::méthode"`, `"Classe::init"`) de façon unique dans le programme.
pub type CalleeKey = String;

/// `class_name → { noms de membres appelables (méthodes + "init") }` — vue
/// minimale du programme suffisante pour résoudre un appel vers une
/// `CalleeKey`, sans dépendre de `&Program` (utile côté `lower`, où seul
/// `IrModule` — pas l'AST complet — est systématiquement disponible ; voir
/// `IrModule::class_members`/`escaping_params`,
/// `src/lower/builder.d/program.rs`).
pub type ClassMembers = HashMap<String, HashSet<String>>;

pub fn collect_class_members(classes: &[ClassDecl]) -> ClassMembers {
    let mut out: ClassMembers = HashMap::new();
    for class in classes {
        let mut names: HashSet<String> = HashSet::new();
        for member in &class.members {
            match member {
                ClassMember::Constructor { .. } => { names.insert("init".to_string()); }
                ClassMember::Method { decl, .. } => { names.insert(decl.name.clone()); }
                ClassMember::Field { .. } | ClassMember::Const { .. } => {}
            }
        }
        out.insert(class.name.clone(), names);
    }
    out
}

struct Callable<'p> {
    key:         CalleeKey,
    param_names: Vec<String>,
    body:        &'p Block,
    self_class:  Option<&'p str>,
}

fn collect_callables(program: &Program) -> Vec<Callable<'_>> {
    let mut callables = Vec::new();
    for func in &program.functions {
        callables.push(Callable {
            key:         func.name.clone(),
            param_names: func.params.iter().map(|p| p.name.clone()).collect(),
            body:        &func.body,
            self_class:  None,
        });
    }
    for class in &program.classes {
        for member in &class.members {
            match member {
                ClassMember::Constructor { params, body, .. } => {
                    callables.push(Callable {
                        key:         format!("{}::init", class.name),
                        param_names: params.iter().map(|p| p.name.clone()).collect(),
                        body,
                        self_class:  Some(class.name.as_str()),
                    });
                }
                ClassMember::Method { decl, .. } => {
                    callables.push(Callable {
                        key:         format!("{}::{}", class.name, decl.name),
                        param_names: decl.params.iter().map(|p| p.name.clone()).collect(),
                        body:        &decl.body,
                        self_class:  Some(class.name.as_str()),
                    });
                }
                ClassMember::Field { .. } | ClassMember::Const { .. } => {}
            }
        }
    }
    callables
}

/// Calcule, pour chaque fonction/méthode/constructeur utilisateur, quels
/// paramètres "s'échappent" (indexés dans l'ordre de déclaration).
///
/// Point fixe conservateur : chaque paramètre commence à `false` ("pas
/// prouvé échappant") et ne peut transitionner que vers `true` — converge
/// donc forcément, en au plus N itérations (N = nombre total de paramètres
/// du programme, toutes fonctions confondues).
pub fn compute_escaping_params(program: &Program) -> HashMap<CalleeKey, Vec<bool>> {
    let callables = collect_callables(program);
    let class_members = collect_class_members(&program.classes);

    let mut result: HashMap<CalleeKey, Vec<bool>> = callables.iter()
        .map(|c| (c.key.clone(), vec![false; c.param_names.len()]))
        .collect();

    loop {
        let mut changed = false;
        for c in &callables {
            let escaped = trace_escapes_in_body(&class_members, &c.param_names, c.body, c.self_class, &result);
            let slot = result.get_mut(&c.key).expect("callable enregistré dans result");
            for i in 0..slot.len() {
                if escaped.contains(&i) && !slot[i] {
                    slot[i] = true;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    result
}

/// Résout un couple (classe, membre) vers une `CalleeKey`, uniquement si
/// `class_name` est une classe UTILISATEUR connue (présente dans
/// `class_members`) possédant réellement ce constructeur/cette méthode —
/// `None` pour tout le reste (builtin, classe inconnue, generic non
/// instancié) : ces appels ne sont jamais vérifiés par ce module, exactement
/// le comportement d'aujourd'hui, préservé délibérément.
pub fn resolve_user_callable(class_members: &ClassMembers, class_name: &str, member_name: &str) -> Option<CalleeKey> {
    let members = class_members.get(class_name)?;
    if members.contains(member_name) {
        Some(format!("{}::{}", class_name, member_name))
    } else {
        None
    }
}

/// Trace, dans un unique corps de fonction/méthode, quels noms parmi
/// `tracked` (paramètres ou variables locales selon l'appelant, indexés
/// dans l'ordre du slice) s'échappent — retourne l'ensemble des index de
/// `tracked` concernés.
pub fn trace_escapes_in_body(
    class_members: &ClassMembers,
    tracked: &[String],
    body: &Block,
    self_class: Option<&str>,
    known: &HashMap<CalleeKey, Vec<bool>>,
) -> HashSet<usize> {
    let mut taint: HashMap<String, HashSet<usize>> = HashMap::new();
    for (i, name) in tracked.iter().enumerate() {
        taint.insert(name.clone(), std::iter::once(i).collect());
    }
    let mut escaped: HashSet<usize> = HashSet::new();
    // Non strict : voir la doc de module — un appel non résolu (builtin) est
    // simplement ignoré, comme aujourd'hui (E26 : rater un échappement n'est
    // pas pire qu'avant, où rien n'était vérifié du tout).
    walk_block(class_members, body, self_class, known, &mut taint, &mut escaped, false);
    escaped
}

/// Détermine si `var_name`, déclaré directement dans `block` à l'index
/// `decl_index` (donc `block.stmts[decl_index]` est bien sa déclaration),
/// s'échappe entre ce point et la fin de la fonction — c'est-à-dire dans le
/// reste de CE bloc (`decl_index + 1..`, y compris les blocs imbriqués dans
/// ces statements-là, parcourus récursivement) : rien après la fin du bloc
/// ne peut de toute façon référencer `var_name`, qui sort de portée à la
/// fermeture de son bloc déclarant (mêmes règles de portée qu'un `scoped`).
///
/// Volontairement TRÈS conservateur (voir doc de module) : `false` dès que
/// la moindre forme d'échappement, même imprécisément détectée, est trouvée.
pub fn var_never_escapes(
    class_members: &ClassMembers,
    var_name: &str,
    block: &Block,
    decl_index: usize,
    self_class: Option<&str>,
    known: &HashMap<CalleeKey, Vec<bool>>,
) -> bool {
    let mut taint: HashMap<String, HashSet<usize>> = HashMap::new();
    taint.insert(var_name.to_string(), std::iter::once(0).collect());
    let mut escaped: HashSet<usize> = HashSet::new();
    // Strict : un appel dont le callee n'est PAS résolu (builtin, ex.
    // `Array::push(arr, x)`) doit être traité comme retenant son argument —
    // rater ça libérerait `x` alors qu'il est en réalité stocké dans `arr`
    // (confirmé par reproduction : double free/use-after-free). Voir la doc
    // de module — asymétrie assumée avec le mode non strict utilisé pour E26.
    for stmt in &block.stmts[decl_index + 1..] {
        walk_stmt(class_members, stmt, self_class, known, &mut taint, &mut escaped, true);
    }
    escaped.is_empty()
}

fn walk_block(
    class_members: &ClassMembers, block: &Block, self_class: Option<&str>,
    known: &HashMap<CalleeKey, Vec<bool>>,
    taint: &mut HashMap<String, HashSet<usize>>, escaped: &mut HashSet<usize>,
    strict: bool,
) {
    for stmt in &block.stmts {
        walk_stmt(class_members, stmt, self_class, known, taint, escaped, strict);
    }
}

fn walk_stmt(
    class_members: &ClassMembers, stmt: &Stmt, self_class: Option<&str>,
    known: &HashMap<CalleeKey, Vec<bool>>,
    taint: &mut HashMap<String, HashSet<usize>>, escaped: &mut HashSet<usize>,
    strict: bool,
) {
    match stmt {
        Stmt::Var { name, value, .. } => {
            walk_expr_for_calls(class_members, value, self_class, known, taint, escaped, strict);
            let t = taint_of_expr(value, taint);
            if t.is_empty() { taint.remove(name); } else { taint.insert(name.clone(), t); }
        }
        Stmt::Const { value, .. } => {
            walk_expr_for_calls(class_members, value, self_class, known, taint, escaped, strict);
        }
        Stmt::Expr(e) => walk_expr_for_calls(class_members, e, self_class, known, taint, escaped, strict),
        Stmt::Assign { target, value, .. } => {
            walk_expr_for_calls(class_members, value, self_class, known, taint, escaped, strict);
            let t = taint_of_expr(value, taint);
            match target {
                Expr::Ident(name, _) => {
                    if t.is_empty() { taint.remove(name); } else { taint.insert(name.clone(), t); }
                }
                Expr::Field { object, .. } => {
                    // Stockage dans un champ (self.x = ... ou obj.x = ...) :
                    // échappe quel que soit le porteur — on ne trace pas la
                    // durée de vie de l'objet cible plus loin.
                    escaped.extend(t);
                    walk_expr_for_calls(class_members, object, self_class, known, taint, escaped, strict);
                }
                Expr::Index { object, index, .. } => {
                    // Stockage dans un élément de tableau/map : idem.
                    escaped.extend(t);
                    walk_expr_for_calls(class_members, object, self_class, known, taint, escaped, strict);
                    walk_expr_for_calls(class_members, index, self_class, known, taint, escaped, strict);
                }
                _ => {}
            }
        }
        Stmt::Return { value: Some(e), .. } | Stmt::Result { value: Some(e), .. } => {
            walk_expr_for_calls(class_members, e, self_class, known, taint, escaped, strict);
            escaped.extend(taint_of_expr(e, taint));
        }
        Stmt::Return { .. } | Stmt::Result { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => {}
        Stmt::If { condition, then_block, elseif, else_block, .. } => {
            walk_expr_for_calls(class_members, condition, self_class, known, taint, escaped, strict);
            walk_block(class_members, then_block, self_class, known, taint, escaped, strict);
            for (c, b) in elseif {
                walk_expr_for_calls(class_members, c, self_class, known, taint, escaped, strict);
                walk_block(class_members, b, self_class, known, taint, escaped, strict);
            }
            if let Some(b) = else_block { walk_block(class_members, b, self_class, known, taint, escaped, strict); }
        }
        Stmt::Switch { subject, cases, default, .. } => {
            walk_expr_for_calls(class_members, subject, self_class, known, taint, escaped, strict);
            for c in cases { walk_block(class_members, &c.body, self_class, known, taint, escaped, strict); }
            if let Some(b) = default { walk_block(class_members, b, self_class, known, taint, escaped, strict); }
        }
        Stmt::While { condition, body, .. } => {
            walk_expr_for_calls(class_members, condition, self_class, known, taint, escaped, strict);
            walk_block(class_members, body, self_class, known, taint, escaped, strict);
        }
        Stmt::ForIn { iter, body, .. } => {
            walk_expr_for_calls(class_members, iter, self_class, known, taint, escaped, strict);
            walk_block(class_members, body, self_class, known, taint, escaped, strict);
        }
        Stmt::ForMap { iter, body, .. } => {
            walk_expr_for_calls(class_members, iter, self_class, known, taint, escaped, strict);
            walk_block(class_members, body, self_class, known, taint, escaped, strict);
        }
        Stmt::Try { body, handlers, .. } => {
            walk_block(class_members, body, self_class, known, taint, escaped, strict);
            for h in handlers { walk_block(class_members, &h.body, self_class, known, taint, escaped, strict); }
        }
        Stmt::Raise { value, .. } => {
            walk_expr_for_calls(class_members, value, self_class, known, taint, escaped, strict);
            // Une valeur levée peut survivre à l'appel courant (portée par
            // l'objet exception) — traitée prudemment comme échappante.
            escaped.extend(taint_of_expr(value, taint));
        }
        Stmt::Emit { value, .. } => {
            walk_expr_for_calls(class_members, value, self_class, known, taint, escaped, strict);
            // Même raisonnement que `return`/`raise` : la valeur émise part
            // vers le consommateur du générateur, traitée prudemment comme
            // échappante (voir docs/roadmap.d/langage-emit-iterable.md).
            escaped.extend(taint_of_expr(value, taint));
        }
    }
}

/// Ce qu'une expression "vaut" du point de vue du taint : uniquement un
/// identifiant simple, ou la réunion des branches d'un `match` (les autres
/// formes — appel, littéral, champ, opération... — produisent une valeur
/// fraîche, jamais un alias direct de `tracked`).
fn taint_of_expr(expr: &Expr, taint: &HashMap<String, HashSet<usize>>) -> HashSet<usize> {
    match expr {
        Expr::Ident(name, _) => taint.get(name).cloned().unwrap_or_default(),
        Expr::Match { arms, .. } => {
            let mut out = HashSet::new();
            for arm in arms { out.extend(taint_of_expr(&arm.body, taint)); }
            out
        }
        _ => HashSet::new(),
    }
}

/// Parcourt exhaustivement `expr` à la recherche de points d'échappement :
/// argument d'un appel résolu vers un paramètre échappant, élément d'un
/// literal array/map, capture par une closure `nameless`.
fn walk_expr_for_calls(
    class_members: &ClassMembers, expr: &Expr, self_class: Option<&str>,
    known: &HashMap<CalleeKey, Vec<bool>>,
    taint: &mut HashMap<String, HashSet<usize>>, escaped: &mut HashSet<usize>,
    strict: bool,
) {
    match expr {
        Expr::Ident(..) | Expr::Literal(..) | Expr::SelfExpr(_) | Expr::ParentExpr(_) | Expr::StaticConst { .. } => {}
        Expr::Field { object, .. } => walk_expr_for_calls(class_members, object, self_class, known, taint, escaped, strict),
        Expr::Call { callee, args, .. } => {
            walk_expr_for_calls(class_members, callee, self_class, known, taint, escaped, strict);
            let resolved = match callee.as_ref() {
                Expr::Field { object, field, .. } if matches!(object.as_ref(), Expr::SelfExpr(_)) => {
                    self_class.and_then(|c| resolve_user_callable(class_members, c, field))
                }
                _ => None, // récepteur autre que `self` : type non tracé ici, non résolu (voir doc de module)
            };
            check_call_args(args, resolved.as_deref(), known, taint, escaped, strict);
            for a in args { walk_expr_for_calls(class_members, a, self_class, known, taint, escaped, strict); }
        }
        Expr::StaticCall { class, method, args, .. } => {
            let resolved = resolve_user_callable(class_members, class, method);
            check_call_args(args, resolved.as_deref(), known, taint, escaped, strict);
            for a in args { walk_expr_for_calls(class_members, a, self_class, known, taint, escaped, strict); }
        }
        Expr::New { class, args, .. } => {
            let resolved = resolve_user_callable(class_members, class, "init");
            check_call_args(args, resolved.as_deref(), known, taint, escaped, strict);
            for a in args { walk_expr_for_calls(class_members, a, self_class, known, taint, escaped, strict); }
        }
        Expr::Binary { left, right, .. } => {
            walk_expr_for_calls(class_members, left, self_class, known, taint, escaped, strict);
            walk_expr_for_calls(class_members, right, self_class, known, taint, escaped, strict);
        }
        Expr::Unary { operand, .. } => walk_expr_for_calls(class_members, operand, self_class, known, taint, escaped, strict),
        Expr::Index { object, index, .. } => {
            walk_expr_for_calls(class_members, object, self_class, known, taint, escaped, strict);
            walk_expr_for_calls(class_members, index, self_class, known, taint, escaped, strict);
        }
        Expr::Range { start, end, .. } => {
            walk_expr_for_calls(class_members, start, self_class, known, taint, escaped, strict);
            walk_expr_for_calls(class_members, end, self_class, known, taint, escaped, strict);
        }
        Expr::Array { elements, .. } => {
            // Un élément taint stocké dans un literal array s'échappe : on
            // ne trace pas la durée de vie du tableau résultant plus loin.
            for e in elements {
                escaped.extend(taint_of_expr(e, taint));
                walk_expr_for_calls(class_members, e, self_class, known, taint, escaped, strict);
            }
        }
        Expr::Map { entries, .. } => {
            for (k, v) in entries {
                escaped.extend(taint_of_expr(k, taint));
                escaped.extend(taint_of_expr(v, taint));
                walk_expr_for_calls(class_members, k, self_class, known, taint, escaped, strict);
                walk_expr_for_calls(class_members, v, self_class, known, taint, escaped, strict);
            }
        }
        Expr::Template { parts, .. } => {
            for part in parts {
                if let TemplatePartExpr::Expr(e) = part {
                    walk_expr_for_calls(class_members, e, self_class, known, taint, escaped, strict);
                }
            }
        }
        Expr::Match { subject, arms, .. } => {
            walk_expr_for_calls(class_members, subject, self_class, known, taint, escaped, strict);
            for arm in arms { walk_expr_for_calls(class_members, &arm.body, self_class, known, taint, escaped, strict); }
        }
        Expr::IsCheck { expr, .. } => walk_expr_for_calls(class_members, expr, self_class, known, taint, escaped, strict),
        Expr::Resolve { expr, .. } => walk_expr_for_calls(class_members, expr, self_class, known, taint, escaped, strict),
        Expr::IncDec { target, .. } => walk_expr_for_calls(class_members, target, self_class, known, taint, escaped, strict),
        Expr::Nameless { body, .. } => {
            // Une closure peut survivre à l'appel courant (Thread::spawn la
            // stocke pour exécution différée) — toute variable suivie
            // référencée n'importe où dans son corps est traitée comme
            // capturée, donc échappante. Ne descend pas plus loin dans son
            // propre corps pour la détection d'appels (ses appels internes
            // concernent la portée de LA closure, pas celle-ci).
            let mut refs = HashSet::new();
            collect_ident_refs(body, &mut refs);
            for name in &refs {
                if let Some(t) = taint.get(name) {
                    escaped.extend(t.iter().copied());
                }
            }
        }
    }
}

/// Vérifie chaque argument d'un appel dont le callee a été résolu vers
/// `resolved` (`None` = builtin/inconnu).
///
/// `strict` distingue les deux consommateurs de ce module (voir sa doc
/// d'en-tête) : en mode non strict (E26/diagnostic), un callee non résolu
/// n'est jamais vérifié — comportement inchangé, rater un échappement n'est
/// pas pire qu'avant. En mode strict (libération auto d'un `var`), un callee
/// non résolu (builtin — ex. `Array::push(arr, x)`, qui RETIENT bel et bien
/// son 2ᵉ argument dans `arr`) est traité comme retenant TOUS ses arguments
/// — confirmé nécessaire par reproduction (sans ça : double free/use-after-
/// free sur une valeur poussée dans un tableau puis lue ensuite).
fn check_call_args(
    args: &[Expr], resolved: Option<&str>,
    known: &HashMap<CalleeKey, Vec<bool>>,
    taint: &HashMap<String, HashSet<usize>>, escaped: &mut HashSet<usize>,
    strict: bool,
) {
    match resolved {
        None => {
            if strict {
                for arg in args {
                    escaped.extend(taint_of_expr(arg, taint));
                }
            }
        }
        Some(key) => {
            let param_escapes = known.get(key);
            for (i, arg) in args.iter().enumerate() {
                let escapes_here = match param_escapes {
                    Some(v) => v.get(i).copied().unwrap_or(true),
                    // Clé résolue mais absente de `known` (ne devrait pas
                    // arriver — voir `compute_escaping_params`) : prudent.
                    None => true,
                };
                if escapes_here {
                    escaped.extend(taint_of_expr(arg, taint));
                }
            }
        }
    }
}

/// Collecte tous les identifiants référencés n'importe où dans `block`
/// (utilisé uniquement pour détecter une capture de closure — volontairement
/// grossier : toute mention, lecture ou affectation, compte).
fn collect_ident_refs(block: &Block, out: &mut HashSet<String>) {
    for stmt in &block.stmts {
        collect_ident_refs_stmt(stmt, out);
    }
}

fn collect_ident_refs_stmt(stmt: &Stmt, out: &mut HashSet<String>) {
    match stmt {
        Stmt::Var { value, .. } | Stmt::Const { value, .. } => collect_ident_refs_expr(value, out),
        Stmt::Expr(e) => collect_ident_refs_expr(e, out),
        Stmt::Assign { target, value, .. } => {
            collect_ident_refs_expr(target, out);
            collect_ident_refs_expr(value, out);
        }
        Stmt::Return { value: Some(e), .. } | Stmt::Result { value: Some(e), .. } => collect_ident_refs_expr(e, out),
        Stmt::Return { .. } | Stmt::Result { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => {}
        Stmt::If { condition, then_block, elseif, else_block, .. } => {
            collect_ident_refs_expr(condition, out);
            collect_ident_refs(then_block, out);
            for (c, b) in elseif { collect_ident_refs_expr(c, out); collect_ident_refs(b, out); }
            if let Some(b) = else_block { collect_ident_refs(b, out); }
        }
        Stmt::Switch { subject, cases, default, .. } => {
            collect_ident_refs_expr(subject, out);
            for c in cases { collect_ident_refs(&c.body, out); }
            if let Some(b) = default { collect_ident_refs(b, out); }
        }
        Stmt::While { condition, body, .. } => { collect_ident_refs_expr(condition, out); collect_ident_refs(body, out); }
        Stmt::ForIn { iter, body, .. } => { collect_ident_refs_expr(iter, out); collect_ident_refs(body, out); }
        Stmt::ForMap { iter, body, .. } => { collect_ident_refs_expr(iter, out); collect_ident_refs(body, out); }
        Stmt::Try { body, handlers, .. } => {
            collect_ident_refs(body, out);
            for h in handlers { collect_ident_refs(&h.body, out); }
        }
        Stmt::Raise { value, .. } => collect_ident_refs_expr(value, out),
        Stmt::Emit { value, .. } => collect_ident_refs_expr(value, out),
    }
}

fn collect_ident_refs_expr(expr: &Expr, out: &mut HashSet<String>) {
    match expr {
        Expr::Ident(name, _) => { out.insert(name.clone()); }
        Expr::SelfExpr(_) | Expr::ParentExpr(_) | Expr::Literal(..) | Expr::StaticConst { .. } => {}
        Expr::Field { object, .. } => collect_ident_refs_expr(object, out),
        Expr::Call { callee, args, .. } => {
            collect_ident_refs_expr(callee, out);
            for a in args { collect_ident_refs_expr(a, out); }
        }
        Expr::StaticCall { args, .. } => { for a in args { collect_ident_refs_expr(a, out); } }
        Expr::New { args, .. } => { for a in args { collect_ident_refs_expr(a, out); } }
        Expr::Binary { left, right, .. } => { collect_ident_refs_expr(left, out); collect_ident_refs_expr(right, out); }
        Expr::Unary { operand, .. } => collect_ident_refs_expr(operand, out),
        Expr::Index { object, index, .. } => { collect_ident_refs_expr(object, out); collect_ident_refs_expr(index, out); }
        Expr::Range { start, end, .. } => { collect_ident_refs_expr(start, out); collect_ident_refs_expr(end, out); }
        Expr::Array { elements, .. } => { for e in elements { collect_ident_refs_expr(e, out); } }
        Expr::Map { entries, .. } => { for (k, v) in entries { collect_ident_refs_expr(k, out); collect_ident_refs_expr(v, out); } }
        Expr::Template { parts, .. } => {
            for part in parts { if let TemplatePartExpr::Expr(e) = part { collect_ident_refs_expr(e, out); } }
        }
        Expr::Match { subject, arms, .. } => {
            collect_ident_refs_expr(subject, out);
            for arm in arms { collect_ident_refs_expr(&arm.body, out); }
        }
        Expr::IsCheck { expr, .. } => collect_ident_refs_expr(expr, out),
        Expr::Resolve { expr, .. } => collect_ident_refs_expr(expr, out),
        Expr::IncDec { target, .. } => collect_ident_refs_expr(target, out),
        // Nameless imbriquée : ne descend pas — ses propres captures sont
        // son affaire, pas celle du corps englobant.
        Expr::Nameless { .. } => {}
    }
}
