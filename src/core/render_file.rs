// ─────────────────────────────────────────────────────────────────────────────
// Désucrage de HTML::renderFile / HTML::renderFileCached
//
// `HTML::renderFile(path)` et `HTML::renderFileCached(path, cache_key)` ne sont
// pas de vraies méthodes runtime : ce sont du sucre syntaxique résolu à la
// compilation. Le fichier pointé par `path` est lu pendant la compilation et
// son contenu est traité EXACTEMENT comme un littéral template `` `...` `` —
// les `${...}` qu'il contient sont découpés et re-parsés en vraies
// sous-expressions AST, au même titre qu'un backtick écrit en dur dans le
// `.oc`. L'appel est ensuite réécrit en `HTML::render(<template>)` /
// `HTML::renderCached(<template>, cache_key)`.
//
// Conséquences (voulues) :
//   - Les variables référencées via `${x}` dans le fichier sont visitées par
//     le typecheck comme n'importe quelle expression → plus de faux warning
//     "unused" sur `x`, et interpolation réelle à l'exécution.
//   - Le contenu du fichier est figé dans le binaire au moment de la
//     compilation (relire le fichier plus tard ne change rien sans
//     recompiler).
//   - `path` peut être un littéral (`'templates/home.html'`) ou une variable,
//     à condition que sa valeur soit déterminable statiquement (assignée
//     directement à un littéral, sans passer par une branche ambiguë). Sinon
//     : erreur de compilation.
//   - Résolution du chemin : absolu tel quel, sinon relatif au répertoire
//     courant du COMPILATEUR au moment du build (comme `File::read` le fait
//     déjà à l'exécution, mais ici c'est fait pendant `ocara build`).
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::parsing::ast::{
    BinOp, Block, ClassMember, Expr, Literal, Program, Stmt, TemplatePartExpr,
};
use crate::parsing::token::{Span, TemplatePart};

/// Valeurs de variables locales dont on sait, à ce point du programme, qu'elles
/// contiennent tel littéral string (sans ambiguïté de branche).
type ConstMap = HashMap<String, String>;

/// Erreur de désucrage : span (pour le message GCC-like) + message.
pub type DesugarError = (Span, String);

/// Point d'entrée : parcourt tout le programme et réécrit chaque appel
/// `HTML::renderFile` / `HTML::renderFileCached` en `HTML::render` /
/// `HTML::renderCached` avec un vrai template compilé depuis le fichier.
pub fn desugar_render_file(program: &mut Program) -> Result<(), DesugarError> {
    // Les `const` de niveau module sont visibles partout : on les pré-charge
    // dans la map de départ de chaque fonction/méthode, sinon
    // `const DIR:string = "templates/"` suivi de `HTML::renderFile(DIR + "x")`
    // dans une autre fonction ne serait jamais résolu.
    let globals = collect_global_consts(program);

    for f in &mut program.functions {
        let mut map = globals.clone();
        desugar_block(&mut f.body, &mut map)?;
    }
    for c in &mut program.classes {
        desugar_members(&mut c.members, &globals)?;
    }
    for m in &mut program.modules {
        desugar_members(&mut m.members, &globals)?;
    }
    for g in &mut program.generics {
        desugar_members(&mut g.members, &globals)?;
    }
    for c in &mut program.consts {
        let mut map = globals.clone();
        desugar_expr(&mut c.value, &mut map)?;
    }
    Ok(())
}

/// Pré-scan des `const` de niveau module dont la valeur est un littéral
/// string direct — seul cas qu'on sait résoudre sans dépendre de l'ordre de
/// déclaration.
fn collect_global_consts(program: &Program) -> ConstMap {
    let mut map = ConstMap::new();
    for c in &program.consts {
        if let Expr::Literal(Literal::String(s), _) = &c.value {
            map.insert(c.name.clone(), s.clone());
        }
    }
    map
}

