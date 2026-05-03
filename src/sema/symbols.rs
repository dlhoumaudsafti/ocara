// Sous-modules de la table des symboles (voir symbols.d/mod.rs)
#[path = "symbols.d/mod.rs"]
mod symbols_d;

// Re-exports des types publics depuis symbols.d/types.rs
pub use symbols_d::types::{
    FuncSig,
    FieldInfo,
    ClassInfo,
    SymbolTable,
};

impl SymbolTable {
    pub fn new() -> Self {
        let mut table = Self::default();
        
        // Enregistrer automatiquement la classe String pour les méthodes intégrées
        // sur les variables de type string (ex: "hello".trim())
        if let Some(string_class) = crate::builtins::builtin_class("String") {
            table.classes.insert("String".to_string(), string_class);
        }
        
        // Enregistrer automatiquement la classe Array pour les méthodes intégrées
        // sur les variables de type array (ex: arr.len())
        if let Some(array_class) = crate::builtins::builtin_class("Array") {
            table.classes.insert("Array".to_string(), array_class);
        }
        
        // Enregistrer automatiquement la classe Map pour les méthodes intégrées
        // sur les variables de type map (ex: m.size())
        if let Some(map_class) = crate::builtins::builtin_class("Map") {
            table.classes.insert("Map".to_string(), map_class);
        }
        
        // Enregistrer automatiquement la classe JSON pour les méthodes intégrées
        // sur les variables de type array/map/string (ex: data.encode(), json.decode())
        if let Some(json_class) = crate::builtins::builtin_class("JSON") {
            table.classes.insert("JSON".to_string(), json_class);
        }
        
        table
    }
    // Les fonctions de recherche (lookup_*) sont maintenant dans lookups.rs
    // Les fonctions d'enregistrement (register_*) sont maintenant dans registers.rs
}
