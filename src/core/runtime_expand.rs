use crate::core::diagnostics::Diagnostic;
use crate::parsing::{ast::{self, Stmt}, lexer::Lexer, parser::Parser, token};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Helpers pour extraire les numéros de ligne des statements (pour contexte runtime)
// ─────────────────────────────────────────────────────────────────────────────

pub fn get_stmt_start_line(stmt: &Stmt) -> usize {
    get_stmt_span(stmt).line
}

pub fn get_stmt_span(stmt: &Stmt) -> &token::Span {
    match stmt {
        Stmt::Expr(e) => e.span(),
        Stmt::Var { span, .. } | Stmt::Const { span, .. } | Stmt::Assign { span, .. }
        | Stmt::If { span, .. } | Stmt::While { span, .. } | Stmt::ForIn { span, .. }
        | Stmt::ForMap { span, .. } | Stmt::Switch { span, .. } | Stmt::Return { span, .. }
        | Stmt::Result { span, .. } | Stmt::Break { span, .. } | Stmt::Continue { span, .. }
        | Stmt::Try { span, .. } | Stmt::Raise { span, .. } | Stmt::Emit { span, .. } => span,
    }
}

pub fn get_stmt_end_line(stmt: &Stmt) -> usize {
    match stmt {
        Stmt::Var { span, .. } | Stmt::Const { span, .. } | Stmt::Assign { span, .. }
        | Stmt::Return { span, .. } | Stmt::Result { span, .. } | Stmt::Break { span, .. }
        | Stmt::Continue { span, .. } | Stmt::Raise { span, .. } | Stmt::Emit { span, .. } => span.line,
        
        Stmt::Expr(e) => e.span().line,
        
        Stmt::If { else_block, then_block, elseif, span, .. } => {
            if let Some(eb) = else_block {
                if let Some(last) = eb.stmts.last() {
                    return get_stmt_end_line(last);
                }
            }
            for (_, block) in elseif.iter().rev() {
                if let Some(last) = block.stmts.last() {
                    return get_stmt_end_line(last);
                }
            }
            if let Some(last) = then_block.stmts.last() {
                return get_stmt_end_line(last);
            }
            span.line
        }
        
        Stmt::While { body, span, .. } | Stmt::ForIn { body, span, .. } | Stmt::ForMap { body, span, .. } => {
            body.stmts.last().map_or(span.line, |last| get_stmt_end_line(last))
        }
        
        Stmt::Switch { default, cases, span, .. } => {
            if let Some(def) = default {
                if let Some(last) = def.stmts.last() {
                    return get_stmt_end_line(last);
                }
            }
            for case in cases.iter().rev() {
                if let Some(last) = case.body.stmts.last() {
                    return get_stmt_end_line(last);
                }
            }
            span.line
        }
        
        Stmt::Try { handlers, body, span, .. } => {
            for handler in handlers.iter().rev() {
                if let Some(last) = handler.body.stmts.last() {
                    return get_stmt_end_line(last);
                }
            }
            body.stmts.last().map_or(span.line, |last| get_stmt_end_line(last))
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Expansion des runtime imports
// ─────────────────────────────────────────────────────────────────────────────

pub fn expand_runtime_imports(program: &mut ast::Program, source_dir: &std::path::Path, input_file: &std::path::Path) -> Result<(), Diagnostic> {
    // Map: RuntimeBlockKind -> Vec<statements>
    let mut blocks_map: HashMap<ast::RuntimeBlockKind, Vec<ast::Stmt>> = HashMap::new();
    
    // Ajouter les blocs existants dans le programme
    for block in &program.runtime_blocks {
        blocks_map.entry(block.kind)
            .or_insert_with(Vec::new)
            .extend(block.statements.clone());
    }
    
    // Traiter chaque import runtime
    for rt_import in &program.runtime_imports {
        // Résoudre le chemin du fichier runtime
        let runtime_file = resolve_runtime_file(source_dir, &rt_import.path);
        
        match runtime_file {
            Some(path) => {
                // Charger et parser le fichier runtime
                match load_runtime_file(&path, rt_import, input_file) {
                    Ok(blocks) => {
                        // Si kind est spécifié (runtime X is init), ajouter tous les statements au bloc spécifié
                        // Sinon (runtime X), importer tous les blocs déclarés
                        if let Some(target_kind) = rt_import.kind {
                            // Mode: le contenu du fichier devient le bloc spécifié
                            for block in blocks {
                                blocks_map.entry(target_kind)
                                    .or_insert_with(Vec::new)
                                    .extend(block.statements);
                            }
                        } else {
                            // Mode: importer tous les blocs déclarés du fichier
                            for block in blocks {
                                blocks_map.entry(block.kind)
                                    .or_insert_with(Vec::new)
                                    .extend(block.statements);
                            }
                        }
                    }
                    Err(e) => {
                        return Err(Diagnostic::error(input_file, rt_import.span.line, rt_import.span.col, e));
                    }
                }
            }
            None => {
                let path_str = rt_import.path.join(".");
                return Err(Diagnostic::error(
                    input_file,
                    rt_import.span.line,
                    rt_import.span.col,
                    format!("runtime file not found: `{}` (tried .runtime.oc, .run.oc, .rt.oc, .oc)", path_str),
                ));
            }
        }
    }
    
    // Reconstruire la liste des blocs runtime et transformer les return
    program.runtime_blocks = blocks_map.into_iter()
        .map(|(kind, statements)| {
            // NE PLUS transformer les returns ici !
            // La transformation est maintenant faite au niveau du lowering IR
            // (voir src/lower/stmt.d/statements.rs)
            ast::RuntimeBlock {
                kind,
                statements,
                span: token::Span::new(0, 0), // Span fusionné
            }
        })
        .collect();
    Ok(())
}

/// Résout le chemin d'un fichier runtime : essaie .runtime.oc, .run.oc, .rt.oc, .oc
pub fn resolve_runtime_file(source_dir: &std::path::Path, path: &[String]) -> Option<std::path::PathBuf> {
    let path_str = path.join("/");
    let extensions = ["runtime.oc", "run.oc", "rt.oc", "oc"];
    
    for ext in &extensions {
        let file_path = source_dir.join(format!("{}.{}", path_str, ext));
        if file_path.exists() {
            return Some(file_path);
        }
    }
    
    None
}

/// Charge et parse un fichier runtime, retourne tous les blocs définis.
/// `runtime X is <bloc>` : le contenu du fichier devient le bloc ; il est
/// enveloppé au niveau des jetons, donc chaque instruction garde sa ligne et
/// sa colonne dans le fichier, auquel elle est rattachée (diagnostics).
fn load_runtime_file(
    file_path: &std::path::Path,
    rt_import: &ast::RuntimeImport,
    _main_file: &std::path::Path,
) -> Result<Vec<ast::RuntimeBlock>, String> {
    let source = crate::core::source::read(file_path)
        .map_err(|e| format!("cannot read '{}': {}", file_path.display(), e))?;
    let tokens = Lexer::new(&source).tokenize()
        .map_err(|e| format!("lexing error in '{}': {:?}", file_path.display(), e))?;
    let tokens = match rt_import.kind {
        Some(kind) => wrap_in_block(tokens, kind),
        None => tokens,
    };
    let mut runtime_program = Parser::new(tokens).parse_program()
        .map_err(|e| format!("parse error in '{}' at {}:{}: {}",
            file_path.display(), e.span.line, e.span.col, e.message))?;
    update_program_spans_with_file(&mut runtime_program, &file_path.to_string_lossy());

    if let Some(target_kind) = rt_import.kind {
        return runtime_program.runtime_blocks.into_iter()
            .find(|b| b.kind == target_kind)
            .map(|b| vec![b])
            .ok_or_else(|| format!("internal error: failed to parse wrapped runtime block '{}'", target_kind.as_str()));
    }
    Ok(runtime_program.runtime_blocks)
}

/// `<imports> <corps>` → `<imports> <bloc> { <corps> }` : jetons du bloc
/// insérés après les lignes `import`/`namespace` de tête, avec la position
/// du jeton qui les suit (aucune position du fichier ne change).
fn wrap_in_block(mut tokens: Vec<token::Token>, kind: ast::RuntimeBlockKind) -> Vec<token::Token> {
    use token::{Token, TokenKind};
    let mut start = 0;
    while matches!(tokens[start].kind, TokenKind::Import | TokenKind::Namespace) {
        let line = tokens[start].span.line;
        while tokens[start].kind != TokenKind::Eof && tokens[start].span.line == line {
            start += 1;
        }
    }
    let keyword = match kind {
        ast::RuntimeBlockKind::Init => TokenKind::Init,
        ast::RuntimeBlockKind::Main => TokenKind::Main,
        ast::RuntimeBlockKind::Error => TokenKind::Error,
        ast::RuntimeBlockKind::Success => TokenKind::Success,
        ast::RuntimeBlockKind::Exit => TokenKind::Exit,
    };
    let at = tokens[start].span.clone();
    let end = tokens.len() - 1;
    let eof = tokens[end].span.clone();
    tokens.insert(end, Token::new(TokenKind::RBrace, "}", eof));
    tokens.insert(start, Token::new(TokenKind::LBrace, "{", at.clone()));
    tokens.insert(start, Token::new(keyword, kind.as_str(), at));
    tokens
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper pour mettre à jour tous les Spans d'un Program avec le nom du fichier
// ─────────────────────────────────────────────────────────────────────────────

pub fn update_program_spans_with_file(program: &mut ast::Program, file_path: &str) {
    use crate::parsing::ast::Expr;
    
    // Un span déjà rattaché à un fichier (instruction d'un fichier runtime,
    // déclaration importée) le garde.
    fn update_span(span: &mut token::Span, file: &str) {
        if span.file.is_none() {
            span.file = Some(file.to_string());
        }
    }

    fn update_params(params: &mut [ast::Param], file: &str) {
        for p in params {
            update_span(&mut p.span, file);
        }
    }
    
    // Helper pour mettre à jour les spans dans une expression
    fn update_expr_spans(expr: &mut Expr, file: &str) {
        match expr {
            Expr::Literal(_, span) | Expr::Ident(_, span) | Expr::SelfExpr(span) | Expr::ParentExpr(span) => {
                update_span(span, file);
            }
            Expr::Binary { left, right, span, .. } => {
                update_span(span, file);
                update_expr_spans(left, file);
                update_expr_spans(right, file);
            }
            Expr::Unary { operand, span, .. } => {
                update_span(span, file);
                update_expr_spans(operand, file);
            }
            Expr::Call { callee, args, span } => {
                update_span(span, file);
                update_expr_spans(callee, file);
                for arg in args {
                    update_expr_spans(arg, file);
                }
            }
            Expr::StaticCall { args, span, .. } => {
                update_span(span, file);
                for arg in args {
                    update_expr_spans(arg, file);
                }
            }
            Expr::Field { object, span, .. } => {
                update_span(span, file);
                update_expr_spans(object, file);
            }
            Expr::Index { object, index, span } => {
                update_span(span, file);
                update_expr_spans(object, file);
                update_expr_spans(index, file);
            }
            Expr::Array { elements, span } => {
                update_span(span, file);
                for elem in elements {
                    update_expr_spans(elem, file);
                }
            }
            Expr::Map { entries, span } => {
                update_span(span, file);
                for (k, v) in entries {
                    update_expr_spans(k, file);
                    update_expr_spans(v, file);
                }
            }
            Expr::Range { start, end, span, .. } => {
                update_span(span, file);
                update_expr_spans(start, file);
                update_expr_spans(end, file);
            }
            Expr::Match { subject, arms, span } => {
                update_span(span, file);
                update_expr_spans(subject, file);
                for arm in arms {
                    update_expr_spans(&mut arm.body, file);
                }
            }
            Expr::Template { parts, span } => {
                update_span(span, file);
                // Les expressions interpolées (`${...}`) ont leur PROPRE
                // span (voir ast.d::span_shift, qui corrige leur ligne/colonne
                // juste après le parsing) — sans cette récursion, leur champ
                // `file` restait toujours `None`, faisant retomber tout
                // diagnostic les concernant sur le fichier d'entrée passé à
                // `ocara build` au lieu du fichier réel où vit ce template
                // (voir docs/roadmap.d/langage-template-interpolation-span-position.md).
                for part in parts {
                    if let ast::TemplatePartExpr::Expr(inner) = part {
                        update_expr_spans(inner, file);
                    }
                }
            }
            Expr::Nameless { body, span, .. } => {
                update_span(span, file);
                for stmt in &mut body.stmts {
                    update_stmt_spans(stmt, file);
                }
            }
            Expr::Resolve { expr: e, span } | Expr::IsCheck { expr: e, span, .. } => {
                update_span(span, file);
                update_expr_spans(e, file);
            }
            Expr::New { args, span, .. } => {
                update_span(span, file);
                for arg in args {
                    update_expr_spans(arg, file);
                }
            }
            Expr::StaticConst { span, .. } => {
                update_span(span, file);
            }
            Expr::IncDec { target, span, .. } | Expr::NamedArg { value: target, span, .. } => {
                update_span(span, file);
                update_expr_spans(target, file);
            }
        }
    }

    // Helper pour mettre à jour les spans dans un statement
    fn update_stmt_spans(stmt: &mut Stmt, file: &str) {
        match stmt {
            Stmt::Var { value, span, .. } | Stmt::Const { value, span, .. } => {
                update_span(span, file);
                update_expr_spans(value, file);
            }
            Stmt::Assign { target, value, span } => {
                update_span(span, file);
                update_expr_spans(target, file);
                update_expr_spans(value, file);
            }
            Stmt::Expr(expr) => {
                update_expr_spans(expr, file);
            }
            Stmt::If { condition, then_block, elseif, else_block, span } => {
                update_span(span, file);
                update_expr_spans(condition, file);
                for stmt in &mut then_block.stmts {
                    update_stmt_spans(stmt, file);
                }
                for (cond, block) in elseif {
                    update_expr_spans(cond, file);
                    for stmt in &mut block.stmts {
                        update_stmt_spans(stmt, file);
                    }
                }
                if let Some(block) = else_block {
                    for stmt in &mut block.stmts {
                        update_stmt_spans(stmt, file);
                    }
                }
            }
            Stmt::While { condition, body, span } | Stmt::ForIn { iter: condition, body, span, .. } | Stmt::ForMap { iter: condition, body, span, .. } => {
                update_span(span, file);
                update_expr_spans(condition, file);
                for stmt in &mut body.stmts {
                    update_stmt_spans(stmt, file);
                }
            }
            Stmt::Switch { subject, cases, default, span } => {
                update_span(span, file);
                update_expr_spans(subject, file);
                for case in cases {
                    for stmt in &mut case.body.stmts {
                        update_stmt_spans(stmt, file);
                    }
                }
                if let Some(block) = default {
                    for stmt in &mut block.stmts {
                        update_stmt_spans(stmt, file);
                    }
                }
            }
            Stmt::Try { body, handlers, span } => {
                update_span(span, file);
                for stmt in &mut body.stmts {
                    update_stmt_spans(stmt, file);
                }
                for handler in handlers {
                    for stmt in &mut handler.body.stmts {
                        update_stmt_spans(stmt, file);
                    }
                }
            }
            Stmt::Return { value, span } | Stmt::Result { value, span } => {
                update_span(span, file);
                if let Some(expr) = value {
                    update_expr_spans(expr, file);
                }
            }
            Stmt::Raise { value, span } => {
                update_span(span, file);
                update_expr_spans(value, file);
            }
            Stmt::Emit { value, span } => {
                update_span(span, file);
                update_expr_spans(value, file);
            }
            Stmt::Break { span } | Stmt::Continue { span } => {
                update_span(span, file);
            }
        }
    }
    
    // Mettre à jour les classes
    for class in &mut program.classes {
        update_span(&mut class.span, file_path);
        for member in &mut class.members {
            match member {
                ast::ClassMember::Field { span, .. } => update_span(span, file_path),
                ast::ClassMember::Method { span, decl, .. } => {
                    update_span(span, file_path);
                    update_span(&mut decl.span, file_path);
                    update_params(&mut decl.params, file_path);
                    // Mettre à jour le body de la méthode
                    for stmt in &mut decl.body.stmts {
                        update_stmt_spans(stmt, file_path);
                    }
                }
                ast::ClassMember::Constructor { span, body, params } => {
                    update_span(span, file_path);
                    update_params(params, file_path);
                    // Mettre à jour le body du constructeur
                    for stmt in &mut body.stmts {
                        update_stmt_spans(stmt, file_path);
                    }
                }
                ast::ClassMember::Const { span, value, .. } => {
                    update_span(span, file_path);
                    update_expr_spans(value, file_path);
                }
            }
        }
    }
    
    // Mettre à jour les blocs runtime
    for block in &mut program.runtime_blocks {
        update_span(&mut block.span, file_path);
        for stmt in &mut block.statements {
            update_stmt_spans(stmt, file_path);
        }
    }

    // Mettre à jour les fonctions
    for func in &mut program.functions {
        update_span(&mut func.span, file_path);
        update_params(&mut func.params, file_path);
        for stmt in &mut func.body.stmts {
            update_stmt_spans(stmt, file_path);
        }
    }
    
    // Mettre à jour les interfaces (et leurs `wiring` : une erreur sur une
    // cible de `wiring` doit pointer le fichier de l'interface, pas le
    // fichier d'entrée de la compilation)
    for iface in &mut program.interfaces {
        update_span(&mut iface.span, file_path);
        for method in &mut iface.methods {
            update_span(&mut method.span, file_path);
        }
        for wiring in &mut iface.wirings {
            update_span(&mut wiring.span, file_path);
        }
    }

    // Mettre à jour les imports (réenfilés tels quels pour être chargés à
    // leur tour : une erreur de résolution doit pointer CE fichier)
    for imp in &mut program.imports {
        update_span(&mut imp.span, file_path);
    }
    
    // Mettre à jour les constantes
    for const_decl in &mut program.consts {
        update_span(&mut const_decl.span, file_path);
        update_expr_spans(&mut const_decl.value, file_path);
    }
    
    // Mettre à jour les génériques
    for generic in &mut program.generics {
        update_span(&mut generic.span, file_path);
        for member in &mut generic.members {
            match member {
                ast::ClassMember::Field { span, .. } => update_span(span, file_path),
                ast::ClassMember::Method { span, decl, .. } => {
                    update_span(span, file_path);
                    update_span(&mut decl.span, file_path);
                    update_params(&mut decl.params, file_path);
                    for stmt in &mut decl.body.stmts {
                        update_stmt_spans(stmt, file_path);
                    }
                }
                ast::ClassMember::Constructor { span, body, params } => {
                    update_span(span, file_path);
                    update_params(params, file_path);
                    for stmt in &mut body.stmts {
                        update_stmt_spans(stmt, file_path);
                    }
                }
                ast::ClassMember::Const { span, value, .. } => {
                    update_span(span, file_path);
                    update_expr_spans(value, file_path);
                }
            }
        }
    }
    
    // Mettre à jour les modules
    for module in &mut program.modules {
        update_span(&mut module.span, file_path);
        for member in &mut module.members {
            match member {
                ast::ClassMember::Method { span, decl, .. } => {
                    update_span(span, file_path);
                    update_span(&mut decl.span, file_path);
                    update_params(&mut decl.params, file_path);
                    for stmt in &mut decl.body.stmts {
                        update_stmt_spans(stmt, file_path);
                    }
                }
                ast::ClassMember::Const { span, value, .. } => {
                    update_span(span, file_path);
                    update_expr_spans(value, file_path);
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsing::ast::{Expr, Stmt, TemplatePartExpr};

    fn parse(src: &str) -> ast::Program {
        let tokens = Lexer::new(src).tokenize().expect("lex");
        Parser::new(tokens).parse_program().expect("parse")
    }

    #[test]
    fn template_interpolation_gets_file_stamped() {
        let mut program = parse(
            "function f(): string {\n    return `x ${y} z`\n}\n",
        );
        update_program_spans_with_file(&mut program, "helper/Alert.oc");

        let func = &program.functions[0];
        let Stmt::Return { value: Some(expr), .. } = &func.body.stmts[0] else {
            panic!("expected a return statement");
        };
        let Expr::Template { parts, .. } = expr else {
            panic!("expected Expr::Template");
        };
        let TemplatePartExpr::Expr(inner) = &parts[1] else {
            panic!("expected the interpolated part");
        };
        assert_eq!(inner.span().file.as_deref(), Some("helper/Alert.oc"));
    }
}
