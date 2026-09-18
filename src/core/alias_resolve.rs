/// Résolution des alias d'import (`import X as Y`) pour des symboles
/// utilisateur (classe/generic/interface/module/fonction) — voir
/// docs/roadmap.d/langage-alias-classe-utilisateur-heritage-casse.md.
///
/// Contrairement à un builtin `ocara.*` (fabriqué à la demande par
/// `register_import`, sans référence préexistante ailleurs dans le
/// programme), un symbole UTILISATEUR peut être référencé par son VRAI NOM
/// dans D'AUTRES fichiers (type de paramètre, `extends`, `implements`,
/// `on e is X`...). L'ancien comportement (`src/main.rs`) RENOMMAIT le
/// symbole importé vers son alias avant de le fusionner dans le programme —
/// ce qui cassait silencieusement toute résolution passant par le vrai nom
/// (`class_parents`/`class_field_types`/résolution de méthode héritée ne
/// connaissent alors plus que le nom final) dès qu'un AUTRE fichier
/// référençait la même classe sans jamais voir l'alias. Confirmé par
/// reproduction : `import configs.Server as HTTP` empêchait TOUTE route
/// d'être enregistrée dans `examples/advanced/tauri_httpserver` (la méthode
/// `route()`, héritée de `HTTPServer`, ne se résolvait plus pour un
/// paramètre `server:Server` déclaré dans un autre fichier).
///
/// Corrigé en NE RENOMMANT JAMAIS le symbole lui-même : à la place, cette
/// passe réécrit CHAQUE occurrence de l'alias vers le vrai nom, dans le SEUL
/// fichier qui a écrit `import ... as alias` (un alias n'est visible que
/// dans le fichier qui l'a déclaré — jamais transmis à un autre fichier).
/// Appelée une fois par fichier chargé (fichier principal ET chaque fichier
/// importé, voir les deux points d'appel dans `src/main.rs`), AVANT toute
/// fusion dans le programme global : après cette passe, l'alias n'existe
/// plus nulle part dans l'AST de ce fichier, aucune coordination
/// supplémentaire n'est nécessaire en aval (sema/lowering ne voient jamais
/// l'alias).
use std::collections::HashMap;
use crate::parsing::ast::*;

/// Construit la table alias → nom réel à partir des SEULES déclarations
/// d'import d'un fichier donné (`ImportDecl.alias`) — ne jamais mélanger les
/// imports de deux fichiers différents ici, un alias n'étant visible que
/// dans le fichier qui l'a écrit.
pub fn compute_aliases(imports: &[ImportDecl]) -> HashMap<String, String> {
    imports.iter()
        .filter_map(|imp| {
            let alias = imp.alias.as_ref()?;
            let real = imp.path.last()?;
            if alias == real { return None; }
            Some((alias.clone(), real.clone()))
        })
        .collect()
}

/// Réécrit chaque occurrence d'un alias connu vers son nom réel, dans tout
/// le contenu du programme donné (classes, generics, interfaces, modules,
/// fonctions, constantes, blocs runtime) — voir la doc de module.
pub fn resolve_aliases(program: &mut Program, aliases: &HashMap<String, String>) {
    if aliases.is_empty() {
        return;
    }

    for c in &mut program.classes {
        if let Some(ext) = &mut c.extends {
            resolve_name(aliases, ext);
        }
        for m in &mut c.modules {
            resolve_name(aliases, m);
        }
        for i in &mut c.implements {
            resolve_name(aliases, i);
        }
        for member in &mut c.members {
            resolve_class_member(aliases, member);
        }
    }
    for g in &mut program.generics {
        if let Some(ext) = &mut g.extends {
            resolve_name(aliases, ext);
        }
        for a in &mut g.extends_args {
            resolve_type(aliases, a);
        }
        for m in &mut g.modules {
            resolve_name(aliases, m);
        }
        for i in &mut g.implements {
            resolve_name(aliases, i);
        }
        for member in &mut g.members {
            resolve_class_member(aliases, member);
        }
    }
    for m in &mut program.modules {
        for member in &mut m.members {
            resolve_class_member(aliases, member);
        }
    }
    for i in &mut program.interfaces {
        for method in &mut i.methods {
            for p in &mut method.params {
                resolve_type(aliases, &mut p.ty);
                if let Some(d) = &mut p.default_value {
                    resolve_expr(aliases, d);
                }
            }
            resolve_type(aliases, &mut method.ret_ty);
        }
    }
    for f in &mut program.functions {
        resolve_func(aliases, f);
    }
    for c in &mut program.consts {
        resolve_type(aliases, &mut c.ty);
        resolve_expr(aliases, &mut c.value);
    }
    for rb in &mut program.runtime_blocks {
        for s in &mut rb.statements {
            resolve_stmt(aliases, s);
        }
    }
}

