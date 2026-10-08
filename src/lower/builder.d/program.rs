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

/// Nom de "classe" concret d'un type de retour DÉCLARÉ (fonction libre OU
/// méthode, statique ou d'instance) — classe utilisateur, famille builtin
/// `"String"`/`"Array"`/`"Map"`, ou nom monomorphisé d'un générique. SEULE
/// source de vérité pour peupler `IrModule::func_ret_class`/
/// `IrModule::method_ret_class`, consultée par
/// `crate::lower::expr::helpers::resolve_receiver_class` pour résoudre un
/// appel/accès chaîné à une profondeur arbitraire — voir
/// docs/roadmap.d/langage-chained-call-depth-limit.md. `Type::Generic` inclus
/// (contrairement à l'ancienne copie de cette logique, propre à
/// `func_ret_class`, qui ne le gérait pas) : même résolution que pour un
/// CHAMP de type générique (voir `resolve_chained_field_class`) — un
/// oubli ici referait la même classe de bug pour une fonction/méthode
/// retournant un générique plutôt qu'un champ.
fn concrete_return_class(ty: &Type) -> Option<String> {
    match ty {
        Type::Named(n)   => Some(n.clone()),
        Type::String     => Some("String".to_string()),
        Type::Array(_)   => Some("Array".to_string()),
        Type::Map(_, _)  => Some("Map".to_string()),
        Type::Generic { name, args } => Some(crate::core::monomorph::monomorphized_name(name, args)),
        _ => None,
    }
}

