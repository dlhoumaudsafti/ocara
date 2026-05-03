/// Fonctions utilitaires pour manipuler les paramètres de fonctions
use crate::parsing::ast::{Type, Param};

// Convertit une liste de paramètres en Vec<(String, Type)> pour FuncSig
pub fn params_to_vec(params: &[Param]) -> Vec<(String, Type)> {
    params.iter().map(|p| (p.name.clone(), p.ty.clone())).collect()
}

// Vérifie si la liste de paramètres contient un paramètre variadic (le dernier)
pub fn has_variadic_param(params: &[Param]) -> bool {
    params.last().map_or(false, |p| p.is_variadic)
}

// Compte le nombre de paramètres fixes (non-variadic) dans la liste de paramètres
pub fn fixed_params_count(params: &[Param]) -> usize {
    if has_variadic_param(params) {
        params.len() - 1
    } else {
        params.len()
    }
}

// Compte le nombre de paramètres obligatoires (sans default_value) dans la liste de paramètres
pub fn required_params_count(params: &[Param]) -> usize {
    // Compte les paramètres sans default_value (obligatoires)
    params.iter()
        .take_while(|p| !p.is_variadic && p.default_value.is_none())
        .count()
}