fn resolve_name(aliases: &HashMap<String, String>, name: &mut String) {
    if let Some(real) = aliases.get(name.as_str()) {
        *name = real.clone();
    }
}

fn resolve_type(aliases: &HashMap<String, String>, ty: &mut Type) {
    match ty {
        Type::Named(n) => resolve_name(aliases, n),
        Type::Array(inner) => resolve_type(aliases, inner),
        Type::Map(k, v) => {
            resolve_type(aliases, k);
            resolve_type(aliases, v);
        }
        Type::Message(inner) => resolve_type(aliases, inner),
        Type::Generic { name, args } => {
            resolve_name(aliases, name);
            for a in args {
                resolve_type(aliases, a);
            }
        }
        Type::Union(types) => {
            for t in types {
                resolve_type(aliases, t);
            }
        }
        Type::Function { ret_ty, param_tys } => {
            resolve_type(aliases, ret_ty);
            for p in param_tys {
                resolve_type(aliases, p);
            }
        }
        // `Qualified` n'est jamais produit par ce parser pour un chemin
        // résolu par alias (voir les usages de `Type::Named`/`Qualified`) ;
        // les primitifs n'ont rien à résoudre.
        Type::Qualified(_) | Type::Int | Type::Float | Type::String | Type::Bool
        | Type::Mixed | Type::Void | Type::Null => {}
    }
}

fn resolve_expr(aliases: &HashMap<String, String>, expr: &mut Expr) {
    match expr {
        Expr::Literal(..) | Expr::Ident(..) | Expr::SelfExpr(_) | Expr::ParentExpr(_) => {}
        Expr::Field { object, .. } => resolve_expr(aliases, object),
        Expr::Call { callee, args, .. } => {
            resolve_expr(aliases, callee);
            for a in args {
                resolve_expr(aliases, a);
            }
        }
        Expr::StaticCall { class, args, .. } => {
            resolve_name(aliases, class);
            for a in args {
                resolve_expr(aliases, a);
            }
        }
        Expr::StaticConst { class, .. } => resolve_name(aliases, class),
        Expr::New { class, type_args, args, .. } => {
            resolve_name(aliases, class);
            for t in type_args {
                resolve_type(aliases, t);
            }
            for a in args {
                resolve_expr(aliases, a);
            }
        }
        Expr::Binary { left, right, .. } => {
            resolve_expr(aliases, left);
            resolve_expr(aliases, right);
        }
        Expr::Unary { operand, .. } => resolve_expr(aliases, operand),
        Expr::Array { elements, .. } => {
            for e in elements {
                resolve_expr(aliases, e);
            }
        }
        Expr::Map { entries, .. } => {
            for (k, v) in entries {
                resolve_expr(aliases, k);
                resolve_expr(aliases, v);
            }
        }
        Expr::Template { parts, .. } => {
            for p in parts {
                if let TemplatePartExpr::Expr(e) = p {
                    resolve_expr(aliases, e);
                }
            }
        }
        Expr::Index { object, index, .. } => {
            resolve_expr(aliases, object);
            resolve_expr(aliases, index);
        }
        Expr::Range { start, end, .. } => {
            resolve_expr(aliases, start);
            resolve_expr(aliases, end);
        }
        Expr::Match { subject, arms, .. } => {
            resolve_expr(aliases, subject);
            for arm in arms {
                if let Some(MatchPattern::IsType(ty)) = &mut arm.pattern {
                    resolve_type(aliases, ty);
                }
                resolve_expr(aliases, &mut arm.body);
            }
        }
        Expr::Nameless { params, ret_ty, body, .. } => {
            for p in params {
                resolve_type(aliases, &mut p.ty);
                if let Some(d) = &mut p.default_value {
                    resolve_expr(aliases, d);
                }
            }
            if let Some(rt) = ret_ty {
                resolve_type(aliases, rt);
            }
            resolve_block(aliases, body);
        }
        Expr::Resolve { expr, .. } => resolve_expr(aliases, expr),
        Expr::IsCheck { expr, ty, .. } => {
            resolve_expr(aliases, expr);
            resolve_type(aliases, ty);
        }
        Expr::IncDec { target, .. } => resolve_expr(aliases, target),
    }
}

