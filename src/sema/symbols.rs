use std::collections::HashMap;
use crate::ast::{
    ClassDecl, ModuleDecl, FuncDecl, InterfaceDecl, ImportDecl, ConstDecl, Type, Param, Visibility,
};

// ─────────────────────────────────────────────────────────────────────────────
// Table des symboles globaux
// ─────────────────────────────────────────────────────────────────────────────

/// Signature d'une fonction (méthode ou fonction libre)
#[derive(Debug, Clone)]
pub struct FuncSig {
    pub params:    Vec<(String, Type)>,
    pub ret_ty:    Type,
    pub is_static: bool,
}

/// Descripteur d'un champ de classe
#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub ty:      Type,
    pub mutable: bool,
    pub vis:     Visibility,
}

/// Descripteur complet d'une classe enregistrée
#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub extends:      Option<String>,
    pub implements:   Vec<String>,
    pub fields:       HashMap<String, FieldInfo>,
    pub methods:      HashMap<String, FuncSig>,
    pub class_consts: HashMap<String, (Type, Visibility)>,
    /// Vrai si la classe est issue d'un import non résolu (accès opaques autorisés)
    pub is_opaque:    bool,
}

/// Descripteur complet d'un module (mixin)
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub fields:       HashMap<String, FieldInfo>,
    pub methods:      HashMap<String, FuncSig>,
    pub class_consts: HashMap<String, (Type, Visibility)>,
}

/// Descripteur d'une interface enregistrée
#[derive(Debug, Clone)]
pub struct InterfaceInfo {
    pub methods: HashMap<String, FuncSig>,
}

