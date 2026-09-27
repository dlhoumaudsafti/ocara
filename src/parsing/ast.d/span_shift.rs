/// Décalage des spans d'une expression re-parsée en isolation (interpolation
/// `${...}` d'un template literal, voir `expressions.rs::parse_primary` pour
/// le site d'appel) — le sous-lexer/sous-parser utilisés pour cette
/// expression ne connaissent que son texte brut, donc toute position qu'ils
/// produisent démarre à (ligne 1, colonne 1) de CE texte, jamais la vraie
/// position dans le fichier source d'origine. `shift_expr_spans` corrige ça
/// après coup : `origin` est la position réelle du premier caractère de
/// l'interpolation dans le fichier d'origine, et chaque span de l'expression
/// re-parsée (relative à ce texte) est traduite en position absolue.
///
/// Mirroir volontaire de `core::runtime_expand::update_expr_spans`/
/// `update_stmt_spans` (même liste de variants, même structure de
/// récursion) mais avec une opération de fond différente (décalage plutôt
/// que remplacement de `file`) — dupliqué plutôt que factorisé pour ne pas
/// risquer de régression sur cette fonction existante déjà éprouvée en la
/// généralisant sous pression de temps ; les deux marchent sur le même AST
/// mais ne se recoupent jamais au même moment (celle-ci tourne pendant le
/// parsing, l'autre après, au chargement d'un import).

use super::expressions::Expr;
use super::statements::Stmt;
use crate::parsing::token::Span;

fn shift_span(span: &mut Span, origin: &Span) {
    if span.line <= 1 {
        span.col = origin.col + span.col.saturating_sub(1);
        span.line = origin.line;
    } else {
        span.line = origin.line + span.line - 1;
        // Colonne déjà correcte : après un saut de ligne à l'intérieur du
        // texte de l'interpolation, la colonne ne dépend plus de `origin.col`.
    }
}

pub fn shift_expr_spans(expr: &mut Expr, origin: &Span) {
    match expr {
        Expr::Literal(_, span) | Expr::Ident(_, span) | Expr::SelfExpr(span) | Expr::ParentExpr(span) => {
            shift_span(span, origin);
        }
        Expr::Binary { left, right, span, .. } => {
            shift_span(span, origin);
            shift_expr_spans(left, origin);
            shift_expr_spans(right, origin);
        }
        Expr::Unary { operand, span, .. } => {
            shift_span(span, origin);
            shift_expr_spans(operand, origin);
        }
        Expr::Call { callee, args, span } => {
            shift_span(span, origin);
            shift_expr_spans(callee, origin);
            for arg in args {
                shift_expr_spans(arg, origin);
            }
        }
        Expr::StaticCall { args, span, .. } => {
            shift_span(span, origin);
            for arg in args {
                shift_expr_spans(arg, origin);
            }
        }
        Expr::Field { object, span, .. } => {
            shift_span(span, origin);
            shift_expr_spans(object, origin);
        }
        Expr::Index { object, index, span } => {
            shift_span(span, origin);
            shift_expr_spans(object, origin);
            shift_expr_spans(index, origin);
        }
        Expr::Array { elements, span } => {
            shift_span(span, origin);
            for elem in elements {
                shift_expr_spans(elem, origin);
            }
        }
        Expr::Map { entries, span } => {
            shift_span(span, origin);
            for (k, v) in entries {
                shift_expr_spans(k, origin);
                shift_expr_spans(v, origin);
            }
        }
        Expr::Range { start, end, span, .. } => {
            shift_span(span, origin);
            shift_expr_spans(start, origin);
            shift_expr_spans(end, origin);
        }
        Expr::Match { subject, arms, span } => {
            shift_span(span, origin);
            shift_expr_spans(subject, origin);
            for arm in arms {
                shift_expr_spans(&mut arm.body, origin);
            }
        }
        Expr::Template { parts, span } => {
            shift_span(span, origin);
            for part in parts {
                if let super::literals::TemplatePartExpr::Expr(inner) = part {
                    shift_expr_spans(inner, origin);
                }
            }
        }
        Expr::Nameless { body, span, .. } => {
            shift_span(span, origin);
            for stmt in &mut body.stmts {
                shift_stmt_spans(stmt, origin);
            }
        }
        Expr::Resolve { expr: e, span } | Expr::IsCheck { expr: e, span, .. } => {
            shift_span(span, origin);
            shift_expr_spans(e, origin);
        }
        Expr::New { args, span, .. } => {
            shift_span(span, origin);
            for arg in args {
                shift_expr_spans(arg, origin);
            }
        }
        Expr::StaticConst { span, .. } => {
            shift_span(span, origin);
        }
        Expr::IncDec { target, span, .. } => {
            shift_span(span, origin);
            shift_expr_spans(target, origin);
        }
    }
}

