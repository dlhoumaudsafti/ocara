/// Lowering du programme complet

use std::collections::{HashMap, HashSet};
use crate::parsing::ast::*;
use crate::ir::module::IrModule;
use crate::ir::types::IrType;
use super::functions::lower_const_global;
use super::functions::lower_func;
use super::classes::lower_class;
use super::runtime::lower_runtime_blocks;
use super::wrappers::{generate_wrapper, generate_async_wrapper};

pub fn lower_program(program: &Program, source_file: &str) -> IrModule {
    let module_name = "ocara_module".to_string();
    let mut module = IrModule::new(module_name);
    
    // Stocker le nom du fichier source pour les messages d'erreur
    module.source_file = source_file.to_string();

    // Enregistre les modules importés (dernier segment du path : "ocara.IO" → "IO")
    for imp in &program.imports {
        if let Some(last) = imp.path.last() {
            module.imports.push(last.clone());
        }
    }

    // Constantes globales → globals du module
    for c in &program.consts {
        lower_const_global(&mut module, c);
    }

    // Pré-collecte des types de retour de toutes les fonctions utilisateur
    let mut fn_ret_types: HashMap<String, IrType> = HashMap::new();
    for func in &program.functions {
        fn_ret_types.insert(func.name.clone(), IrType::from_ast(&func.ret_ty));
    }

    // Collecte des fonctions marquées async
    let mut async_funcs: HashSet<String> = HashSet::new();
    for func in &program.functions {
        if func.is_async {
            async_funcs.insert(func.name.clone());
        }
    }

    // Pré-collecte des types de paramètres (fonctions libres + méthodes statiques)
    // Uniquement les fonctions référençables comme type Function
    let mut fn_param_types: HashMap<String, Vec<IrType>> = HashMap::new();
    let mut fn_variadic_info: HashMap<String, (usize, IrType)> = HashMap::new();
    let mut func_default_args: HashMap<String, Vec<Option<Expr>>> = HashMap::new();
    
    for func in &program.functions {
        let param_types: Vec<IrType> = func.params.iter()
            .map(|p| IrType::from_ast(&p.ty))
            .collect();
        fn_param_types.insert(func.name.clone(), param_types);
        
        // Collecte des valeurs par défaut
        let default_args: Vec<Option<Expr>> = func.params.iter()
            .map(|p| p.default_value.clone())
            .collect();
        func_default_args.insert(func.name.clone(), default_args);
        
        // Si dernier paramètre est variadic, enregistrer les infos
        if let Some(last_param) = func.params.last() {
            if last_param.is_variadic {
                let fixed_count = func.params.len() - 1;
                let elem_ty = IrType::from_ast(&last_param.ty);
                fn_variadic_info.insert(func.name.clone(), (fixed_count, elem_ty));
            }
        }
    }
    
    for class in &program.classes {
        for member in &class.members {
            if let ClassMember::Method { decl, is_static, .. } = member {
                let mangled = format!("{}_{}", class.name, decl.name);
                
                if *is_static {
                    let param_types: Vec<IrType> = decl.params.iter()
                        .map(|p| IrType::from_ast(&p.ty))
                        .collect();
                    fn_param_types.insert(mangled.clone(), param_types);
                    
                    // Collecte des valeurs par défaut
                    let default_args: Vec<Option<Expr>> = decl.params.iter()
                        .map(|p| p.default_value.clone())
                        .collect();
                    func_default_args.insert(mangled.clone(), default_args);
                    
                    // Si dernier paramètre est variadic
                    if let Some(last_param) = decl.params.last() {
                        if last_param.is_variadic {
                            let fixed_count = decl.params.len() - 1;
                            let elem_ty = IrType::from_ast(&last_param.ty);
                            fn_variadic_info.insert(mangled, (fixed_count, elem_ty));
                        }
                    }
                } else {
                    // Méthodes d'instance : collecte des valeurs par défaut (sans self)
                    // self est géré séparément dans le lowering, donc on ne l'inclut pas ici
                    let default_args: Vec<Option<Expr>> = decl.params.iter()
                        .map(|p| p.default_value.clone())
                        .collect();
                    func_default_args.insert(mangled, default_args);
                }
            }
        }
    }

    // Enregistre les parents et construit les layouts (champs parents en premier)
    for class in &program.classes {
        if let Some(parent_name) = &class.extends {
            module.class_parents.insert(class.name.clone(), parent_name.clone());
        }
    }
    // Construction des layouts dans l'ordre (parents avant enfants) — on refait si besoin
    fn collect_fields(
        classes: &[ClassDecl],
        modules: &[ModuleDecl],
        class_name: &str,
    ) -> Vec<(String, IrType)> {
        let class = match classes.iter().find(|c| c.name == class_name) {
            Some(c) => c,
            None    => return vec![],
        };
        let mut fields = if let Some(parent) = &class.extends {
            collect_fields(classes, modules, parent)
        } else {
            vec![]
        };
        
        // Ajouter les champs des modules en premier
        for module_name in &class.modules {
            if let Some(module_decl) = modules.iter().find(|m| &m.name == module_name) {
                for member in &module_decl.members {
                    if let ClassMember::Field { name, ty, .. } = member {
                        // Éviter les doublons
                        if !fields.iter().any(|(f, _)| f == name) {
                            fields.push((name.clone(), IrType::from_ast(ty)));
                        }
                    }
                }
            }
        }
        
        // Puis ajouter les champs de la classe elle-même
        for member in &class.members {
            if let ClassMember::Field { name, ty, .. } = member {
                // Éviter les doublons
                if !fields.iter().any(|(f, _)| f == name) {
                    fields.push((name.clone(), IrType::from_ast(ty)));
                }
            }
        }
        fields
    }
    for class in &program.classes {
        let fields = collect_fields(&program.classes, &program.modules, &class.name);
        module.class_layouts.insert(class.name.clone(), fields);
    }

    // Ajouter les layouts des exceptions builtin (même structure pour toutes)
    let exception_layout = vec![
        ("message".to_string(), IrType::Ptr),  // offset 0
        ("code".to_string(), IrType::I64),      // offset 8
        ("source".to_string(), IrType::Ptr),    // offset 16
    ];
    module.class_layouts.insert("Exception".to_string(), exception_layout.clone());
    module.class_layouts.insert("FileException".to_string(), exception_layout.clone());
    module.class_layouts.insert("DirectoryException".to_string(), exception_layout.clone());
    module.class_layouts.insert("IOException".to_string(), exception_layout.clone());
    module.class_layouts.insert("SystemException".to_string(), exception_layout.clone());
    module.class_layouts.insert("ArrayException".to_string(), exception_layout.clone());
    module.class_layouts.insert("MapException".to_string(), exception_layout.clone());
    module.class_layouts.insert("MathException".to_string(), exception_layout.clone());
    module.class_layouts.insert("ConvertException".to_string(), exception_layout.clone());
    module.class_layouts.insert("RegexException".to_string(), exception_layout.clone());
    module.class_layouts.insert("DateTimeException".to_string(), exception_layout.clone());
    module.class_layouts.insert("DateException".to_string(), exception_layout.clone());
    module.class_layouts.insert("TimeException".to_string(), exception_layout.clone());
    module.class_layouts.insert("ThreadException".to_string(), exception_layout.clone());
    module.class_layouts.insert("MutexException".to_string(), exception_layout.clone());
    module.class_layouts.insert("UnitTestException".to_string(), exception_layout);

    // Collecte les types de paramètres des constructeurs (pour le boxing mixed)
    for class in &program.classes {
        if let Some(ctor_params) = class.members.iter().find_map(|m| {
            if let ClassMember::Constructor { params, .. } = m { Some(params) } else { None }
        }) {
            let param_types: Vec<IrType> = ctor_params.iter()
                .map(|p| IrType::from_ast(&p.ty))
                .collect();
            module.ctor_param_types.insert(class.name.clone(), param_types);
        }
    }

    // Collecte les constantes de classes pour inlining (Class::NAME)
    for class in &program.classes {
        for member in &class.members {
            if let ClassMember::Const { name, ty, value, .. } = member {
                if let Expr::Literal(lit, _) = value {
                    let key = format!("{}__{}", class.name, name);
                    module.class_consts.insert(key, (IrType::from_ast(ty), lit.clone()));
                }
            }
        }
    }

    // Collecte les variantes d'enum comme constantes int inlinables (Enum::Variant)
    for en in &program.enums {
        let mut next_val: i64 = 0;
        for v in &en.variants {
            let val = v.value.unwrap_or(next_val);
            next_val = val + 1;
            let key = format!("{}__{}", en.name, v.name);
            module.class_consts.insert(key, (IrType::I64, crate::parsing::ast::Literal::Int(val)));
        }
    }

    // Collecte les types de retour des méthodes propres
    for class in &program.classes {
        for member in &class.members {
            if let ClassMember::Method { decl, .. } = member {
                fn_ret_types.insert(
                    format!("{}_{}", class.name, decl.name),
                    IrType::from_ast(&decl.ret_ty),
                );
            }
        }
    }
    
    // Ajout des types de retour des méthodes builtin String
    // (utilisé pour le chaînage des appels comme a.trim().lower())
    fn_ret_types.insert("String_len".to_string(), IrType::I64);
    fn_ret_types.insert("String_upper".to_string(), IrType::Ptr);
    fn_ret_types.insert("String_lower".to_string(), IrType::Ptr);
    fn_ret_types.insert("String_capitalize".to_string(), IrType::Ptr);
    fn_ret_types.insert("String_trim".to_string(), IrType::Ptr);
    fn_ret_types.insert("String_replace".to_string(), IrType::Ptr);
    fn_ret_types.insert("String_split".to_string(), IrType::Ptr);
    fn_ret_types.insert("String_explode".to_string(), IrType::Ptr);
    fn_ret_types.insert("String_between".to_string(), IrType::Ptr);
    fn_ret_types.insert("String_empty".to_string(), IrType::Bool);
    
    // Ajout des types de retour des méthodes builtin Array
    // (utilisé pour le chaînage des appels comme arr.sort().reverse())
    fn_ret_types.insert("Array_len".to_string(), IrType::I64);
    fn_ret_types.insert("Array_push".to_string(), IrType::Void);
    fn_ret_types.insert("Array_pop".to_string(), IrType::Ptr);
    fn_ret_types.insert("Array_first".to_string(), IrType::Ptr);
    fn_ret_types.insert("Array_last".to_string(), IrType::Ptr);
    fn_ret_types.insert("Array_contains".to_string(), IrType::Bool);
    fn_ret_types.insert("Array_indexOf".to_string(), IrType::I64);
    fn_ret_types.insert("Array_reverse".to_string(), IrType::Ptr);  // Chainable
    fn_ret_types.insert("Array_slice".to_string(), IrType::Ptr);    // Chainable
    fn_ret_types.insert("Array_join".to_string(), IrType::Ptr);
    fn_ret_types.insert("Array_sort".to_string(), IrType::Ptr);     // Chainable
    fn_ret_types.insert("Array_get".to_string(), IrType::Ptr);
    fn_ret_types.insert("Array_set".to_string(), IrType::Void);
    
    // Ajout des types de retour des méthodes builtin Map
    // (utilisé pour le chaînage des appels si nécessaire)
    fn_ret_types.insert("Map_size".to_string(), IrType::I64);
    fn_ret_types.insert("Map_has".to_string(), IrType::Bool);
    fn_ret_types.insert("Map_get".to_string(), IrType::Ptr);
    fn_ret_types.insert("Map_set".to_string(), IrType::Void);
    fn_ret_types.insert("Map_remove".to_string(), IrType::Void);
    fn_ret_types.insert("Map_keys".to_string(), IrType::Ptr);
    fn_ret_types.insert("Map_values".to_string(), IrType::Ptr);
    fn_ret_types.insert("Map_merge".to_string(), IrType::Ptr);
    fn_ret_types.insert("Map_isEmpty".to_string(), IrType::Bool);
    
    // Propage les types de retour des méthodes héritées (non surchargées) dans fn_ret_types
    for class in &program.classes {
        if let Some(parent_name) = &class.extends {
            let own_methods: HashSet<String> = class.members.iter()
                .filter_map(|m| if let ClassMember::Method { decl, .. } = m { Some(decl.name.clone()) } else { None })
                .collect();
            if let Some(parent) = program.classes.iter().find(|c| &c.name == parent_name) {
                for member in &parent.members {
                    if let ClassMember::Method { decl, .. } = member {
                        if !own_methods.contains(&decl.name) {
                            let child_key  = format!("{}_{}", class.name, decl.name);
                            let parent_key = format!("{}_{}", parent_name, decl.name);
                            if let Some(ty) = fn_ret_types.get(&parent_key).cloned() {
                                fn_ret_types.insert(child_key, ty);
                            }
                        }
                    }
                }
            }
        }
    }

    // Génère les wrappers __fn_wrap_* pour toutes les fonctions/méthodes statiques
    // référençables comme type Function. Convention : wrapper(env_ptr, args...) { return orig(args) }
    {
        let names_to_wrap: Vec<(String, Vec<IrType>, IrType)> = fn_param_types.iter()
            .map(|(k, params)| {
                let ret_ty = fn_ret_types.get(k.as_str()).cloned().unwrap_or(IrType::I64);
                (k.clone(), params.clone(), ret_ty)
            })
            .collect();
        for (func_name, param_tys, ret_ty) in &names_to_wrap {
            let wrapper_name = format!("__fn_wrap_{}", func_name);
            generate_wrapper(&mut module, func_name, &wrapper_name, param_tys, ret_ty.clone(), &fn_ret_types);
        }
    }

    // Fonctions libres (les constantes sont inlinées dans chaque fonction)
    for func in &program.functions {
        lower_func(&mut module, func, &program.consts, &fn_ret_types, &fn_param_types, &fn_variadic_info, &func_default_args, None, &async_funcs);
        // Générer le wrapper async si la fonction est marquée async
        if func.is_async {
            let param_tys: Vec<IrType> = func.params.iter().map(|p| IrType::from_ast(&p.ty)).collect();
            let ret_ty = IrType::from_ast(&func.ret_ty);
            let wrapper_name = format!("__async_wrap_{}", func.name);
            generate_async_wrapper(&mut module, &func.name, &wrapper_name, &param_tys, ret_ty, &fn_ret_types);
        }
    }

    // Méthodes de classes (passe toutes les classes pour l'héritage)
    for class in &program.classes {
        lower_class(&mut module, class, &program.classes, &program.modules, &program.consts, &fn_ret_types, &fn_param_types, &fn_variadic_info, &func_default_args, &async_funcs);
    }

    // Blocs runtime → fonctions __init__, __main__, etc.
    lower_runtime_blocks(&mut module, program, &program.consts, &fn_ret_types, &fn_param_types, &fn_variadic_info, &func_default_args, &async_funcs);

    module
}
