/// Fonctions de recherche (lookup) dans la table des symboles
use super::super::{
    SymbolTable, FuncSig, ClassInfo, ModuleInfo, GenericInfo, 
    InterfaceInfo, EnumInfo, FieldInfo,
};
use crate::parsing::ast::{Type, Visibility};

impl SymbolTable {
    /// Recherche une fonction par son nom
    pub fn lookup_function(&self, name: &str) -> Option<&FuncSig> {
        self.functions.get(name)
    }

    /// Recherche une classe par son nom
    pub fn lookup_class(&self, name: &str) -> Option<&ClassInfo> {
        self.classes.get(name)
    }

    /// Recherche un module (mixin) par son nom
    #[allow(dead_code)]
    pub fn lookup_module(&self, name: &str) -> Option<&ModuleInfo> {
        self.modules.get(name)
    }

    /// Recherche un générique par son nom
    pub fn lookup_generic(&self, name: &str) -> Option<&GenericInfo> {
        self.generics.get(name)
    }

    /// Recherche une interface par son nom
    #[allow(dead_code)]
    pub fn lookup_interface(&self, name: &str) -> Option<&InterfaceInfo> {
        self.interfaces.get(name)
    }

    /// Recherche une constante par son nom
    pub fn lookup_const(&self, name: &str) -> Option<&Type> {
        self.consts.get(name)
    }

    /// Recherche un enum par son nom
    #[allow(dead_code)]
    pub fn lookup_enum(&self, name: &str) -> Option<&EnumInfo> {
        self.enums.get(name)
    }

    /// Recherche une constante de classe (ex: MyClass::CONST_NAME)
    pub fn lookup_class_const(&self, class: &str, name: &str) -> Option<&(Type, Visibility)> {
        self.classes.get(class)?.class_consts.get(name)
    }

    /// Cherche un champ en remontant la chaîne d'héritage
    pub fn lookup_field_in_chain(&self, class_name: &str, field: &str) -> Option<&FieldInfo> {
        let mut current = class_name;
        loop {
            let info = self.classes.get(current)?;
            if let Some(f) = info.fields.get(field) {
                return Some(f);
            }
            match info.extends.as_deref() {
                Some(parent) => current = parent,
                None => return None,
            }
        }
    }

    /// Cherche une méthode en remontant la chaîne d'héritage
    pub fn lookup_method_in_chain(&self, class_name: &str, method: &str) -> Option<&FuncSig> {
        let mut current = class_name;
        loop {
            let info = self.classes.get(current)?;
            if let Some(m) = info.methods.get(method) {
                return Some(m);
            }
            match info.extends.as_deref() {
                Some(parent) => current = parent,
                None => return None,
            }
        }
    }

    /// Résoudre un type nommé → vérifie que la classe ou interface existe
    #[allow(dead_code)]
    pub fn type_exists(&self, name: &str) -> bool {
        self.classes.contains_key(name)
            || self.interfaces.contains_key(name)
            || matches!(
                name,
                "int" | "float" | "string" | "bool" | "mixed" | "void"
            )
    }
}
