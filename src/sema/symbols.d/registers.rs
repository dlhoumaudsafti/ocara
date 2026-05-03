/// Fonctions d'enregistrement (register_*) dans la table des symboles
use super::types::{
    ClassInfo,
    EnumInfo,
    FieldInfo,
    FuncSig,
    GenericInfo,
    InterfaceInfo,
    ModuleInfo,
    SymbolTable,
    ImportInfo,
};
use crate::parsing::ast::{
    ClassDecl,
    ConstDecl,
    EnumDecl,
    FuncDecl,
    ImportDecl,
    InterfaceDecl,
    ModuleDecl,
    Type,
    Visibility,
};
use std::collections::HashMap;

// Import des helpers depuis le module helpers
use super::helpers::{
    params_to_vec,
    has_variadic_param,
    fixed_params_count,
    required_params_count,
};

impl SymbolTable {
    /// Recherche une fonction par son nom
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

        // Import depuis un fichier (nouveau format "from") : sera résolu dans main.rs
        // On ne crée pas de classe opaque car les classes seront fusionnées
        if decl.file_path.is_some() {
            return;
        }

        // Import namespace non-ocara : sera chargé dans main.rs (depuis la v0.1.2)
        // On ne crée pas de classe opaque car les classes seront chargées
        if !is_ocara {
            self.imports.push(ImportInfo {
                path:  decl.path.clone(),
                alias: decl.alias.clone(),
            });
            return;
        }

        // Import ordinaire : classe opaque (ne devrait plus arriver sauf bugs)
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
                is_async:  decl.is_async,
                has_variadic: has_variadic_param(&decl.params),
                fixed_params_count: fixed_params_count(&decl.params),
                required_params_count: required_params_count(&decl.params),
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
        is_async:  false,
                    has_variadic: has_variadic_param(&m.params),
                    fixed_params_count: fixed_params_count(&m.params),
                    required_params_count: required_params_count(&m.params),
                },
            );
        }
        self.interfaces.insert(decl.name.clone(), InterfaceInfo { methods });
        true
    }

    pub fn register_module(&mut self, decl: &ModuleDecl) -> bool {
        use crate::parsing::ast::ClassMember;
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
        is_async:  false,
                        has_variadic: has_variadic_param(&fd.params),
                        fixed_params_count: fixed_params_count(&fd.params),
                        required_params_count: required_params_count(&fd.params),
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

    pub fn register_enum(&mut self, decl: &EnumDecl) -> bool {
        if self.enums.contains_key(&decl.name) || self.classes.contains_key(&decl.name) {
            return false;
        }
        // Calculer les valeurs : auto-incrémentation depuis 0, ou valeur explicite
        let mut next_val: i64 = 0;
        let mut variants = HashMap::new();
        let mut class_consts = HashMap::new();

        for v in &decl.variants {
            let val = v.value.unwrap_or(next_val);
            next_val = val + 1;
            variants.insert(v.name.clone(), val);
            class_consts.insert(v.name.clone(), (Type::Int, Visibility::Public));
        }

        self.enums.insert(decl.name.clone(), EnumInfo { variants });
        // Enregistrer aussi comme "classe" avec class_consts pour que
        // StaticConst `Enum::Variant` soit résolu par la sema existante
        self.classes.insert(decl.name.clone(), ClassInfo {
            extends:      None,
            implements:   vec![],
            fields:       HashMap::new(),
            methods:      HashMap::new(),
            class_consts,
            is_opaque:    false,
        });
        true
    }

    pub fn register_class(&mut self, decl: &ClassDecl) -> bool {        use crate::parsing::ast::ClassMember;
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
        is_async:  false,
                        has_variadic: has_variadic_param(&fd.params),
                        fixed_params_count: fixed_params_count(&fd.params),
                        required_params_count: required_params_count(&fd.params),
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

    pub fn register_generic(&mut self, decl: &crate::parsing::ast::GenericDecl) -> bool {
        use crate::parsing::ast::ClassMember;
        if self.generics.contains_key(&decl.name) {
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
                        is_async:  false,
                        has_variadic: has_variadic_param(&fd.params),
                        fixed_params_count: fixed_params_count(&fd.params),
                        required_params_count: required_params_count(&fd.params),
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
        }

        self.generics.insert(
            decl.name.clone(),
            GenericInfo {
                type_params: decl.type_params.clone(),
                extends:     decl.extends.clone(),
                extends_args: decl.extends_args.clone(),
                implements:  decl.implements.clone(),
                fields,
                methods,
                class_consts,
            },
        );
        true
    }
}