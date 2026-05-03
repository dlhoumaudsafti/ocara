use crate::parsing::{ast::{self, Stmt}, diagnostic, lexer::Lexer, parser::Parser, token};
use std::collections::HashMap;
use std::fs;

// ─────────────────────────────────────────────────────────────────────────────
// Helpers pour extraire les numéros de ligne des statements (pour contexte runtime)
// ─────────────────────────────────────────────────────────────────────────────

pub fn get_stmt_start_line(stmt: &Stmt) -> usize {
    match stmt {
        Stmt::Var { span, .. } => span.line,
        Stmt::Const { span, .. } => span.line,
        Stmt::Assign { span, .. } => span.line,
        Stmt::Expr(e) => e.span().line,
        Stmt::If { span, .. } => span.line,
        Stmt::While { span, .. } => span.line,
        Stmt::ForIn { span, .. } => span.line,
        Stmt::ForMap { span, .. } => span.line,
        Stmt::Switch { span, .. } => span.line,
        Stmt::Return { span, .. } => span.line,
        Stmt::Break { span, .. } => span.line,
        Stmt::Continue { span, .. } => span.line,
        Stmt::Try { span, .. } => span.line,
        Stmt::Raise { span, .. } => span.line,
    }
}

pub fn get_stmt_end_line(stmt: &Stmt) -> usize {
    match stmt {
        Stmt::Var { span, .. } | Stmt::Const { span, .. } | Stmt::Assign { span, .. }
        | Stmt::Return { span, .. } | Stmt::Break { span, .. }
        | Stmt::Continue { span, .. } | Stmt::Raise { span, .. } => span.line,
        
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

pub fn expand_runtime_imports(program: &mut ast::Program, source_dir: &std::path::Path, input_file: &std::path::Path) {
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
                        diagnostic::print_error(input_file, rt_import.span.line, rt_import.span.col, &e);
                        std::process::exit(1);
                    }
                }
            }
            None => {
                let path_str = rt_import.path.join(".");
                diagnostic::print_error(
                    input_file,
                    rt_import.span.line,
                    rt_import.span.col,
                    &format!("runtime file not found: `{}` (tried .runtime.oc, .run.oc, .rt.oc, .oc)", path_str)
                );
                std::process::exit(1);
            }
        }
    }
    
    // Reconstruire la liste des blocs runtime et transformer les return
    program.runtime_blocks = blocks_map.into_iter()
        .map(|(kind, mut statements)| {
            transform_runtime_returns(&mut statements);
            ast::RuntimeBlock {
                kind,
                statements,
                span: token::Span::new(0, 0), // Span fusionné
            }
        })
        .collect();
}

/// Transforme les return dans les blocs runtime
/// - return ERROR → ERROR = 1
/// - return <expr> → ERROR = <expr>
fn transform_runtime_returns(stmts: &mut Vec<ast::Stmt>) {
    for stmt in stmts.iter_mut() {
        match stmt {
            ast::Stmt::Return { value: Some(expr), span } => {
                // Vérifier si c'est "return ERROR"
                if let ast::Expr::Ident(name, _) = expr {
                    if name == "ERROR" {
                        // Transformer en ERROR = 1
                        *stmt = ast::Stmt::Assign {
                            target: ast::Expr::Ident("ERROR".to_string(), token::Span::new(0, 0)),
                            value: ast::Expr::Literal(ast::Literal::Int(1), token::Span::new(0, 0)),
                            span: span.clone(),
                        };
                        continue;
                    }
                }
                // Sinon c'est "return <expr>" → ERROR = <expr>
                *stmt = ast::Stmt::Assign {
                    target: ast::Expr::Ident("ERROR".to_string(), token::Span::new(0, 0)),
                    value: expr.clone(),
                    span: span.clone(),
                };
            }
            ast::Stmt::If { then_block, elseif, else_block, .. } => {
                transform_runtime_returns(&mut then_block.stmts);
                for (_, block) in elseif.iter_mut() {
                    transform_runtime_returns(&mut block.stmts);
                }
                if let Some(block) = else_block {
                    transform_runtime_returns(&mut block.stmts);
                }
            }
            ast::Stmt::While { body, .. } | ast::Stmt::ForIn { body, .. } | ast::Stmt::ForMap { body, .. } => {
                transform_runtime_returns(&mut body.stmts);
            }
            ast::Stmt::Switch { cases, default, .. } => {
                for case in cases.iter_mut() {
                    transform_runtime_returns(&mut case.body.stmts);
                }
                if let Some(block) = default {
                    transform_runtime_returns(&mut block.stmts);
                }
            }
            ast::Stmt::Try { body, handlers, .. } => {
                transform_runtime_returns(&mut body.stmts);
                for handler in handlers.iter_mut() {
                    transform_runtime_returns(&mut handler.body.stmts);
                }
            }
            _ => {}
        }
    }
}

