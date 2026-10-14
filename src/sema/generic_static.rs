/// Appel statique sur un `generic` (`Box<int>::of(4)`, `Box::of(4)`) — voir
/// docs/roadmap.d/langage-generique-methode-statique.md.
///
/// Arguments de type : explicites (`Box<int>::`), sinon inférés depuis les
/// arguments de l'appel, puis depuis le type attendu (`var b:Box<int> =
/// Box::of(4)`), puis les valeurs par défaut des paramètres de type. Sans
/// instanciation déductible : erreur. L'instanciation retenue est notée
/// (`AstRewrites::generic_static`) : la monomorphisation appelle ensuite la
/// méthode de la classe spécialisée (`Box_int_of`).

use std::collections::HashMap;
use crate::parsing::ast::{Expr, Type};
use crate::parsing::token::Span;
use crate::sema::error::SemaError;
use crate::sema::named_args::site_key;
use crate::sema::typecheck::{substitute_type_params, type_name, types_compat, TypeChecker};

impl<'a> TypeChecker<'a> {
    /// `Some(type de retour)` si `class` est un `generic`, `None` sinon.
    pub(crate) fn check_generic_static_call(
        &mut self,
        class: &str,
        method: &str,
        explicit: &[Type],
        args: &[Expr],
        span: &Span,
        hint: Option<&Type>,
    ) -> Option<Type> {
        let ginfo = self.symbols.lookup_generic(class)?.clone();
        let arg_tys: Vec<Type> = args.iter().map(|a| self.infer_expr(a)).collect();
        let Some(sig) = ginfo.methods.get(method).cloned() else {
            self.errors.push(SemaError::UndefinedSymbol { name: format!("{}::{}", class, method), span: span.clone() });
            return Some(Type::Mixed);
        };
        if !sig.is_static {
            self.errors.push(SemaError::NotStaticMethod { class: class.to_string(), method: method.to_string(), span: span.clone() });
        }
        let names: Vec<&str> = ginfo.type_params.iter().map(|p| p.name.as_str()).collect();

        let type_args: Vec<Type> = if !explicit.is_empty() {
            let min = ginfo.type_params.iter().filter(|p| p.default.is_none()).count();
            if explicit.len() < min || explicit.len() > names.len() {
                self.errors.push(SemaError::GenericArityMismatch {
                    name: class.to_string(), expected_min: min, expected_max: names.len(), found: explicit.len(), span: span.clone(),
                });
                return Some(Type::Mixed);
            }
            explicit.to_vec()
        } else {
            let mut bindings: HashMap<String, Type> = HashMap::new();
            for ((_, param_ty), found) in sig.params.iter().zip(&arg_tys) {
                unify(param_ty, found, &names, &mut bindings);
            }
            if let Some(Type::Generic { name, args: hinted }) = hint {
                if name == class {
                    for (n, ty) in names.iter().zip(hinted) {
                        bindings.entry(n.to_string()).or_insert_with(|| ty.clone());
                    }
                }
            }
            let mut resolved = Vec::new();
            for p in &ginfo.type_params {
                match bindings.get(&p.name).cloned().or_else(|| p.default.clone()) {
                    Some(ty) => resolved.push(ty),
                    None => {
                        self.errors.push(SemaError::GenericTypeArgsNotInferred {
                            name: class.to_string(), method: method.to_string(), span: span.clone(),
                        });
                        return Some(Type::Mixed);
                    }
                }
            }
            resolved
        };

        let arity_ok = args.len() >= sig.required_params_count && (sig.has_variadic || args.len() <= sig.params.len());
        if !arity_ok {
            self.errors.push(SemaError::WrongArgCount {
                name: format!("{}::{}", class, method), expected: sig.required_params_count, found: args.len(), span: span.clone(),
            });
        }
        for ((param, param_ty), (arg, found)) in sig.params.iter().zip(args.iter().zip(&arg_tys)) {
            let want = substitute_type_params(param_ty, &ginfo.type_params, &type_args);
            if !types_compat(found, &want, &self.symbols) {
                self.errors.push(SemaError::ArgTypeMismatch {
                    callee: format!("{}::{}", class, method), param: param.clone(),
                    expected: type_name(&want), found: type_name(found), span: arg.span().clone(),
                });
            }
        }
        self.rewrites.generic_static.insert(site_key(span), type_args.clone());
        let ret = substitute_type_params(&sig.ret_ty, &ginfo.type_params, &type_args);
        Some(if sig.is_async { Type::Resolvable(Box::new(ret)) } else { ret })
    }
}

/// Lie les paramètres de type `names` apparaissant dans `param` au type
/// correspondant de `found` (première liaison retenue).
fn unify(param: &Type, found: &Type, names: &[&str], bindings: &mut HashMap<String, Type>) {
    match (param, found) {
        (_, Type::Mixed) => {}
        (Type::Named(n), _) if names.contains(&n.as_str()) => {
            bindings.entry(n.clone()).or_insert_with(|| found.clone());
        }
        (Type::Array(p), Type::Array(f)) => unify(p, f, names, bindings),
        (Type::Map(pk, pv), Type::Map(fk, fv)) => {
            unify(pk, fk, names, bindings);
            unify(pv, fv, names, bindings);
        }
        (Type::Generic { name: pn, args: pa }, Type::Generic { name: fname, args: fa }) if pn == fname => {
            for (p, f) in pa.iter().zip(fa) {
                unify(p, f, names, bindings);
            }
        }
        _ => {}
    }
}