pub fn lower_program(program: &Program, source_file: &str) -> IrModule {
    let module_name = "ocara_module".to_string();
    let mut module = IrModule::new(module_name);
    
    // Stocker le nom du fichier source pour les messages d'erreur
    module.source_file = source_file.to_string();

    // Enregistre les modules importés (dernier segment du path : "ocara.IO" → "IO")
    for imp in &program.imports {
        if let Some(last) = imp.path.last() {
            module.imports.push(last.clone());
            // Alias d'un builtin natif (`import ocara.X as Y`) : mémoriser
            // Y → X pour que `Expr::StaticCall` puisse mangler vers le vrai
            // symbole runtime plutôt que vers "Y_method" (voir doc du champ).
            let is_ocara = imp.path.first().map(|s| s == "ocara").unwrap_or(false);
            if is_ocara {
                if let Some(alias) = &imp.alias {
                    module.import_aliases.insert(alias.clone(), last.clone());
                }
            }
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
        // Nom de "classe" concret du retour (classe utilisateur OU famille
        // builtin string/array/map, même convention que
        // `resolve_chained_field_class` pour un champ) — voir la doc de
        // `IrModule::func_ret_class` et
        // docs/roadmap.d/langage-chained-call-on-free-function-result.md.
        // `array<T>`/`map<K,V>` INCLUS : sans ça, `maFonction().len()` (une
        // fonction libre retournant un tableau, méthode builtin ensuite
        // chaînée dessus) échouait exactement de la même façon qu'une
        // classe utilisateur — confirmé par reproduction.
        if let Some(name) = concrete_return_class(&func.ret_ty) {
            module.func_ret_class.insert(func.name.clone(), name);
        }
        module.call_ret_types.insert(func.name.clone(), func.ret_ty.clone());
    }

    // Collecte des fonctions marquées async
    let mut async_funcs: HashSet<String> = HashSet::new();
    for func in &program.functions {
        if func.is_async {
            async_funcs.insert(func.name.clone());
        }
    }
    
    // Collecte des méthodes async de classes
    for class in &program.classes {
        for member in &class.members {
            if let ClassMember::Method { decl, .. } = member {
                if decl.is_async {
                    let mangled = format!("{}_{}", class.name, decl.name);
                    async_funcs.insert(mangled);
                }
            }
        }
    }

    // Pré-collecte des types de paramètres (fonctions libres + méthodes statiques)
    // Uniquement les fonctions référençables comme type Function
    let mut fn_param_types: HashMap<String, Vec<IrType>> = HashMap::new();
    let mut fn_param_names: HashMap<String, Vec<String>> = HashMap::new();
    let mut fn_variadic_info: HashMap<String, (usize, IrType)> = HashMap::new();
    let mut func_default_args: HashMap<String, Vec<Option<Expr>>> = HashMap::new();

    for func in &program.functions {
        let param_types: Vec<IrType> = func.params.iter()
            .map(|p| IrType::from_ast(&p.ty))
            .collect();
        fn_param_types.insert(func.name.clone(), param_types);
        fn_param_names.insert(func.name.clone(), func.params.iter().map(|p| p.name.clone()).collect());

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

                // Nom de "classe" concret du retour (voir `IrModule::method_ret_class`)
                // — statique ET d'instance, la seule source de vérité pour résoudre un
                // appel de méthode chaîné (`w.getCircle().shapeName()`) à une profondeur
                // arbitraire. Voir docs/roadmap.d/langage-chained-call-depth-limit.md.
                if let Some(name) = concrete_return_class(&decl.ret_ty) {
                    module.method_ret_class.insert(mangled.clone(), name);
                }
                module.call_ret_types.insert(mangled.clone(), decl.ret_ty.clone());

                if *is_static {
                    let param_types: Vec<IrType> = decl.params.iter()
                        .map(|p| IrType::from_ast(&p.ty))
                        .collect();
                    fn_param_types.insert(mangled.clone(), param_types);
                    fn_param_names.insert(mangled.clone(), decl.params.iter().map(|p| p.name.clone()).collect());

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
                    // Méthodes d'instance : types de paramètres (sans `self`,
                    // géré séparément dans le lowering) — dans
                    // `module.method_param_types`, PAS `fn_param_types` (voir
                    // sa doc dans `src/ir/module.rs`). Uniquement pour la
                    // décision de boxing `mixed` d'un argument d'appel.
                    let param_types: Vec<IrType> = decl.params.iter()
                        .map(|p| IrType::from_ast(&p.ty))
                        .collect();
                    module.method_param_types.insert(mangled.clone(), param_types);

                    // Méthode d'instance variadic (sans `self`, ajouté à part
                    // au site d'appel) — voir `pack_variadic_args`.
                    if let Some(last_param) = decl.params.last() {
                        if last_param.is_variadic {
                            fn_variadic_info.insert(mangled.clone(), (decl.params.len() - 1, IrType::from_ast(&last_param.ty)));
                        }
                    }

                    // Collecte des valeurs par défaut (sans self)
                    let default_args: Vec<Option<Expr>> = decl.params.iter()
                        .map(|p| p.default_value.clone())
                        .collect();
                    func_default_args.insert(mangled, default_args);
                }
            }
        }
        // Constructeur (hérité compris) : complété par `Expr::New` comme
        // n'importe quel appel (`use P(1)` pour `init(a:int, b:int = 9)`).
        if let Some((params, _, _)) = super::classes::nearest_constructor(&program.classes, class) {
            let default_args: Vec<Option<Expr>> = params.iter()
                .map(|p| p.default_value.clone())
                .collect();
            func_default_args.insert(format!("{}_init", class.name), default_args);
        }
    }

    // Méthodes des classes builtin (`Convert::strToArray(s, ",").len()`,
    // `HTML::...`) : même table, sans jamais écraser une classe utilisateur
    // homonyme déjà enregistrée ci-dessus.
    for (class_name, info) in crate::builtins::all_builtins() {
        for (method_name, sig) in &info.methods {
            let mangled = format!("{}_{}", class_name, method_name);
            if let Some(name) = concrete_return_class(&sig.ret_ty) {
                module.method_ret_class.entry(mangled.clone()).or_insert(name);
            }
            module.call_ret_types.entry(mangled).or_insert_with(|| sig.ret_ty.clone());
        }
    }

    // Même collecte que ci-dessus, mais pour les méthodes déclarées par une
    // `interface` (mangled "Interface_méthode", ex. "Shape_area") — SANS
    // CELA, un site d'appel `s.méthode()` où `s` est typé par une interface
    // (polymorphisme classique via `implements`, RIEN À VOIR avec `wiring`)
    // ne trouvait ni type de paramètre ni type de retour pour la fonction
    // RÉELLEMENT appelée (le dispatcher `Interface_méthode` généré par
    // `generate_interface_dispatchers`, voir `interfaces.rs` — un mécanisme
    // préexistant à `wiring`, déjà correctement branché sur CE chemin
    // d'appel : `class_dispatcher_name` (héritage de classe) renvoie `None`
    // pour un nom d'interface, donc `call_target` retombe déjà sur
    // `func_mangled` = "Interface_méthode" directement). `fn_ret_types` (une
    // classe ordinaire n'obtenait pas non plus d'entrée pour un défaut
    // "Ptr" au hasard : `program.classes` ne contenait simplement jamais de
    // membre nommé "Shape") retombait donc sur son défaut `IrType::Ptr`,
    // alors que le dispatcher réellement généré (`generate_interface_dispatchers`)
    // respecte lui le VRAI type de retour déclaré (ex. `F64` pour
    // `area(): float`) — un désaccord d'ABI Cranelift au site d'appel
    // (convention d'appel entière vs flottante selon le type), confirmé
    // SIGSEGV par reproduction minimale (`var s:Shape = c; s.area()`,
    // aucun `wiring` en jeu) et par backtrace gdb (crash dans
    // `ocara_runtime::val_to_string`, appelé avec une valeur garbage —
    // pas dans le dispatch lui-même, qui identifie pourtant la bonne classe
    // concrète et retourne la bonne valeur F64, simplement mal réinterprétée
    // au retour de l'appel). Voir docs/roadmap.d/langage-interface-instance-dispatch-segfault.md.
    for iface in &program.interfaces {
        for method in &iface.methods {
            let mangled = format!("{}_{}", iface.name, method.name);
            fn_ret_types.insert(mangled.clone(), IrType::from_ast(&method.ret_ty));
            if let Some(name) = concrete_return_class(&method.ret_ty) {
                module.method_ret_class.insert(mangled.clone(), name);
            }

            if method.is_static {
                let param_types: Vec<IrType> = method.params.iter()
                    .map(|p| IrType::from_ast(&p.ty))
                    .collect();
                fn_param_types.insert(mangled.clone(), param_types);
                fn_param_names.insert(mangled.clone(), method.params.iter().map(|p| p.name.clone()).collect());

                let default_args: Vec<Option<Expr>> = method.params.iter()
                    .map(|p| p.default_value.clone())
                    .collect();
                func_default_args.insert(mangled.clone(), default_args);

                if let Some(last_param) = method.params.last() {
                    if last_param.is_variadic {
                        let fixed_count = method.params.len() - 1;
                        let elem_ty = IrType::from_ast(&last_param.ty);
                        fn_variadic_info.insert(mangled, (fixed_count, elem_ty));
                    }
                }
            } else {
                let param_types: Vec<IrType> = method.params.iter()
                    .map(|p| IrType::from_ast(&p.ty))
                    .collect();
                module.method_param_types.insert(mangled.clone(), param_types);

                let default_args: Vec<Option<Expr>> = method.params.iter()
                    .map(|p| p.default_value.clone())
                    .collect();
                func_default_args.insert(mangled, default_args);
            }
        }
    }

    // Enregistre les parents et construit les layouts (champs parents en premier)
    for class in &program.classes {
        if let Some(parent_name) = &class.extends {
            module.class_parents.insert(class.name.clone(), parent_name.clone());
        }
    }

    // Identité de classe à l'exécution (voir IrModule::class_ids) : un id
    // entier unique par classe, attribué dans l'ordre de déclaration — 0 est
    // réservé (jamais attribué) pour rester un sentinel "aucune classe"
    // détectable sans ambiguïté.
    for (i, class) in program.classes.iter().enumerate() {
        module.class_ids.insert(class.name.clone(), (i + 1) as i64);
    }

    // Doit être calculé ICI, AVANT le lowering du moindre corps de
    // fonction/méthode ci-dessous : chaque site d'appel de méthode consulte
    // `module.classes_with_subclasses` pour décider s'il doit rediriger vers
    // un dispatcher dynamique (voir `class_dispatch::class_dispatcher_name`,
    // `src/lower/expr.d/lower.rs`) — les CORPS de ces dispatchers ne sont
    // générés que bien plus tard (`generate_class_dispatchers`), une fois
    // les méthodes concrètes lowered, mais l'ENSEMBLE des classes qui EN
    // auront un doit déjà être connu.
    super::class_dispatch::compute_classes_with_subclasses(&mut module, program);

    // `async_funcs` doit aussi connaître le nom des DISPATCHERS dynamiques
    // (`__dispatch_Classe_méthode` pour l'héritage, `Interface_méthode` pour
    // une interface) quand la méthode qu'ils multiplexent est `async` — voir
    // docs/roadmap.d/langage-async-instance-method-dispatch-broken.md. Le
    // dispatcher lui-même reste une fonction 100% SYNCHRONE (il choisit la
    // bonne implémentation concrète par `__class_id` puis l'appelle
    // directement, exactement comme pour une méthode non-async) : c'est son
    // PROPRE wrapper async (`__async_wrap_<dispatcher>`, généré plus bas,
    // une fois `generate_class_dispatchers`/`generate_interface_dispatchers`
    // passées) qui doit exister pour que le site d'appel (`lower.rs`) puisse
    // le spawn — sans cette entrée ICI, AVANT le lowering du moindre corps
    // (même contrainte d'ordre que `classes_with_subclasses` ci-dessus, un
    // site d'appel peut être lowered avant que le dispatcher n'existe),
    // `builder.async_funcs.contains(dispatcher_name)` répondrait toujours
    // `false` pour un appel polymorphe, retombant sur l'appel synchrone
    // direct cassé (même symptôme que le bug d'origine).
    for class in &program.classes {
        if !module.classes_with_subclasses.contains(&class.name) {
            continue;
        }
        for method_name in super::class_dispatch::callable_instance_method_names(&class.name, &program.classes) {
            if let Some(decl) = super::class_dispatch::find_method_decl(&class.name, &method_name, &program.classes) {
                if decl.is_async {
                    async_funcs.insert(format!("__dispatch_{}_{}", class.name, method_name));
                }
            }
        }
    }
    for iface in &program.interfaces {
        let has_implementer = program.classes.iter().any(|c| c.implements.iter().any(|i| i == &iface.name));
        if !has_implementer {
            continue;
        }
        for method in &iface.methods {
            // Une méthode STATIQUE n'a jamais de dispatcher (voir
            // `generate_interface_dispatchers`, qui les saute déjà —
            // docs/roadmap.d/langage-interface-wiring.md) : un appel
            // statique async est déjà résolu vers la classe concrète avant
            // le lowering (`wiring`) ou rejeté (E38), jamais via ce chemin.
            if method.is_async && !method.is_static {
                async_funcs.insert(format!("{}_{}", iface.name, method.name));
            }
        }
    }

    // Générateurs (`emit`/`message<T>`) : même contrainte d'ordre — un site
    // de consommation peut être lowered AVANT la fonction/méthode qui
    // déclare le générateur (voir la doc de
    // `message_gen::register_all_message_funcs`).
    super::message_gen::register_all_message_funcs(&mut module, program);

    // Candidats d'un `is ClassName`/`is InterfaceName` réel (voir
    // IrModule::is_check_candidates) — même contrainte d'ordre que
    // `compute_classes_with_subclasses` ci-dessus : nécessaire AVANT le
    // lowering du moindre corps, chaque `is` étant vérifié à son site.
    for class in &program.classes {
        let mut ids = vec![module.class_ids.get(&class.name).copied().unwrap_or(0)];
        for descendant in super::class_dispatch::transitive_descendants(&class.name, &program.classes) {
            ids.push(module.class_ids.get(&descendant).copied().unwrap_or(0));
        }
        module.is_check_candidates.insert(class.name.clone(), ids);
    }
    for iface in &program.interfaces {
        let ids: Vec<i64> = program.classes.iter()
            .filter(|c| c.implements.iter().any(|i| i == &iface.name))
            .map(|c| module.class_ids.get(&c.name).copied().unwrap_or(0))
            .collect();
        module.is_check_candidates.insert(iface.name.clone(), ids);
    }

    // Hiérarchie des exceptions builtin : toutes "héritent" de `Exception`
    // pour le filtrage `on e is X` (voir IrModule::ancestor_chain, lower_raise,
    // et docs/roadmap.d/langage-exceptions.md) — `.entry(...).or_insert` pour
    // ne jamais écraser un `extends` explicite d'une classe utilisateur qui
    // porterait le même nom (cas limite improbable, mais gratuit à éviter).
    for name in crate::builtins::exception::BUILTIN_EXCEPTION_NAMES {
        module.class_parents.entry((*name).to_string()).or_insert_with(|| "Exception".to_string());
    }
    
    // Construction des layouts dans l'ordre (parents avant enfants) — on refait si besoin
    fn collect_fields(
        classes: &[ClassDecl],
        modules: &[ModuleDecl],
        class_layouts: &HashMap<String, Vec<(String, IrType)>>,
        class_name: &str,
    ) -> Vec<(String, IrType)> {
        let class = match classes.iter().find(|c| c.name == class_name) {
            Some(c) => c,
            None    => {
                // Si pas dans classes utilisateur, chercher dans les builtin layouts
                return class_layouts.get(class_name).cloned().unwrap_or_default();
            }
        };
        let mut fields = if let Some(parent) = &class.extends {
            collect_fields(classes, modules, class_layouts, parent)
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
    module.class_layouts.insert("UnitTestException".to_string(), exception_layout.clone());
    module.class_layouts.insert("HTTPServerException".to_string(), exception_layout.clone());
    module.class_layouts.insert("TauriException".to_string(), exception_layout.clone());
    module.class_layouts.insert("SDLException".to_string(), exception_layout.clone());
    module.class_layouts.insert("SQLiteException".to_string(), exception_layout.clone());
    module.class_layouts.insert("MySQLException".to_string(), exception_layout.clone());
    module.class_layouts.insert("MariaDBException".to_string(), exception_layout.clone());
    module.class_layouts.insert("DotEnvException".to_string(), exception_layout.clone());
    module.class_layouts.insert("YAMLException".to_string(), exception_layout);
    super::rc_layout::compute_rc_objects(&mut module, program);

    // Ajouter les layouts des builtins opaques (pointeur vers structure Rust)
    // Ces classes ont un constructeur _init qui alloue une structure opaque
    let opaque_layout = vec![("__opaque_ptr".to_string(), IrType::Ptr)];
    module.class_layouts.insert("HTTPServer".to_string(), opaque_layout.clone());
    module.class_layouts.insert("Thread".to_string(), opaque_layout.clone());
    module.class_layouts.insert("Mutex".to_string(), opaque_layout.clone());
    module.class_layouts.insert("HTMLComponent".to_string(), opaque_layout);
    
    // Construire les layouts des classes utilisateur (APRÈS les builtins)
    for class in &program.classes {
        let fields = collect_fields(&program.classes, &program.modules, &module.class_layouts, &class.name);
        module.class_layouts.insert(class.name.clone(), fields);
    }

    // Comme collect_fields ci-dessus, mais garde le vrai Type AST par champ
    // (pas IrType) — voir la doc de IrModule::class_field_types. Volontairement
    // limité aux classes utilisateur : si le parent (`extends`) n'est PAS
    // dans `classes` (parent builtin, ex: exception personnalisée), on
    // s'arrête là plutôt que d'inventer un type — ces champs hérités ne
    // seront simplement pas libérés/clonés récursivement (fuite, pas un bug).
    fn collect_field_types(
        classes: &[ClassDecl],
        modules: &[ModuleDecl],
        class_name: &str,
    ) -> Vec<(String, Type)> {
        let class = match classes.iter().find(|c| c.name == class_name) {
            Some(c) => c,
            None    => return vec![],
        };
        let mut fields = if let Some(parent) = &class.extends {
            collect_field_types(classes, modules, parent)
        } else {
            vec![]
        };
        for module_name in &class.modules {
            if let Some(module_decl) = modules.iter().find(|m| &m.name == module_name) {
                for member in &module_decl.members {
                    if let ClassMember::Field { name, ty, .. } = member {
                        if !fields.iter().any(|(f, _)| f == name) {
                            fields.push((name.clone(), ty.clone()));
                        }
                    }
                }
            }
        }
        for member in &class.members {
            if let ClassMember::Field { name, ty, .. } = member {
                if !fields.iter().any(|(f, _)| f == name) {
                    fields.push((name.clone(), ty.clone()));
                }
            }
        }
        fields
    }
    for class in &program.classes {
        let fields = collect_field_types(&program.classes, &program.modules, &class.name);
        module.class_field_types.insert(class.name.clone(), fields);
    }

    // Types AST des paramètres (littéral passé en argument, voir `lower_call_arg`).
    for func in &program.functions {
        module.param_ast_types.insert(func.name.clone(), func.params.iter().map(|p| p.ty.clone()).collect());
    }
    for class in &program.classes {
        for member in &class.members {
            if let ClassMember::Method { decl, .. } = member {
                module.param_ast_types.insert(format!("{}_{}", class.name, decl.name), decl.params.iter().map(|p| p.ty.clone()).collect());
            }
        }
        if let Some((ctor_params, _, _)) = super::classes::nearest_constructor(&program.classes, class) {
            module.param_ast_types.insert(format!("{}_init", class.name), ctor_params.iter().map(|p| p.ty.clone()).collect());
        }
    }

    // Champs de type map<K,V> par classe (hérités inclus) — voir la doc du champ
    // module.class_map_fields (ir/module.rs) : indispensable pour que
    // `self.champMap[clé] = v` émette __map_set plutôt que __array_set.
    fn collect_map_fields(classes: &[ClassDecl], class_name: &str) -> HashSet<String> {
        let class = match classes.iter().find(|c| c.name == class_name) {
            Some(c) => c,
            None    => return HashSet::new(),
        };
        let mut fields = if let Some(parent) = &class.extends {
            collect_map_fields(classes, parent)
        } else {
            HashSet::new()
        };
        for member in &class.members {
            if let ClassMember::Field { name, ty: Type::Map(_, _), .. } = member {
                fields.insert(name.clone());
            }
        }
        fields
    }
    for class in &program.classes {
        let map_fields = collect_map_fields(&program.classes, &class.name);
        module.class_map_fields.insert(class.name.clone(), map_fields);
    }


    // Collecte les types de paramètres des constructeurs (pour le boxing mixed)
    for class in &program.classes {
        if let Some((ctor_params, _, _)) = super::classes::nearest_constructor(&program.classes, class) {
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
                if let Some(lit) = value.const_literal() {
                    let key = format!("{}__{}", class.name, name);
                    module.class_consts.insert(key, (IrType::from_ast(ty), lit));
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

    // Ajout des types de retour des méthodes builtin Date/Time/DateTime (voir
    // src/builtins/{date,time,datetime}.rs pour la table de référence — sans ces
    // entrées, un appel direct comme IO::writeln(Date::today()) est mal dispatché
    // (retombe sur I64 par défaut et affiche le pointeur comme un entier brut au
    // lieu de la chaîne réelle) ; passer par une variable typée (var s:string =
    // Date::today()) contournait déjà le problème, ce qui l'a laissé inaperçu.
    fn_ret_types.insert("Date_today".to_string(), IrType::Ptr);
    fn_ret_types.insert("Date_fromTimestamp".to_string(), IrType::Ptr);
    fn_ret_types.insert("Date_year".to_string(), IrType::I64);
    fn_ret_types.insert("Date_month".to_string(), IrType::I64);
    fn_ret_types.insert("Date_day".to_string(), IrType::I64);
    fn_ret_types.insert("Date_dayOfWeek".to_string(), IrType::I64);
    fn_ret_types.insert("Date_isLeapYear".to_string(), IrType::Bool);
    fn_ret_types.insert("Date_daysInMonth".to_string(), IrType::I64);
    fn_ret_types.insert("Date_addDays".to_string(), IrType::Ptr);
    fn_ret_types.insert("Date_diffDays".to_string(), IrType::I64);

    fn_ret_types.insert("Time_now".to_string(), IrType::Ptr);
    fn_ret_types.insert("Time_fromTimestamp".to_string(), IrType::Ptr);
    fn_ret_types.insert("Time_hour".to_string(), IrType::I64);
    fn_ret_types.insert("Time_minute".to_string(), IrType::I64);
    fn_ret_types.insert("Time_second".to_string(), IrType::I64);
    fn_ret_types.insert("Time_fromSeconds".to_string(), IrType::Ptr);
    fn_ret_types.insert("Time_toSeconds".to_string(), IrType::I64);
    fn_ret_types.insert("Time_addSeconds".to_string(), IrType::Ptr);
    fn_ret_types.insert("Time_diffSeconds".to_string(), IrType::I64);

    fn_ret_types.insert("DateTime_now".to_string(), IrType::I64);
    fn_ret_types.insert("DateTime_fromTimestamp".to_string(), IrType::Ptr);
    fn_ret_types.insert("DateTime_year".to_string(), IrType::I64);
    fn_ret_types.insert("DateTime_month".to_string(), IrType::I64);
    fn_ret_types.insert("DateTime_day".to_string(), IrType::I64);
    fn_ret_types.insert("DateTime_hour".to_string(), IrType::I64);
    fn_ret_types.insert("DateTime_minute".to_string(), IrType::I64);
    fn_ret_types.insert("DateTime_second".to_string(), IrType::I64);
    fn_ret_types.insert("DateTime_format".to_string(), IrType::Ptr);
    fn_ret_types.insert("DateTime_parse".to_string(), IrType::I64);

    // Ajout des types de retour des méthodes builtin Tauri (voir src/builtins/tauri.rs
    // pour la table de référence — sans cette entrée, expr_ir_type() (typeinfer.rs) ne
    // peut pas savoir qu'un appel comme `ui.getTitle()` retourne un Ptr (string) plutôt
    // qu'un I64 brut, ce qui casse par exemple le dispatch de IO::writeln(ui.getTitle())).
    fn_ret_types.insert("Tauri_listen".to_string(), IrType::Void);
    fn_ret_types.insert("Tauri_emit".to_string(), IrType::Void);
    fn_ret_types.insert("Tauri_dialog".to_string(), IrType::Ptr);
    fn_ret_types.insert("Tauri_notify".to_string(), IrType::Void);
    fn_ret_types.insert("Tauri_getTitle".to_string(), IrType::Ptr);
    fn_ret_types.insert("Tauri_setTitle".to_string(), IrType::Void);
    fn_ret_types.insert("Tauri_getWidth".to_string(), IrType::I64);
    fn_ret_types.insert("Tauri_setWidth".to_string(), IrType::Void);
    fn_ret_types.insert("Tauri_getHeight".to_string(), IrType::I64);
    fn_ret_types.insert("Tauri_setHeight".to_string(), IrType::Void);
    fn_ret_types.insert("Tauri_getUrl".to_string(), IrType::Ptr);
    fn_ret_types.insert("Tauri_setUrl".to_string(), IrType::Void);
    fn_ret_types.insert("Tauri_open".to_string(), IrType::Void);
    fn_ret_types.insert("Tauri_close".to_string(), IrType::Void);
    fn_ret_types.insert("Tauri_isOpen".to_string(), IrType::Bool);
    fn_ret_types.insert("Tauri_focus".to_string(), IrType::Void);
    fn_ret_types.insert("Tauri_minimize".to_string(), IrType::Void);
    fn_ret_types.insert("Tauri_maximize".to_string(), IrType::Void);
    fn_ret_types.insert("Tauri_restore".to_string(), IrType::Void);
    fn_ret_types.insert("Tauri_hasFocus".to_string(), IrType::Bool);
    fn_ret_types.insert("Tauri_isMinimized".to_string(), IrType::Bool);
    fn_ret_types.insert("Tauri_isMaximized".to_string(), IrType::Bool);
    fn_ret_types.insert("Tauri_run".to_string(), IrType::Void);

    // Ajout des types de retour des méthodes builtin SDL (voir src/builtins/sdl.rs
    // pour la table de référence — même raison que le bloc Tauri ci-dessus).
    fn_ret_types.insert("SDL_pollEvent".to_string(), IrType::Ptr);
    fn_ret_types.insert("SDL_setDrawColor".to_string(), IrType::Void);
    fn_ret_types.insert("SDL_clear".to_string(), IrType::Void);
    fn_ret_types.insert("SDL_fillRect".to_string(), IrType::Void);
    fn_ret_types.insert("SDL_drawRect".to_string(), IrType::Void);
    fn_ret_types.insert("SDL_drawLine".to_string(), IrType::Void);
    fn_ret_types.insert("SDL_drawPoint".to_string(), IrType::Void);
    fn_ret_types.insert("SDL_present".to_string(), IrType::Void);
    fn_ret_types.insert("SDL_isOpen".to_string(), IrType::Bool);
    fn_ret_types.insert("SDL_close".to_string(), IrType::Void);
    fn_ret_types.insert("SDL_getWidth".to_string(), IrType::I64);
    fn_ret_types.insert("SDL_getHeight".to_string(), IrType::I64);
    fn_ret_types.insert("SDL_getTitle".to_string(), IrType::Ptr);
    fn_ret_types.insert("SDL_setTitle".to_string(), IrType::Void);
    fn_ret_types.insert("SDL_ticks".to_string(), IrType::I64);
    fn_ret_types.insert("SDL_delay".to_string(), IrType::Void);
    fn_ret_types.insert("SDL_loadTexture".to_string(), IrType::I64);
    fn_ret_types.insert("SDL_textureWidth".to_string(), IrType::I64);
    fn_ret_types.insert("SDL_textureHeight".to_string(), IrType::I64);
    fn_ret_types.insert("SDL_drawTexture".to_string(), IrType::Void);
    fn_ret_types.insert("SDL_drawTextureScaled".to_string(), IrType::Void);
    fn_ret_types.insert("SDL_drawTextureRegion".to_string(), IrType::Void);
    fn_ret_types.insert("SDL_unloadTexture".to_string(), IrType::Void);
    fn_ret_types.insert("SDL_loadFont".to_string(), IrType::I64);
    fn_ret_types.insert("SDL_unloadFont".to_string(), IrType::Void);
    fn_ret_types.insert("SDL_drawText".to_string(), IrType::Void);
    fn_ret_types.insert("SDL_isButtonPressed".to_string(), IrType::Bool);
    fn_ret_types.insert("SDL_getAxis".to_string(), IrType::I64);
    fn_ret_types.insert("SDL_loadSound".to_string(), IrType::I64);
    fn_ret_types.insert("SDL_unloadSound".to_string(), IrType::Void);
    fn_ret_types.insert("SDL_playSound".to_string(), IrType::Void);
    fn_ret_types.insert("SDL_playMusic".to_string(), IrType::Void);
    fn_ret_types.insert("SDL_pauseMusic".to_string(), IrType::Void);
    fn_ret_types.insert("SDL_resumeMusic".to_string(), IrType::Void);
    fn_ret_types.insert("SDL_stopMusic".to_string(), IrType::Void);
    fn_ret_types.insert("SDL_setMusicVolume".to_string(), IrType::Void);

    // System:: — méthodes I64/Ptr absentes du raccourci générique existant
    // (qui ne couvrait que cwd/exec/env, tous Ptr) : `pid`/`passthrough`/
    // `execCode` retournent I64 — confirmé faux par reproduction
    // (`System::pid()` interpolé dans un template SEGFAULT dès qu'un PID
    // assez grand a par hasard les bits bas à 00 et échoue le test `is_ptr`,
    // voir src/builtins/system.rs).
    fn_ret_types.insert("System_pid".to_string(), IrType::I64);
    fn_ret_types.insert("System_passthrough".to_string(), IrType::I64);
    fn_ret_types.insert("System_execCode".to_string(), IrType::I64);
    fn_ret_types.insert("System_args".to_string(), IrType::Ptr);

    // IO::read* — chaque méthode explicitement : le raccourci générique
    // `fname.starts_with("IO_read") => Ptr` (dans `expr_ir_type`) classait à
    // tort `readInt`/`readFloat`/`readBool` (I64/F64/Bool réels) comme `Ptr`
    // — confirmé faux par reproduction sur `examples/builtins/io.oc`
    // (`IO::readFloat()` ressortait comme un entier astronomique, bit
    // pattern brut du float).
    fn_ret_types.insert("IO_read".to_string(), IrType::Ptr);
    fn_ret_types.insert("IO_readln".to_string(), IrType::Ptr);
    fn_ret_types.insert("IO_readInt".to_string(), IrType::I64);
    fn_ret_types.insert("IO_readFloat".to_string(), IrType::F64);
    fn_ret_types.insert("IO_readBool".to_string(), IrType::Bool);
    fn_ret_types.insert("IO_readArray".to_string(), IrType::Ptr);
    fn_ret_types.insert("IO_readMap".to_string(), IrType::Ptr);

    // Math:: — absent de cette table jusqu'ici (aucune entrée), retombait
    // donc entièrement sur le filet de sécurité générique de `expr_ir_type` —
    // inoffensif pour les méthodes qui retournent I64 (`abs`/`min`/`max`/
    // `pow`/`clamp`/`random`/`floor`/`ceil`/`round`), mais `sqrt` (F64) en
    // sortait corrompue (bit pattern brut affiché comme un entier
    // astronomique, confirmé par reproduction) — voir
    // src/builtins/math.rs.
    fn_ret_types.insert("Math_abs".to_string(), IrType::I64);
    fn_ret_types.insert("Math_min".to_string(), IrType::I64);
    fn_ret_types.insert("Math_max".to_string(), IrType::I64);
    fn_ret_types.insert("Math_pow".to_string(), IrType::I64);
    fn_ret_types.insert("Math_clamp".to_string(), IrType::I64);
    fn_ret_types.insert("Math_random".to_string(), IrType::I64);
    fn_ret_types.insert("Math_sqrt".to_string(), IrType::F64);
    fn_ret_types.insert("Math_floor".to_string(), IrType::I64);
    fn_ret_types.insert("Math_ceil".to_string(), IrType::I64);
    fn_ret_types.insert("Math_round".to_string(), IrType::I64);

    // Convert:: — chaque méthode explicitement, par vrai type de retour
    // (voir src/builtins/convert.rs) : le raccourci générique qui existait
    // ici auparavant (`fname.starts_with("Convert_") => Ptr`, dans
    // `expr_ir_type`) classait à tort TOUTES les méthodes `Convert_*` comme
    // `Ptr`, y compris `strToInt`/`strToFloat`/`strToBool`/`floatToInt`/
    // `intToBool`/... qui retournent en réalité I64/F64/Bool — inoffensif
    // tant que rien n'agissait différemment selon le type rapporté, mais
    // confirmé faux dès que ce chantier a donné un sens réel à I64 vs Ptr
    // (`examples/builtins/convert.oc` plantait dès le premier appel).
    fn_ret_types.insert("Convert_strToInt".to_string(), IrType::I64);
    fn_ret_types.insert("Convert_strToFloat".to_string(), IrType::F64);
    fn_ret_types.insert("Convert_strToBool".to_string(), IrType::Bool);
    fn_ret_types.insert("Convert_strToArray".to_string(), IrType::Ptr);
    fn_ret_types.insert("Convert_strToMap".to_string(), IrType::Ptr);
    fn_ret_types.insert("Convert_intToStr".to_string(), IrType::Ptr);
    fn_ret_types.insert("Convert_intToFloat".to_string(), IrType::F64);
    fn_ret_types.insert("Convert_intToBool".to_string(), IrType::Bool);
    fn_ret_types.insert("Convert_floatToStr".to_string(), IrType::Ptr);
    fn_ret_types.insert("Convert_floatToInt".to_string(), IrType::I64);
    fn_ret_types.insert("Convert_floatToBool".to_string(), IrType::Bool);
    fn_ret_types.insert("Convert_boolToStr".to_string(), IrType::Ptr);
    fn_ret_types.insert("Convert_boolToInt".to_string(), IrType::I64);
    fn_ret_types.insert("Convert_boolToFloat".to_string(), IrType::F64);
    fn_ret_types.insert("Convert_arrayToStr".to_string(), IrType::Ptr);
    fn_ret_types.insert("Convert_arrayToMap".to_string(), IrType::Ptr);
    fn_ret_types.insert("Convert_mapToStr".to_string(), IrType::Ptr);
    fn_ret_types.insert("Convert_mapKeysToArray".to_string(), IrType::Ptr);
    fn_ret_types.insert("Convert_mapValuesToArray".to_string(), IrType::Ptr);

    // SQLite/MySQL/MariaDB : constructeurs statiques et méthodes de requête
    // retournant un objet/tableau/map (Ptr), auparavant absents d'ici —
    // retombaient donc sur le filet de sécurité générique de `expr_ir_type`
    // (`Expr::StaticCall`, `src/lower/expr.d/typeinfer.rs`). Les rendre
    // explicites ici reste la pratique établie pour tout le reste de cette
    // table, et documente le vrai type au lieu de dépendre du filet.
    fn_ret_types.insert("SQLite_open".to_string(), IrType::Ptr);
    fn_ret_types.insert("SQLite_query".to_string(), IrType::Ptr);
    fn_ret_types.insert("SQLite_queryOne".to_string(), IrType::Ptr);
    fn_ret_types.insert("SQLite_lastInsertId".to_string(), IrType::I64);
    fn_ret_types.insert("SQLite_affectedRows".to_string(), IrType::I64);
    fn_ret_types.insert("SQLite_close".to_string(), IrType::Void);
    for prefix in ["MySQL", "MariaDB"] {
        fn_ret_types.insert(format!("{}_connect", prefix), IrType::Ptr);
        fn_ret_types.insert(format!("{}_execute", prefix), IrType::I64);
        fn_ret_types.insert(format!("{}_query", prefix), IrType::Ptr);
        fn_ret_types.insert(format!("{}_queryOne", prefix), IrType::Ptr);
        fn_ret_types.insert(format!("{}_lastInsertId", prefix), IrType::I64);
        fn_ret_types.insert(format!("{}_affectedRows", prefix), IrType::I64);
        fn_ret_types.insert(format!("{}_close", prefix), IrType::Void);
    }

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
                                fn_ret_types.insert(child_key.clone(), ty);
                            }
                            // Même propagation pour `method_ret_class` (voir sa doc) :
                            // une méthode héritée non surchargée doit rester résolvable
                            // pour le chaînage (`sousClasse.methodeHeritee().autreChose()`).
                            if let Some(name) = module.method_ret_class.get(&parent_key).cloned() {
                                module.method_ret_class.insert(child_key, name);
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
        // Générateur (`emit`/`message<T>`, voir docs/roadmap.d/langage-emit-iterable.md) :
        // deux fonctions IR séparées (`<nom>__new`/`<nom>__resume`), pas le
        // lowering normal d'une fonction — voir
        // `super::message_gen::lower_message_func`.
        if super::message_gen::is_message_func(&func.ret_ty) {
            super::message_gen::lower_message_func(&mut module, func, &fn_ret_types, &fn_param_types, &fn_param_names);
            continue;
        }
        lower_func(&mut module, func, &program.consts, &fn_ret_types, &fn_param_types, &fn_param_names, &fn_variadic_info, &func_default_args, None, None, &async_funcs);
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
        lower_class(&mut module, class, &program.classes, &program.modules, &program.consts, &fn_ret_types, &fn_param_types, &fn_param_names, &fn_variadic_info, &func_default_args, &async_funcs);
        
        // Générer les wrappers async pour les méthodes de classes
        for member in &class.members {
            if let ClassMember::Method { decl, is_static, .. } = member {
                if decl.is_async {
                    let mangled_name = format!("{}_{}", class.name, decl.name);
                    let param_tys: Vec<IrType> = if *is_static {
                        // Méthode statique : pas de self
                        decl.params.iter().map(|p| IrType::from_ast(&p.ty)).collect()
                    } else {
                        // Méthode d'instance : self en premier (Type::Mixed → IrType::Ptr)
                        let mut types = vec![IrType::Ptr];
                        types.extend(decl.params.iter().map(|p| IrType::from_ast(&p.ty)));
                        types
                    };
                    let ret_ty = IrType::from_ast(&decl.ret_ty);
                    let wrapper_name = format!("__async_wrap_{}", mangled_name);
                    generate_async_wrapper(&mut module, &mangled_name, &wrapper_name, &param_tys, ret_ty, &fn_ret_types);
                }
            }
        }
    }

    // Dispatch dynamique réel pour les interfaces ET l'héritage de classe
    // (voir docs/roadmap.d/langage-interfaces.md) — après toutes les
    // classes, dont les méthodes concrètes doivent déjà exister pour être
    // appelées depuis les dispatchers générés ici.
    super::interfaces::generate_interface_dispatchers(&mut module, program);
    super::class_dispatch::generate_class_dispatchers(&mut module, program);

    // Wrappers async pour les DISPATCHERS dynamiques dont la méthode
    // multiplexée est `async` — voir la remarque déjà posée plus haut (au
    // moment de peupler `async_funcs` avec leur nom) et
    // docs/roadmap.d/langage-async-instance-method-dispatch-broken.md.
    // Exactement le même patron que les wrappers déjà générés ci-dessus
    // pour une méthode de classe concrète (`generate_async_wrapper` ne se
    // soucie jamais de QUI il appelle, seulement de la signature) :
    // `param_tys` commence par `self` (`Ptr`), le dispatcher lui-même vient
    // d'être généré juste au-dessus (son CORPS existe déjà à ce point,
    // contrairement à `async_funcs`, qui devait le savoir bien plus tôt).
    for class in &program.classes {
        if !module.classes_with_subclasses.contains(&class.name) {
            continue;
        }
        for method_name in super::class_dispatch::callable_instance_method_names(&class.name, &program.classes) {
            let Some(decl) = super::class_dispatch::find_method_decl(&class.name, &method_name, &program.classes) else { continue };
            if !decl.is_async {
                continue;
            }
            let dispatcher_name = format!("__dispatch_{}_{}", class.name, method_name);
            let mut param_tys = vec![IrType::Ptr];
            param_tys.extend(decl.params.iter().map(|p| IrType::from_ast(&p.ty)));
            let ret_ty = IrType::from_ast(&decl.ret_ty);
            let wrapper_name = format!("__async_wrap_{}", dispatcher_name);
            generate_async_wrapper(&mut module, &dispatcher_name, &wrapper_name, &param_tys, ret_ty, &fn_ret_types);
        }
    }
    for iface in &program.interfaces {
        let has_implementer = program.classes.iter().any(|c| c.implements.iter().any(|i| i == &iface.name));
        if !has_implementer {
            continue;
        }
        for method in &iface.methods {
            if !method.is_async || method.is_static {
                continue;
            }
            let dispatcher_name = format!("{}_{}", iface.name, method.name);
            let mut param_tys = vec![IrType::Ptr];
            param_tys.extend(method.params.iter().map(|p| IrType::from_ast(&p.ty)));
            let ret_ty = IrType::from_ast(&method.ret_ty);
            let wrapper_name = format!("__async_wrap_{}", dispatcher_name);
            generate_async_wrapper(&mut module, &dispatcher_name, &wrapper_name, &param_tys, ret_ty, &fn_ret_types);
        }
    }

    // Blocs runtime → fonctions __init__, __main__, etc.
    lower_runtime_blocks(&mut module, program, &program.consts, &fn_ret_types, &fn_param_types, &fn_param_names, &fn_variadic_info, &func_default_args, &async_funcs);

    super::rc_layout::compute_class_masks(&mut module);
    module
}