pub fn shift_stmt_spans(stmt: &mut Stmt, origin: &Span) {
    match stmt {
        Stmt::Var { value, span, .. } | Stmt::Const { value, span, .. } => {
            shift_span(span, origin);
            shift_expr_spans(value, origin);
        }
        Stmt::Assign { target, value, span } => {
            shift_span(span, origin);
            shift_expr_spans(target, origin);
            shift_expr_spans(value, origin);
        }
        Stmt::Expr(expr) => {
            shift_expr_spans(expr, origin);
        }
        Stmt::If { condition, then_block, elseif, else_block, span } => {
            shift_span(span, origin);
            shift_expr_spans(condition, origin);
            for stmt in &mut then_block.stmts {
                shift_stmt_spans(stmt, origin);
            }
            for (cond, block) in elseif {
                shift_expr_spans(cond, origin);
                for stmt in &mut block.stmts {
                    shift_stmt_spans(stmt, origin);
                }
            }
            if let Some(block) = else_block {
                for stmt in &mut block.stmts {
                    shift_stmt_spans(stmt, origin);
                }
            }
        }
        Stmt::While { condition, body, span } | Stmt::ForIn { iter: condition, body, span, .. } | Stmt::ForMap { iter: condition, body, span, .. } => {
            shift_span(span, origin);
            shift_expr_spans(condition, origin);
            for stmt in &mut body.stmts {
                shift_stmt_spans(stmt, origin);
            }
        }
        Stmt::Switch { subject, cases, default, span } => {
            shift_span(span, origin);
            shift_expr_spans(subject, origin);
            for case in cases {
                for stmt in &mut case.body.stmts {
                    shift_stmt_spans(stmt, origin);
                }
            }
            if let Some(block) = default {
                for stmt in &mut block.stmts {
                    shift_stmt_spans(stmt, origin);
                }
            }
        }
        Stmt::Try { body, handlers, span } => {
            shift_span(span, origin);
            for stmt in &mut body.stmts {
                shift_stmt_spans(stmt, origin);
            }
            for handler in handlers {
                for stmt in &mut handler.body.stmts {
                    shift_stmt_spans(stmt, origin);
                }
            }
        }
        Stmt::Return { value, span } | Stmt::Result { value, span } => {
            shift_span(span, origin);
            if let Some(expr) = value {
                shift_expr_spans(expr, origin);
            }
        }
        Stmt::Raise { value, span } => {
            shift_span(span, origin);
            shift_expr_spans(value, origin);
        }
        Stmt::Emit { value, span } => {
            shift_span(span, origin);
            shift_expr_spans(value, origin);
        }
        Stmt::Break { span } | Stmt::Continue { span } => {
            shift_span(span, origin);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsing::ast::TemplatePartExpr;
    use crate::parsing::lexer::Lexer;
    use crate::parsing::parser::Parser;

    fn parse_expr(src: &str) -> Expr {
        let tokens = Lexer::new(src).tokenize().expect("lex");
        Parser::new(tokens).parse_expr().expect("parse")
    }

    #[test]
    fn single_line_ident_shifts_by_origin_column() {
        // "message" ré-analysé isolément a son span à (1,1) — l'identifiant
        // vivait en réalité à la colonne 56 de la ligne 19 du vrai fichier.
        let mut expr = parse_expr("message");
        let origin = Span::new(19, 56);
        shift_expr_spans(&mut expr, &origin);
        assert_eq!(expr.span().line, 19);
        assert_eq!(expr.span().col, 56);
    }

    #[test]
    fn single_line_ident_mid_expression_adds_relative_column() {
        // Dans "1 + message", "message" démarre à la colonne 5 du texte isolé
        // (1-based) — doit devenir origin.col + 5 - 1 dans le vrai fichier.
        let mut expr = parse_expr("1 + message");
        let origin = Span::new(3, 10);
        shift_expr_spans(&mut expr, &origin);
        if let Expr::Binary { right, .. } = &expr {
            assert_eq!(right.span().line, 3);
            assert_eq!(right.span().col, 10 + 5 - 1);
        } else {
            panic!("expected Expr::Binary");
        }
    }

    #[test]
    fn multiline_expression_shifts_line_but_not_column_after_first() {
        let mut expr = parse_expr("1 +\nmessage");
        let origin = Span::new(7, 20);
        shift_expr_spans(&mut expr, &origin);
        if let Expr::Binary { left, right, .. } = &expr {
            // "1" reste sur la première ligne du texte isolé → décalé par origin.
            assert_eq!(left.span().line, 7);
            assert_eq!(left.span().col, 20);
            // "message" est sur la 2e ligne du texte isolé (ligne 2, col 1)
            // → ligne = origin.line + 2 - 1 = 8, colonne inchangée (1).
            assert_eq!(right.span().line, 8);
            assert_eq!(right.span().col, 1);
        } else {
            panic!("expected Expr::Binary");
        }
    }

    #[test]
    fn nested_call_args_all_shifted() {
        let mut expr = parse_expr("Format::price(x)");
        let origin = Span::new(5, 1);
        shift_expr_spans(&mut expr, &origin);
        if let Expr::StaticCall { args, .. } = &expr {
            assert_eq!(args[0].span().line, 5);
        } else {
            panic!("expected Expr::StaticCall");
        }
    }

    #[test]
    fn template_inside_template_parts_are_shifted() {
        let mut expr = parse_expr("`${message}`");
        let origin = Span::new(2, 3);
        shift_expr_spans(&mut expr, &origin);
        if let Expr::Template { parts, .. } = &expr {
            if let TemplatePartExpr::Expr(inner) = &parts[0] {
                assert_eq!(inner.span().line, 2);
            } else {
                panic!("expected an interpolated part");
            }
        } else {
            panic!("expected Expr::Template");
        }
    }
}
