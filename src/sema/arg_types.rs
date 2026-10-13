/// Types des arguments à l'appel (E62) — voir
/// docs/roadmap.d/sema-types-arguments-appel.md.
///
/// Chaque argument d'un appel dont la cible est connue (fonction libre,
/// méthode d'instance ou statique, constructeur, builtin) doit être
/// compatible (`types_compat`) avec le type déclaré de son paramètre ; les
/// arguments d'un paramètre `variadic<T>` sont comparés à `T` (un `T[]`
/// transmis tel quel est aussi accepté). Jusqu'ici seule l'arité était
/// contrôlée : `add("1", 2)` avec `a:int` compilait et passait l'adresse de
/// la chaîne comme entier.
///
/// Les appels nommés arrivent ici déjà résolus en liste positionnelle
/// (`resolve_named_call`), valeurs par défaut comprises.

use crate::parsing::ast::{Expr, Param, Type};
use crate::sema::error::SemaError;
use crate::sema::typecheck::{type_name, types_compat, TypeChecker};

/// Paramètres attendus d'une cible : (nom, type) dans l'ordre de
/// déclaration, le dernier étant `variadic<T>` (type `T`) si `variadic`.
pub(crate) struct ExpectedParams {
    pub params:   Vec<(String, Type)>,
    pub variadic: bool,
}

impl ExpectedParams {
    pub fn from_sig(sig: &crate::sema::symbols::FuncSig) -> Self {
        Self { params: sig.params.clone(), variadic: sig.has_variadic }
    }

    /// Retire les `skip` premiers paramètres (receveur passé implicitement
    /// par le sucre d'instance : `s.trim()` → `String::trim(s)`).
    pub fn skip(mut self, skip: usize) -> Self {
        let n = skip.min(self.params.len().saturating_sub(usize::from(self.variadic)));
        self.params.drain(..n);
        self
    }

    /// Paramètre recevant l'argument d'indice `i`.
    fn slot(&self, i: usize) -> Option<&(String, Type)> {
        match self.params.get(i) {
            Some(p) => Some(p),
            None if self.variadic => self.params.last(),
            None => None,
        }
    }
}

impl<'a> TypeChecker<'a> {
    /// Paramètres déclarés de `class::method` (constructeur : `init`), en
    /// remontant la chaîne `extends`.
    pub(crate) fn user_expected_params(&self, class: &str, method: &str) -> Option<ExpectedParams> {
        let mut owner = class.to_string();
        loop {
            if let Some(params) = self.callable_params.get(&format!("{}::{}", owner, method)) {
                return Some(ExpectedParams {
                    params:   params.iter().map(|p| (p.name.clone(), p.ty.clone())).collect(),
                    variadic: params.last().is_some_and(|p| p.is_variadic),
                });
            }
            owner = self.symbols.lookup_parent_class(&owner)?;
        }
    }

    /// Infère chaque argument et le compare au type de son paramètre.
    /// Rend les types inférés, dans l'ordre des arguments.
    pub(crate) fn check_call_args(&mut self, callee: &str, expected: &ExpectedParams, args: &[Expr]) -> Vec<Type> {
        let mut tys = Vec::with_capacity(args.len());
        for (i, arg) in args.iter().enumerate() {
            let found = self.infer_expr(arg);
            if let Some((param, want)) = expected.slot(i) {
                let is_variadic_slot = expected.variadic && i + 1 >= expected.params.len();
                let ok = types_compat(&found, want, &self.symbols)
                    || (is_variadic_slot && types_compat(&found, &Type::Array(Box::new(want.clone())), &self.symbols));
                if !ok {
                    self.errors.push(SemaError::ArgTypeMismatch {
                        callee:   callee.to_string(),
                        param:    param.clone(),
                        expected: type_name(want),
                        found:    type_name(&found),
                        span:     arg.span().clone(),
                    });
                }
            }
            tys.push(found);
        }
        tys
    }

    /// Valeur par défaut d'un paramètre : même règle qu'un argument
    /// (`f(n:int = "x")` refusé à la déclaration). Ignoré dans un `generic`
    /// (`T` n'est connu qu'à l'instanciation).
    pub(crate) fn check_param_defaults(&mut self, callee: &str, params: &[Param]) {
        if self.current_generic.is_some() {
            return;
        }
        for param in params {
            let Some(default) = &param.default_value else { continue };
            let found = self.infer_expr(default);
            if !types_compat(&found, &param.ty, &self.symbols) {
                self.errors.push(SemaError::ArgTypeMismatch {
                    callee:   callee.to_string(),
                    param:    param.name.clone(),
                    expected: type_name(&param.ty),
                    found:    type_name(&found),
                    span:     default.span().clone(),
                });
            }
        }
    }
}