fn desugar_members(members: &mut [ClassMember], globals: &ConstMap) -> Result<(), DesugarError> {
    for member in members {
        match member {
            ClassMember::Method { decl, .. } => {
                let mut map = globals.clone();
                desugar_block(&mut decl.body, &mut map)?;
            }
            ClassMember::Constructor { body, .. } => {
                let mut map = globals.clone();
                desugar_block(body, &mut map)?;
            }
            ClassMember::Const { value, .. } => {
                let mut map = globals.clone();
                desugar_expr(value, &mut map)?;
            }
            ClassMember::Field { .. } => {}
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Parcours statements / expressions, avec suivi des constantes locales
// ─────────────────────────────────────────────────────────────────────────────

fn desugar_block(block: &mut Block, map: &mut ConstMap) -> Result<(), DesugarError> {
    for stmt in &mut block.stmts {
        desugar_stmt(stmt, map)?;
    }
    Ok(())
}

/// Traite un sous-bloc dans une branche (if/else, boucle, handler `on`...) :
/// part d'une copie de la map courante, et retourne cette copie une fois le
/// bloc parcouru — l'appelant décide ensuite quoi en faire (fusion, invalidation).
fn desugar_branch(block: &mut Block, map: &ConstMap) -> Result<ConstMap, DesugarError> {
    let mut branch_map = map.clone();
    desugar_block(block, &mut branch_map)?;
    Ok(branch_map)
}

/// Après une construction à embranchements (if/switch/boucle/try), une
/// variable modifiée dans N'IMPORTE QUELLE branche redevient "inconnue" dans
/// la map extérieure : on ne peut pas savoir quelle branche s'est exécutée.
fn invalidate_touched(map: &mut ConstMap, branch_maps: &[ConstMap]) {
    let mut touched: HashSet<String> = HashSet::new();
    for bm in branch_maps {
        for (k, v) in bm.iter() {
            if map.get(k) != Some(v) {
                touched.insert(k.clone());
            }
        }
        for k in map.keys() {
            if !bm.contains_key(k) {
                touched.insert(k.clone());
            }
        }
    }
    for k in touched {
        map.remove(&k);
    }
}

/// Enregistre (ou invalide) le fait qu'une variable vaut tel littéral string,
/// suite à une déclaration ou une réaffectation.
fn update_const(map: &mut ConstMap, name: &str, value: &Expr) {
    if let Expr::Literal(Literal::String(s), _) = value {
        map.insert(name.to_string(), s.clone());
    } else {
        map.remove(name);
    }
}

fn desugar_stmt(stmt: &mut Stmt, map: &mut ConstMap) -> Result<(), DesugarError> {
    match stmt {
        Stmt::Var { name, value, .. } => {
            desugar_expr(value, map)?;
            update_const(map, name, value);
        }
        Stmt::Const { name, value, .. } => {
            desugar_expr(value, map)?;
            update_const(map, name, value);
        }
        Stmt::Expr(expr) => {
            desugar_expr(expr, map)?;
        }
        Stmt::If { condition, then_block, elseif, else_block, .. } => {
            desugar_expr(condition, map)?;
            let mut branch_maps = vec![desugar_branch(then_block, map)?];
            for (cond, blk) in elseif.iter_mut() {
                desugar_expr(cond, map)?;
                branch_maps.push(desugar_branch(blk, map)?);
            }
            match else_block {
                Some(blk) => branch_maps.push(desugar_branch(blk, map)?),
                None => branch_maps.push(map.clone()), // pas de else = "rien ne change" possible
            }
            invalidate_touched(map, &branch_maps);
        }
        Stmt::Switch { subject, cases, default, .. } => {
            desugar_expr(subject, map)?;
            let mut branch_maps = Vec::new();
            for case in cases.iter_mut() {
                branch_maps.push(desugar_branch(&mut case.body, map)?);
            }
            match default {
                Some(blk) => branch_maps.push(desugar_branch(blk, map)?),
                None => branch_maps.push(map.clone()),
            }
            invalidate_touched(map, &branch_maps);
        }
        Stmt::While { condition, body, .. } => {
            desugar_expr(condition, map)?;
            let branch_map = desugar_branch(body, map)?;
            invalidate_touched(map, &[branch_map]);
        }
        Stmt::ForIn { iter, body, .. } => {
            desugar_expr(iter, map)?;
            let branch_map = desugar_branch(body, map)?;
            invalidate_touched(map, &[branch_map]);
        }
        Stmt::ForMap { iter, body, .. } => {
            desugar_expr(iter, map)?;
            let branch_map = desugar_branch(body, map)?;
            invalidate_touched(map, &[branch_map]);
        }
        Stmt::Return { value, .. } | Stmt::Result { value, .. } => {
            if let Some(v) = value {
                desugar_expr(v, map)?;
            }
        }
        Stmt::Break { .. } | Stmt::Continue { .. } => {}
        Stmt::Try { body, handlers, .. } => {
            let mut branch_maps = vec![desugar_branch(body, map)?];
            for on in handlers.iter_mut() {
                branch_maps.push(desugar_branch(&mut on.body, map)?);
            }
            invalidate_touched(map, &branch_maps);
        }
        Stmt::Raise { value, .. } => {
            desugar_expr(value, map)?;
        }
        Stmt::Assign { target, value, .. } => {
            desugar_expr(value, map)?;
            if let Expr::Ident(name, _) = target {
                update_const(map, name, value);
            } else {
                desugar_expr(target, map)?;
            }
        }
    }
    Ok(())
}

fn desugar_expr(expr: &mut Expr, map: &mut ConstMap) -> Result<(), DesugarError> {
    match expr {
        Expr::Literal(..) | Expr::Ident(..) | Expr::SelfExpr(..)
        | Expr::ParentExpr(..) | Expr::StaticConst { .. } => {}

        Expr::Field { object, .. } => desugar_expr(object, map)?,

        Expr::Call { callee, args, .. } => {
            desugar_expr(callee, map)?;
            for a in args.iter_mut() {
                desugar_expr(a, map)?;
            }
        }

        Expr::StaticCall { class, method, args, span } => {
            for a in args.iter_mut() {
                desugar_expr(a, map)?;
            }
            if class == "HTML" && (method == "renderFile" || method == "renderFileCached") {
                desugar_render_call(class, method, args, span, map)?;
            }
        }

        Expr::New { args, .. } => {
            for a in args.iter_mut() {
                desugar_expr(a, map)?;
            }
        }

        Expr::Binary { left, right, .. } => {
            desugar_expr(left, map)?;
            desugar_expr(right, map)?;
        }

        Expr::Unary { operand, .. } => desugar_expr(operand, map)?,

        Expr::Array { elements, .. } => {
            for e in elements.iter_mut() {
                desugar_expr(e, map)?;
            }
        }

        Expr::Map { entries, .. } => {
            for (k, v) in entries.iter_mut() {
                desugar_expr(k, map)?;
                desugar_expr(v, map)?;
            }
        }

        Expr::Template { parts, .. } => {
            for part in parts.iter_mut() {
                if let TemplatePartExpr::Expr(e) = part {
                    desugar_expr(e, map)?;
                }
            }
        }

        Expr::Index { object, index, .. } => {
            desugar_expr(object, map)?;
            desugar_expr(index, map)?;
        }

        Expr::Range { start, end, .. } => {
            desugar_expr(start, map)?;
            desugar_expr(end, map)?;
        }

        Expr::Match { subject, arms, .. } => {
            desugar_expr(subject, map)?;
            for arm in arms.iter_mut() {
                desugar_expr(&mut arm.body, map)?;
            }
        }

        Expr::Nameless { body, .. } => {
            // Nouvelle portée de fonction : on part d'une copie (pour pouvoir
            // lire les chemins déjà connus), mais rien n'en ressort — les
            // affectations dans la closure ne doivent pas "fuiter" dehors.
            let mut inner = map.clone();
            desugar_block(body, &mut inner)?;
        }

        Expr::Resolve { expr: e, .. } => desugar_expr(e, map)?,
        Expr::IsCheck { expr: e, .. } => desugar_expr(e, map)?,
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// HTML::renderFile / HTML::renderFileCached → HTML::render / HTML::renderCached
// ─────────────────────────────────────────────────────────────────────────────

fn desugar_render_call(
    class:  &mut String,
    method: &mut String,
    args:   &mut Vec<Expr>,
    span:   &Span,
    map:    &ConstMap,
) -> Result<(), DesugarError> {
    let is_cached = method == "renderFileCached";
    let expected_args = if is_cached { 2 } else { 1 };
    if args.len() != expected_args {
        return Err((span.clone(), format!(
            "HTML::{} expects {} argument(s), found {}",
            method, expected_args, args.len(),
        )));
    }

    let raw_path = resolve_const_string(&args[0], map).ok_or_else(|| (span.clone(), format!(
        "HTML::{}: path must be a compile-time constant — a string literal, \
         or a variable whose value is statically known (assigned directly from a \
         literal, with no ambiguous branch in between)",
        method,
    )))?;

    let resolved_path = resolve_template_path(&raw_path);
    let template_expr = build_template_from_file(&resolved_path, span)
        .map_err(|e| (span.clone(), format!("HTML::{}: {}", method, e)))?;

    *method = if is_cached { "renderCached".to_string() } else { "render".to_string() };
    let _ = class; // déjà "HTML", inchangé
    if is_cached {
        let cache_key = args[1].clone();
        *args = vec![template_expr, cache_key];
    } else {
        *args = vec![template_expr];
    }
    Ok(())
}

/// Tente de réduire une expression à une string connue à la compilation :
/// littéral direct, variable tracée dans `map`, ou concaténation (`+`) de
/// deux sous-expressions elles-mêmes résolvables.
fn resolve_const_string(expr: &Expr, map: &ConstMap) -> Option<String> {
    match expr {
        Expr::Literal(Literal::String(s), _) => Some(s.clone()),
        Expr::Ident(name, _) => map.get(name).cloned(),
        Expr::Binary { op: BinOp::Add, left, right, .. } => {
            let l = resolve_const_string(left, map)?;
            let r = resolve_const_string(right, map)?;
            Some(l + &r)
        }
        _ => None,
    }
}

/// Absolu tel quel, sinon relatif au répertoire courant du compilateur.
fn resolve_template_path(raw: &str) -> PathBuf {
    let p = Path::new(raw);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(p)
    }
}

/// Lit le fichier et construit un `Expr::Template` identique à ce que
/// produirait un littéral `` `...` `` contenant le même texte.
fn build_template_from_file(path: &Path, span: &Span) -> Result<Expr, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read template file '{}': {}", path.display(), e))?;

    let raw_parts = split_file_template(&content)
        .map_err(|e| format!("{} (in '{}')", e, path.display()))?;

    let mut parts = Vec::with_capacity(raw_parts.len());
    for part in raw_parts {
        match part {
            TemplatePart::Literal(s) => parts.push(TemplatePartExpr::Literal(s)),
            TemplatePart::ExprSrc(src) => {
                let expr = parse_expr_src(&src)
                    .map_err(|e| format!("{} (in '{}')", e, path.display()))?;
                parts.push(TemplatePartExpr::Expr(Box::new(expr)));
            }
        }
    }
    Ok(Expr::Template { parts, span: span.clone() })
}

/// Découpe un contenu de fichier brut en `Literal` / `ExprSrc` sur les
/// `${...}` (accolades imbriquées comptées). Contrairement à un littéral
/// backtick Ocara, AUCUNE séquence d'échappement n'est traitée : le fichier
/// n'est pas du code Ocara, ses `\` (JS, regex, CSS...) doivent rester intacts.
fn split_file_template(content: &str) -> Result<Vec<TemplatePart>, String> {
    let chars: Vec<char> = content.chars().collect();
    let mut i = 0usize;
    let mut parts = Vec::new();
    let mut literal = String::new();

    while i < chars.len() {
        if chars[i] == '$' && chars.get(i + 1) == Some(&'{') {
            if !literal.is_empty() {
                parts.push(TemplatePart::Literal(std::mem::take(&mut literal)));
            }
            i += 2; // '$' '{'
            let mut expr_src = String::new();
            let mut depth: usize = 1;
            loop {
                match chars.get(i) {
                    None => return Err("unterminated `${...}` interpolation".to_string()),
                    Some('{') => {
                        depth += 1;
                        expr_src.push('{');
                        i += 1;
                    }
                    Some('}') => {
                        depth -= 1;
                        i += 1;
                        if depth == 0 {
                            break;
                        }
                        expr_src.push('}');
                    }
                    Some(c) => {
                        expr_src.push(*c);
                        i += 1;
                    }
                }
            }
            parts.push(TemplatePart::ExprSrc(expr_src));
        } else {
            literal.push(chars[i]);
            i += 1;
        }
    }
    if !literal.is_empty() {
        parts.push(TemplatePart::Literal(literal));
    }
    Ok(parts)
}

/// Re-lexe puis re-parse le source brut d'une interpolation `${...}` en une
/// vraie expression AST — identique à ce que fait le parser pour un littéral
/// template classique (voir `parser.d/expressions.rs`, cas `LitTemplate`).
fn parse_expr_src(src: &str) -> Result<Expr, String> {
    let mut sub_lex = crate::parsing::lexer::Lexer::new(src);
    let sub_tokens = sub_lex.tokenize()
        .map_err(|e| format!("error in interpolation `${{{}}}`: {}", src, e))?;
    let mut sub_parser = crate::parsing::parser::Parser::new(sub_tokens);
    sub_parser.parse_expr()
        .map_err(|e| format!("error in interpolation `${{{}}}`: {}", src, e.message))
}
