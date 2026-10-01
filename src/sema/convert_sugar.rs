/// Méthodes d'instance de conversion (`s.toInt()`, `n.toStr()`,
/// `arr.toStr(sep)`...) — sucre pour les méthodes statiques de `Convert`,
/// sous un nom DIFFÉRENT (préfixe de type source retiré, porté par le
/// receveur). Voir docs/roadmap.d/stdlib-convert-instance-methods.md et
/// docs/builtins/Convert.md.
///
/// Contrairement au sucre d'instance des classes `String`/`Array`/`Map`
/// (même nom en instance qu'en statique), la correspondance dépend du TYPE
/// du receveur — connu seulement ici : `string`/`array`/`map` sont tous des
/// pointeurs au niveau IR. La sema réécrit donc l'appel en
/// `Convert::<méthode>(receveur, args...)` (`AstRewrites::calls`, réinjecté
/// dans l'AST par `core::named_args`, qui ajoute aussi l'import
/// `ocara.Convert` s'il manque).
///
/// `mapKeysToArray`/`mapValuesToArray` n'ont volontairement pas d'alias :
/// `m.keys()`/`m.values()` (classe `Map`) font déjà exactement la même chose.

use crate::parsing::ast::{Expr, Type};
use crate::parsing::token::Span;
use crate::sema::error::SemaError;
use crate::sema::named_args::{site_key, CallTarget};
use crate::sema::typecheck::TypeChecker;

/// Méthode statique de `Convert` appelée par `<receveur de type ty>.<name>()`.
pub fn convert_method_for(ty: &Type, name: &str) -> Option<&'static str> {
    Some(match (ty, name) {
        (Type::String, "toInt")      => "strToInt",
        (Type::String, "toFloat")    => "strToFloat",
        (Type::String, "toBool")     => "strToBool",
        (Type::String, "toArray")    => "strToArray",
        (Type::String, "toMap")      => "strToMap",
        (Type::Int, "toStr")         => "intToStr",
        (Type::Int, "toFloat")       => "intToFloat",
        (Type::Int, "toBool")        => "intToBool",
        (Type::Float, "toStr")       => "floatToStr",
        (Type::Float, "toInt")       => "floatToInt",
        (Type::Float, "toBool")      => "floatToBool",
        (Type::Bool, "toStr")        => "boolToStr",
        (Type::Bool, "toInt")        => "boolToInt",
        (Type::Bool, "toFloat")      => "boolToFloat",
        (Type::Array(_), "toStr")    => "arrayToStr",
        (Type::Array(_), "toMap")    => "arrayToMap",
        (Type::Map(_, _), "toStr")   => "mapToStr",
        _ => return None,
    })
}

/// Noms des méthodes de conversion disponibles sur un receveur de type `ty`
/// (pour le message d'erreur d'un appel de méthode inconnue sur `int`/...).
pub fn conversion_methods_for(ty: &Type) -> Vec<String> {
    ["toInt", "toFloat", "toBool", "toStr", "toArray", "toMap"]
        .into_iter()
        .filter(|name| convert_method_for(ty, name).is_some())
        .map(|name| format!("{}()", name))
        .collect()
}

impl<'a> TypeChecker<'a> {
    /// `object.method(args)` (receveur déjà inféré en `obj_ty`) si c'est une
    /// conversion `Convert` : vérifie l'appel, enregistre sa réécriture et
    /// retourne son type. `None` : pas une conversion, résolution normale.
    pub(crate) fn resolve_convert_sugar(
        &mut self,
        object: &Expr,
        obj_ty: &Type,
        method: &str,
        args: &[Expr],
        span: &Span,
    ) -> Option<Type> {
        let static_name = convert_method_for(obj_ty, method)?;
        let sig = crate::builtins::builtin_class("Convert")?.methods.get(static_name)?.clone();
        let convert = self.symbols.local_name_for_builtin("Convert");

        let Some(resolved) = self.resolve_named_call(args, |_| {
            Some(CallTarget::from_sig(format!("{}::{}", convert, static_name), &sig, true))
        }) else {
            return Some(sig.ret_ty.clone());
        };
        let extra = sig.params.len() - 1;
        if resolved.len() != extra {
            self.errors.push(SemaError::WrongArgCount {
                name:     format!("{}.{}", crate::sema::typecheck::type_name(obj_ty), method),
                expected: extra,
                found:    resolved.len(),
                span:     span.clone(),
            });
        }
        for arg in resolved.iter() {
            self.infer_expr(arg);
        }

        let mut full_args = vec![object.clone()];
        full_args.extend(resolved.iter().cloned());
        self.rewrites.calls.insert(site_key(span), Expr::StaticCall {
            class:  convert,
            method: static_name.to_string(),
            args:   full_args,
            span:   span.clone(),
        });
        Some(sig.ret_ty.clone())
    }
}
