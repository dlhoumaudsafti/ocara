/// Type par clé d'une map initialisée par un littéral à clés chaîne :
/// `const d:map<string, CarEntity|string> = {"car": car, "rows_html": ""}`
/// → `d['rows_html']` est une `string`, `d['car']` un `CarEntity`, au lieu du
/// type de valeur déclaré pour toutes les clés. Une clé absente du littéral,
/// une valeur `null`/`mixed` ou une map réaffectée retombent sur `map<K, V>`.
use std::collections::HashMap;

use crate::parsing::ast::{Expr, Literal, Type};
use crate::sema::typecheck::{types_compat, TypeChecker};

impl<'a> TypeChecker<'a> {
    /// Littéral `{...}` qui vient d'être inféré : type de chaque clé chaîne
    /// (aucun si une clé n'est pas une chaîne littérale).
    pub(crate) fn note_literal_keys(&mut self, entries: &[(Expr, Expr)], value_types: Vec<Type>) {
        let keys: Option<HashMap<String, Type>> = entries.iter().zip(value_types)
            .map(|((k, _), ty)| match k {
                Expr::Literal(Literal::String(key), _) => Some((key.clone(), ty)),
                _ => None,
            })
            .collect();
        self.last_map_literal = keys;
    }

    /// Déclaration de `name` (type `declared`, valeur `value`) : retient le
    /// type de chaque clé si la valeur est un littéral de map.
    pub(crate) fn bind_map_keys(&mut self, name: &str, declared: &Type, value: &Expr) {
        let keys = self.last_map_literal.take();
        let (Type::Map(_, declared_value), Expr::Map { .. }, Some(keys)) = (declared, value, keys) else {
            self.map_keys.remove(name);
            return;
        };
        let typed = keys.into_iter()
            .map(|(key, ty)| {
                let precise = !matches!(ty, Type::Null | Type::Mixed) && types_compat(&ty, declared_value, self.symbols);
                (key, if precise { ty } else { declared_value.as_ref().clone() })
            })
            .collect();
        self.map_keys.insert(name.to_string(), typed);
    }

    pub(crate) fn forget_map_keys(&mut self, name: &str) {
        self.map_keys.remove(name);
    }

    /// Type de `object[index]` quand `object` est une telle map et `index`
    /// une chaîne littérale qui en est une clé.
    pub(crate) fn map_key_type(&self, object: &Expr, index: &Expr) -> Option<Type> {
        let (Expr::Ident(name, _), Expr::Literal(Literal::String(key), _)) = (object, index) else { return None };
        self.map_keys.get(name)?.get(key).cloned()
    }
}