fn resolve_stmt(aliases: &HashMap<String, String>, stmt: &mut Stmt) {
    match stmt {
        Stmt::Var { ty, value, .. } => {
            resolve_type(aliases, ty);
            resolve_expr(aliases, value);
        }
        Stmt::Const { ty, value, .. } => {
            resolve_type(aliases, ty);
            resolve_expr(aliases, value);
        }
        Stmt::Expr(e) => resolve_expr(aliases, e),
        Stmt::If { condition, then_block, elseif, else_block, .. } => {
            resolve_expr(aliases, condition);
            resolve_block(aliases, then_block);
            for (cond, blk) in elseif {
                resolve_expr(aliases, cond);
                resolve_block(aliases, blk);
            }
            if let Some(b) = else_block {
                resolve_block(aliases, b);
            }
        }
        Stmt::Switch { subject, cases, default, .. } => {
            resolve_expr(aliases, subject);
            for c in cases {
                resolve_block(aliases, &mut c.body);
            }
            if let Some(d) = default {
                resolve_block(aliases, d);
            }
        }
        Stmt::While { condition, body, .. } => {
            resolve_expr(aliases, condition);
            resolve_block(aliases, body);
        }
        Stmt::ForIn { iter, body, .. } => {
            resolve_expr(aliases, iter);
            resolve_block(aliases, body);
        }
        Stmt::ForMap { iter, body, .. } => {
            resolve_expr(aliases, iter);
            resolve_block(aliases, body);
        }
        Stmt::Return { value, .. } => {
            if let Some(v) = value {
                resolve_expr(aliases, v);
            }
        }
        Stmt::Result { value, .. } => {
            if let Some(v) = value {
                resolve_expr(aliases, v);
            }
        }
        Stmt::Break { .. } | Stmt::Continue { .. } => {}
        Stmt::Try { body, handlers, .. } => {
            resolve_block(aliases, body);
            for h in handlers {
                if let Some(cf) = &mut h.class_filter {
                    resolve_name(aliases, cf);
                }
                resolve_block(aliases, &mut h.body);
            }
        }
        Stmt::Raise { value, .. } => resolve_expr(aliases, value),
        Stmt::Emit { value, .. } => resolve_expr(aliases, value),
        Stmt::Assign { target, value, .. } => {
            resolve_expr(aliases, target);
            resolve_expr(aliases, value);
        }
    }
}

fn resolve_block(aliases: &HashMap<String, String>, block: &mut Block) {
    for s in &mut block.stmts {
        resolve_stmt(aliases, s);
    }
}

fn resolve_class_member(aliases: &HashMap<String, String>, member: &mut ClassMember) {
    match member {
        ClassMember::Field { ty, .. } => resolve_type(aliases, ty),
        ClassMember::Const { ty, value, .. } => {
            resolve_type(aliases, ty);
            resolve_expr(aliases, value);
        }
        ClassMember::Method { decl, .. } => resolve_func(aliases, decl),
        ClassMember::Constructor { params, body, .. } => {
            for p in params {
                resolve_type(aliases, &mut p.ty);
                if let Some(d) = &mut p.default_value {
                    resolve_expr(aliases, d);
                }
            }
            resolve_block(aliases, body);
        }
    }
}

fn resolve_func(aliases: &HashMap<String, String>, f: &mut FuncDecl) {
    for p in &mut f.params {
        resolve_type(aliases, &mut p.ty);
        if let Some(d) = &mut p.default_value {
            resolve_expr(aliases, d);
        }
    }
    resolve_type(aliases, &mut f.ret_ty);
    resolve_block(aliases, &mut f.body);
}
