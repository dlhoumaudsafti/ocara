/// Tests unitaires — positions des interpolations d'un gabarit
/// `HTML::renderFile` : rapportées sur l'appel, dans son fichier source.

use crate::core::render_file::build_template_from_file;
use crate::parsing::ast::*;
use crate::parsing::token::Span;

fn call_span() -> Span {
    let mut span = Span::new(9, 17);
    span.file = Some("ctrl/Page.oc".to_string());
    span
}

fn template_from(name: &str, content: &str) -> Expr {
    let dir = std::env::temp_dir().join(format!("ocara_render_file_{}_{}", std::process::id(), name));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("page.html");
    std::fs::write(&path, content).unwrap();
    let expr = build_template_from_file(&path, &call_span()).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    expr
}

fn interpolated_spans(expr: &Expr) -> Vec<Span> {
    let Expr::Template { parts, .. } = expr else { panic!("template attendu") };
    parts.iter().filter_map(|p| match p {
        TemplatePartExpr::Expr(e) => Some(e.span().clone()),
        TemplatePartExpr::Literal(_) => None,
    }).collect()
}

#[test]
fn interpolations_point_to_render_file_call() {
    let expr = template_from("calls", "<p>${name}</p>\n<p>${other}</p>\n");
    let spans = interpolated_spans(&expr);
    assert_eq!(spans.len(), 2);
    for span in spans {
        assert_eq!(span.line, 9);
        assert_eq!(span.col, 17);
        assert_eq!(span.file.as_deref(), Some("ctrl/Page.oc"));
    }
}

#[test]
fn nested_expression_keeps_call_line_and_file() {
    let expr = template_from("nested", "${a + b}");
    let Expr::Template { parts, .. } = &expr else { panic!("template attendu") };
    let Some(TemplatePartExpr::Expr(e)) = parts.first() else { panic!("interpolation attendue") };
    let Expr::Binary { right, .. } = e.as_ref() else { panic!("binaire attendu") };
    assert_eq!(right.span().line, 9);
    assert_eq!(right.span().file.as_deref(), Some("ctrl/Page.oc"));
}