/// Descripteur d'un import
#[derive(Debug, Clone)]
pub struct ImportInfo {
    pub path:  Vec<String>,
    pub alias: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// SymbolTable
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct SymbolTable {
    pub functions:  HashMap<String, FuncSig>,
    pub modules:    HashMap<String, ModuleInfo>,
    pub classes:    HashMap<String, ClassInfo>,
    pub interfaces: HashMap<String, InterfaceInfo>,
    pub consts:     HashMap<String, Type>,
    pub imports:    Vec<ImportInfo>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    // ── Enregistrement ───────────────────────────────────────────────────────

    pub fn register_import(&mut self, decl: &ImportDecl) {
        let is_ocara = decl.path.first().map(|s| s == "ocara").unwrap_or(false);

        if is_ocara {
            // `import ocara.*` → enregistre toutes les classes builtins
            let last = decl.path.last().map(|s| s.as_str()).unwrap_or("");
            if last == "*" {
                for (name, info) in crate::builtins::all_builtins() {
                    self.classes.entry(name.to_string()).or_insert(info);
                }
                return;
            }
            // `import ocara.String` ou `import ocara.String as Alias`
            let class_name = decl.alias.as_ref().cloned().unwrap_or_else(|| last.to_string());
            if let Some(info) = crate::builtins::builtin_class(last) {
                self.classes.entry(class_name.clone()).or_insert(info);
                self.imports.push(ImportInfo { path: decl.path.clone(), alias: decl.alias.clone() });
                return;
            }
        }

        // Import ordinaire : classe opaque
        let alias = decl
            .alias
            .as_ref()
            .cloned()
            .unwrap_or_else(|| decl.path.last().cloned().unwrap_or_default());
        self.classes.entry(alias).or_insert_with(|| ClassInfo {
            extends:      None,
            implements:   vec![],
            fields:       HashMap::new(),
            methods:      HashMap::new(),
            class_consts: HashMap::new(),
            is_opaque:    true,
        });
        self.imports.push(ImportInfo {
            path:  decl.path.clone(),
            alias: decl.alias.clone(),
        });
    }

    pub fn register_const(&mut self, decl: &ConstDecl) -> bool {
        if self.consts.contains_key(&decl.name) {
            return false;
        }
        self.consts.insert(decl.name.clone(), decl.ty.clone());
        true
    }

    pub fn register_function(&mut self, decl: &FuncDecl) -> bool {
        if self.functions.contains_key(&decl.name) {
            return false;
        }
        self.functions.insert(
            decl.name.clone(),
            FuncSig {
                params:    params_to_vec(&decl.params),
                ret_ty:    decl.ret_ty.clone(),
                is_static: false,
            },
        );
        true
    }

    pub fn register_interface(&mut self, decl: &InterfaceDecl) -> bool {
        if self.interfaces.contains_key(&decl.name) {
            return false;
        }
        let mut methods = HashMap::new();
        for m in &decl.methods {
            methods.insert(
                m.name.clone(),
                FuncSig {
                    params:    params_to_vec(&m.params),
                    ret_ty:    m.ret_ty.clone(),
                    is_static: false,
                },
            );
        }
        self.interfaces.insert(decl.name.clone(), InterfaceInfo { methods });
        true
    }

    pub fn register_module(&mut self, decl: &ModuleDecl) -> bool {
        use crate::ast::ClassMember;
        if self.modules.contains_key(&decl.name) {
            return false;
        }
        let mut fields       = HashMap::new();
        let mut methods      = HashMap::new();
        let mut class_consts = HashMap::new();

        for member in &decl.members {
            match member {
                ClassMember::Field { vis, mutable, name, ty, .. } => {
                    fields.insert(name.clone(), FieldInfo {
                        ty:      ty.clone(),
                        mutable: *mutable,
                        vis:     vis.clone(),
                    });
                }
                ClassMember::Const { vis, name, ty, .. } => {
                    class_consts.insert(name.clone(), (ty.clone(), vis.clone()));
                }
                ClassMember::Method { vis: _, is_static, decl: fd, .. } => {
                    methods.insert(fd.name.clone(), FuncSig {
                        params:    params_to_vec(&fd.params),
                        ret_ty:    fd.ret_ty.clone(),
                        is_static: *is_static,
                    });
                }
                ClassMember::Constructor { .. } => {
                    // Les modules ne peuvent pas avoir de constructeurs
                    // On ignore silencieusement ou on pourrait générer une erreur
                }
            }
        }

        self.modules.insert(
            decl.name.clone(),
            ModuleInfo {
                fields,
                methods,
                class_consts,
            },
        );
        true
    }

    pub fn register_class(&mut self, decl: &ClassDecl) -> bool {
        use crate::ast::ClassMember;
        if self.classes.contains_key(&decl.name) {
            return false;
        }
        let mut fields       = HashMap::new();
        let mut methods      = HashMap::new();
        let mut class_consts = HashMap::new();

        for member in &decl.members {
            match member {
                ClassMember::Field { vis, mutable, name, ty, .. } => {
                    fields.insert(name.clone(), FieldInfo {
                        ty:      ty.clone(),
                        mutable: *mutable,
                        vis:     vis.clone(),
                    });
                }
                ClassMember::Const { vis, name, ty, .. } => {
                    class_consts.insert(name.clone(), (ty.clone(), vis.clone()));
                }
                ClassMember::Method { vis: _, is_static, decl: fd, .. } => {
                    methods.insert(fd.name.clone(), FuncSig {
                        params:    params_to_vec(&fd.params),
                        ret_ty:    fd.ret_ty.clone(),
                        is_static: *is_static,
                    });
                }
                ClassMember::Constructor { .. } => {}
            }
        }

        // Composer les membres des modules (mixins)
        for module_name in &decl.modules {
            if let Some(module_info) = self.modules.get(module_name).cloned() {
                // Ajouter les champs du module
                for (name, field) in module_info.fields {
                    fields.entry(name).or_insert(field);
                }
                // Ajouter les méthodes du module
                for (name, method) in module_info.methods {
                    methods.entry(name).or_insert(method);
                }
                // Ajouter les constantes du module
                for (name, const_info) in module_info.class_consts {
                    class_consts.entry(name).or_insert(const_info);
                }
            }
            // Si le module n'existe pas, on pourrait générer une erreur plus tard
        }

        self.classes.insert(
            decl.name.clone(),
            ClassInfo {
                extends:      decl.extends.clone(),
                implements:   decl.implements.clone(),
                fields,
                methods,
                class_consts,
                is_opaque:    false,
            },
        );
        true
    }

    // ── Requêtes ─────────────────────────────────────────────────────────────

    pub fn lookup_function(&self, name: &str) -> Option<&FuncSig> {
        self.functions.get(name)
    }

    pub fn lookup_class(&self, name: &str) -> Option<&ClassInfo> {
        self.classes.get(name)
    }

    #[allow(dead_code)]
    pub fn lookup_module(&self, name: &str) -> Option<&ModuleInfo> {
        self.modules.get(name)
    }

    #[allow(dead_code)]
    pub fn lookup_interface(&self, name: &str) -> Option<&InterfaceInfo> {
        self.interfaces.get(name)
    }

    pub fn lookup_const(&self, name: &str) -> Option<&Type> {
        self.consts.get(name)
    }

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

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn params_to_vec(params: &[Param]) -> Vec<(String, Type)> {
    params.iter().map(|p| (p.name.clone(), p.ty.clone())).collect()
}
