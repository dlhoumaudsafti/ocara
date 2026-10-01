/// Évaluation à la compilation d'une expression constante simple — valeur
/// d'une constante de classe (`public const ZERO:int = -273`,
/// `public const TIMEOUT:int = 60 * 1000`), qui doit être connue à la
/// compilation pour être inlinée/émise en globale. Voir
/// docs/roadmap.d/langage-negative-class-const.md.
///
/// Couvre les littéraux, `-x`/`!x` et l'arithmétique entre littéraux (même
/// règles de type que le langage : `int`/`int` reste entier, un `float`
/// d'un côté donne un `float`, `string + string` concatène). `None` pour
/// toute autre forme (appel, variable...) ou une division par zéro.

use super::expressions::{BinOp, Expr, UnaryOp};
use super::literals::Literal;

impl Expr {
    pub fn const_literal(&self) -> Option<Literal> {
        match self {
            Expr::Literal(lit, _) => Some(lit.clone()),
            Expr::Unary { op: UnaryOp::Neg, operand, .. } => match operand.const_literal()? {
                Literal::Int(n)   => n.checked_neg().map(Literal::Int),
                Literal::Float(f) => Some(Literal::Float(-f)),
                _ => None,
            },
            Expr::Unary { op: UnaryOp::Not, operand, .. } => match operand.const_literal()? {
                Literal::Bool(b) => Some(Literal::Bool(!b)),
                _ => None,
            },
            Expr::Binary { op, left, right, .. } => fold_binary(op, left.const_literal()?, right.const_literal()?),
            _ => None,
        }
    }
}

fn fold_binary(op: &BinOp, left: Literal, right: Literal) -> Option<Literal> {
    match (left, right) {
        (Literal::Int(a), Literal::Int(b)) => Some(Literal::Int(match op {
            BinOp::Add => a.checked_add(b)?,
            BinOp::Sub => a.checked_sub(b)?,
            BinOp::Mul => a.checked_mul(b)?,
            BinOp::Div => a.checked_div(b)?,
            BinOp::Mod => a.checked_rem(b)?,
            _ => return None,
        })),
        (Literal::Int(a), Literal::Float(b)) => fold_float(op, a as f64, b),
        (Literal::Float(a), Literal::Int(b)) => fold_float(op, a, b as f64),
        (Literal::Float(a), Literal::Float(b)) => fold_float(op, a, b),
        (Literal::String(a), Literal::String(b)) if matches!(op, BinOp::Add) => Some(Literal::String(a + &b)),
        _ => None,
    }
}

fn fold_float(op: &BinOp, a: f64, b: f64) -> Option<Literal> {
    Some(Literal::Float(match op {
        BinOp::Add => a + b,
        BinOp::Sub => a - b,
        BinOp::Mul => a * b,
        BinOp::Div if b != 0.0 => a / b,
        _ => return None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsing::lexer::Lexer;
    use crate::parsing::parser::Parser;

    fn fold(src: &str) -> Option<Literal> {
        let tokens = Lexer::new(src).tokenize().expect("lex");
        Parser::new(tokens).parse_expr().expect("parse").const_literal()
    }

    #[test]
    fn negative_literals_fold() {
        assert_eq!(fold("-273"), Some(Literal::Int(-273)));
        assert_eq!(fold("-1.5"), Some(Literal::Float(-1.5)));
    }

    #[test]
    fn arithmetic_between_literals_folds_with_language_typing() {
        assert_eq!(fold("60 * 1000"), Some(Literal::Int(60000)));
        assert_eq!(fold("7 / 2"), Some(Literal::Int(3)));
        assert_eq!(fold("1 + 0.5"), Some(Literal::Float(1.5)));
        assert_eq!(fold("\"a\" + \"b\""), Some(Literal::String("ab".into())));
        assert_eq!(fold("not true"), Some(Literal::Bool(false)));
    }

    #[test]
    fn non_constant_or_invalid_expressions_do_not_fold() {
        assert_eq!(fold("x + 1"), None);
        assert_eq!(fold("f()"), None);
        assert_eq!(fold("1 / 0"), None);
        assert_eq!(fold("\"a\" + 1"), None);
    }
}