/// Résout le chemin d'un fichier runtime : essaie .runtime.oc, .run.oc, .rt.oc, .oc
fn resolve_runtime_file(source_dir: &std::path::Path, path: &[String]) -> Option<std::path::PathBuf> {
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

/// Charge et parse un fichier runtime, retourne tous les blocs définis
fn load_runtime_file(
    file_path: &std::path::Path,
    rt_import: &ast::RuntimeImport,
    _main_file: &std::path::Path,
) -> Result<Vec<ast::RuntimeBlock>, String> {
    // Lire le fichier
    let source = fs::read_to_string(file_path)
        .map_err(|e| format!("cannot read '{}': {}", file_path.display(), e))?;
    
    // Si kind est spécifié (ex: runtime config is init),
    // le contenu du fichier devient directement le contenu du bloc spécifié
    let source_to_parse = if let Some(target_kind) = rt_import.kind {
        // Séparer les imports du reste du code
        let mut imports = String::new();
        let mut body = String::new();
        
        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("import ") || trimmed.starts_with("namespace ") || trimmed.starts_with("//") || trimmed.is_empty() {
                imports.push_str(line);
                imports.push('\n');
            } else {
                body.push_str(line);
                body.push('\n');
            }
        }
        
        // Wrapper le corps dans un bloc runtime fictif
        format!("{}\n{} {{\n{}\n}}", imports, target_kind.as_str(), body)
    } else {
        // Sinon, parser le fichier tel quel (qui doit contenir des blocs déclarés)
        source
    };
    
    // Lexer
    let tokens = Lexer::new(&source_to_parse).tokenize()
        .map_err(|e| format!("lexing error in '{}': {:?}", file_path.display(), e))?;
    
    // Parser
    let runtime_program = Parser::new(tokens).parse_program()
        .map_err(|e| format!("parse error in '{}' at {}:{}: {}", 
            file_path.display(), e.span.line, e.span.col, e.message))?;
    
    // Si kind est spécifié, retourner le bloc créé
    if let Some(target_kind) = rt_import.kind {
        // Le bloc doit exister car on l'a créé nous-même
        for block in &runtime_program.runtime_blocks {
            if block.kind == target_kind {
                return Ok(vec![block.clone()]);
            }
        }
        return Err(format!(
            "internal error: failed to parse wrapped runtime block '{}'",
            target_kind.as_str()
        ));
    }
    
    Ok(runtime_program.runtime_blocks)
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper pour mettre à jour tous les Spans d'un Program avec le nom du fichier
// ─────────────────────────────────────────────────────────────────────────────

pub fn update_program_spans_with_file(program: &mut ast::Program, file_path: &str) {
    use crate::parsing::ast::Expr;
    
    // Helper pour mettre à jour un Span
    fn update_span(span: &mut token::Span, file: &str) {
        span.file = Some(file.to_string());
    }
    
    // Helper pour mettre à jour les spans dans une expression
    fn update_expr_spans(expr: &mut Expr, file: &str) {
        match expr {
            Expr::Literal(_, span) | Expr::Ident(_, span) | Expr::SelfExpr(span) => {
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
            Expr::Template { span, .. } => {
                update_span(span, file);
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
            Stmt::Return { value, span } => {
                update_span(span, file);
                if let Some(expr) = value {
                    update_expr_spans(expr, file);
                }
            }
            Stmt::Raise { value, span } => {
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
                    // Mettre à jour le body de la méthode
                    for stmt in &mut decl.body.stmts {
                        update_stmt_spans(stmt, file_path);
                    }
                }
                ast::ClassMember::Constructor { span, body, .. } => {
                    update_span(span, file_path);
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
    
    // Mettre à jour les fonctions
    for func in &mut program.functions {
        update_span(&mut func.span, file_path);
        for stmt in &mut func.body.stmts {
            update_stmt_spans(stmt, file_path);
        }
    }
    
    // Mettre à jour les interfaces
    for iface in &mut program.interfaces {
        update_span(&mut iface.span, file_path);
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
                    for stmt in &mut decl.body.stmts {
                        update_stmt_spans(stmt, file_path);
                    }
                }
                ast::ClassMember::Constructor { span, body, .. } => {
                    update_span(span, file_path);
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
