/// Expression conditionnelle `cond then a else b` — voir `docs/EBNF.md` §11.3.
///
/// - La condition suit les règles d'un `if` (`bool`, ou `mixed` évalué sur sa
///   valeur) et raffine les types dans chaque branche.
/// - Le type est l'union des types des deux branches, comme `match`.
/// - Forme courte `empty`/`zero`/`null` : valeur par défaut du type de la
///   branche `then`, enregistrée comme littéral (`AstRewrites::cond_defaults`,
///   réinjecté par `core::named_args`) — le lowering ne la voit jamais.

use crate::parsing::ast::{CondDefault, CondElse, Expr, Literal, Type};
use crate::parsing::token::Span;
use crate::sema::error::SemaError;
use crate::sema::named_args::site_key;
use crate::sema::typecheck::{type_name, types_compat, union_of_types, TypeChecker};

/// Littéral de la valeur par défaut `default` pour une branche `then` de
/// type `ty`, et son type ; `None` si ce type n'en a pas.
fn default_value(default: CondDefault, ty: &Type, span: &Span) -> Option<(Expr, Type)> {
    let value = match (default, ty) {
        (CondDefault::Null, _) => return Some((Expr::Literal(Literal::Null, span.clone()), Type::Null)),
        (CondDefault::Empty, Type::String) => Expr::Literal(Literal::String(String::new()), span.clone()),
        (CondDefault::Empty, Type::Array(_)) => Expr::Array { elements: Vec::new(), span: span.clone() },
        (CondDefault::Empty, Type::Map(..)) => Expr::Map { entries: Vec::new(), span: span.clone() },
        (CondDefault::Zero, Type::Int) => Expr::Literal(Literal::Int(0), span.clone()),
        (CondDefault::Zero, Type::Float) => Expr::Literal(Literal::Float(0.0), span.clone()),
        _ => return None,
    };
    Some((value, ty.clone()))
}

impl<'a> TypeChecker<'a> {
    pub(crate) fn infer_cond(&mut self, cond: &Expr, then_expr: &Expr, else_expr: &CondElse, hint: Option<Type>) -> Type {
        let cond_ty = self.infer_expr(cond);
        if !types_compat(&cond_ty, &Type::Bool, self.symbols) {
            self.errors.push(SemaError::TypeMismatch {
                expected: "bool".into(),
                found:    type_name(&cond_ty),
                span:     self.with_runtime_ctx(cond.span()),
            });
        }
        let narrowed = self.condition_narrowings(cond);
        self.expected_hint = hint.clone();
        let then_ty = self.infer_narrowed(then_expr, &narrowed.then);
        let else_ty = match else_expr {
            CondElse::Value(e) => {
                self.expected_hint = hint;
                self.infer_narrowed(e, &narrowed.other)
            }
            CondElse::Default(default, span) => self.cond_default(*default, &then_ty, span),
        };
        union_of_types(vec![then_ty, else_ty])
    }

    fn cond_default(&mut self, default: CondDefault, then_ty: &Type, span: &Span) -> Type {
        let keyword = default.keyword().to_string();
        // Corps d'un `generic` vérifié avec `T` permissif (`mixed`), ou une
        // instanciation qui choisirait un autre littéral : le corps est
        // partagé par toutes les instanciations.
        let on_type_param = default != CondDefault::Null && self.current_generic.is_some() && *then_ty == Type::Mixed;
        let Some((value, ty)) = default_value(default, then_ty, span).filter(|_| !on_type_param) else {
            let span = self.with_runtime_ctx(span);
            self.errors.push(match on_type_param {
                true => SemaError::CondDefaultOnTypeParam { keyword, span },
                false => SemaError::CondDefaultType { keyword, type_name: type_name(then_ty), span },
            });
            return then_ty.clone();
        };
        let key = site_key(span);
        if self.rewrites.cond_defaults.get(&key).is_some_and(|known| *known != value) {
            self.errors.push(SemaError::CondDefaultOnTypeParam { keyword, span: self.with_runtime_ctx(span) });
            return ty;
        }
        self.rewrites.cond_defaults.insert(key, value);
        ty
    }
}
