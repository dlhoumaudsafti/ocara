/// Lowering principal des expressions

use std::collections::HashSet;
use crate::parsing::ast::*;
use crate::ir::inst::{Inst, Value};
use crate::ir::types::IrType;
use crate::lower::builder::LowerBuilder;
use crate::codegen::runtime::builtins;
use super::helpers::*;
use super::typeinfer::expr_ir_type;
use super::nameless::lower_nameless_fn;
use super::captures::collect_captures;
use super::literals::{lower_literal, lower_is_check};

pub fn lower_expr(builder: &mut LowerBuilder, expr: &Expr) -> Value {
    match expr {
        // ── Littéraux ────────────────────────────────────────────────────────
        Expr::Literal(Literal::Int(n), _) => {
            let dest = builder.new_value();
            builder.emit(Inst::ConstInt { dest: dest.clone(), value: *n });
            dest
        }
        Expr::Literal(Literal::Float(f), _) => {
            let dest = builder.new_value();
            builder.emit(Inst::ConstFloat { dest: dest.clone(), value: *f });
            dest
        }
        Expr::Literal(Literal::Bool(b), _) => {
            let dest = builder.new_value();
            builder.emit(Inst::ConstBool { dest: dest.clone(), value: *b });
            dest
        }
        Expr::Literal(Literal::String(s), _) => {
            let idx = builder.module.intern_string(s);
            let dest = builder.new_value();
            builder.emit(Inst::ConstStr { dest: dest.clone(), idx });
            dest
        }        Expr::Literal(Literal::Null, _) => {
            // null = pointeur nul (0)
            let dest = builder.new_value();
            builder.emit(Inst::ConstInt { dest: dest.clone(), value: 0 });
            dest
        }
        // ── Identifiant ──────────────────────────────────────────────────────
        Expr::Ident(name, _) => {
            if let Some((val, _)) = builder.load_local(name) {
                return val;
            }
            // Référence à une fonction libre → fat pointer {wrapper_addr, 0}
            if builder.fn_param_types.contains_key(name.as_str()) {
                let wrapper_name = format!("__fn_wrap_{}", name);
                let func_addr = builder.new_value();
                builder.emit(Inst::FuncAddr { dest: func_addr.clone(), func: wrapper_name });
                let zero = builder.new_value();
                builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
                let fat_ptr = builder.new_value();
                builder.emit(Inst::Alloc { dest: fat_ptr.clone(), class: "__fat_ptr".into() });
                builder.emit(Inst::SetField { obj: fat_ptr.clone(), field: "func".into(), src: func_addr, offset: 0 });
                builder.emit(Inst::SetField { obj: fat_ptr.clone(), field: "env".into(),  src: zero,      offset: 8 });
                return fat_ptr;
            }
            // Fallback : constante globale ou symbole non résolu → nop
            let dest = builder.new_value();
            builder.emit(Inst::Nop);
            dest
        }

        // ── self ─────────────────────────────────────────────────────────────
        Expr::SelfExpr(_) => {
            // `self` est enregistré comme local dans builder.locals["self"]
            if let Some((dest, _)) = builder.load_local("self") {
                dest
            } else {
                // fallback défensif (ne devrait pas arriver dans une méthode valide)
                let dest = builder.new_value();
                builder.emit(Inst::Load {
                    dest: dest.clone(),
                    ptr:  Value(0),
                    ty:   IrType::Ptr,
                });
                dest
            }
        }

        // ── Accès de champ ───────────────────────────────────────────────────
        Expr::Field { object, field, .. } => {
            // Résoudre la classe de l'objet pour calculer l'offset
            let class_name = match object.as_ref() {
                Expr::Ident(name, _) => builder.var_class.get(name.as_str()).cloned(),
                Expr::SelfExpr(_)    => builder.current_class.clone(),
                _ => None,
            };
            let offset = if let Some(cls) = &class_name {
                field_offset(&builder.module.class_layouts, cls, field)
            } else {
                0
            };
            let field_ty = if let Some(cls) = &class_name {
                field_ir_type(&builder.module.class_layouts, cls, field)
            } else {
                IrType::Ptr
            };
            let obj_val = lower_expr(builder, object);
            let dest = builder.new_value();
            builder.emit(Inst::GetField {
                dest:  dest.clone(),
                obj:   obj_val,
                field: field.clone(),
                ty:    field_ty,
                offset,
            });
            dest
        }

        // ── Appel de fonction libre ───────────────────────────────────────────
        Expr::Call { callee, args, .. } => {
            // Bloquer les appels directs aux fonctions internes du codegen
            // SAUF pour les fonctions runtime appelées depuis main()
            if let Expr::Ident(name, _) = callee.as_ref() {
                if name.starts_with("__") && !name.starts_with("__runtime_") {
                    eprintln!("error: `{}` is an internal compiler function and cannot be called directly", name);
                    std::process::exit(1);
                }
            }

            // Appel indirect : variable de type Function → déréférence fat pointer
            if let Expr::Ident(name, _) = callee.as_ref() {
                if builder.func_vars.contains(name.as_str()) {
                    let fat_ptr = builder.load_local(name)
                        .map(|(v, _)| v)
                        .unwrap_or_else(|| { let d = builder.new_value(); builder.emit(Inst::Nop); d });
                    // Lire func_ptr depuis fat_ptr[0]
                    let func_ptr = builder.new_value();
                    builder.emit(Inst::GetField { dest: func_ptr.clone(), obj: fat_ptr.clone(), field: "func".into(), ty: IrType::Ptr, offset: 0 });
                    // Lire env_ptr depuis fat_ptr[8]
                    let env_ptr = builder.new_value();
                    builder.emit(Inst::GetField { dest: env_ptr.clone(), obj: fat_ptr, field: "env".into(), ty: IrType::Ptr, offset: 8 });
                    
                    // Évaluer les arguments fournis
                    let mut arg_vals: Vec<Value> = args.iter().map(|a| lower_expr(builder, a)).collect();
                    
                    // Pour supporter les paramètres par défaut dans les nameless,
                    // compléter avec des sentinelles (0) jusqu'à un maximum raisonnable
                    // La fonction nameless détectera ces sentinelles et utilisera ses valeurs par défaut
                    const MAX_PARAMS: usize = 5;
                    while arg_vals.len() < MAX_PARAMS {
                        let zero = builder.new_value();
                        builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
                        arg_vals.push(zero);
                    }
                    
                    // Appel avec env_ptr en premier (convention uniforme)
                    let mut all_args = vec![env_ptr];
                    all_args.extend(arg_vals);
                    let dest = builder.new_value();
                    // Déterminer le type de retour depuis func_ret_types
                    let ret_ty = builder.func_ret_types.get(name.as_str()).cloned().unwrap_or(IrType::I64);
                    builder.emit(Inst::CallIndirect {
                        dest:   Some(dest.clone()),
                        callee: func_ptr,
                        args:   all_args,
                        ret_ty,
                    });
                    return dest;
                }
            }

            let func_name = match callee.as_ref() {
                Expr::Ident(name, _) => name.clone(),
                Expr::Field { object, field, .. } => {
                    // méthode → appel manglé ClassName_method
                    // On résout le nom de classe depuis var_class ou current_class (self)
                    
                    // ── Cas spécial : méthodes JSON sur types primitifs ───────────────
                    // array/map.encode() → JSON_encode(obj)
                    // string.decode() / string.pretty() / string.minimize() → JSON_<method>(obj)
                    let is_json_method = match field.as_str() {
                        "encode" | "decode" | "pretty" | "minimize" => true,
                        _ => false,
                    };
                    
                    if is_json_method {
                        let obj_val = lower_expr(builder, object);
                        let dest = builder.new_value();
                        let func_name = format!("JSON_{}", field);
                        
                        // JSON est maintenant toujours disponible, pas besoin de vérifier l'import
                        
                        let ret_ty = builder.fn_ret_types.get(&func_name).cloned().unwrap_or(IrType::Ptr);
                        builder.emit(Inst::Call {
                            dest:   Some(dest.clone()),
                            func:   func_name,
                            args:   vec![obj_val],
                            ret_ty,
                        });
                        return dest;
                    }
                    
                    // ── Cas normal : résolution de classe ─────────────────────────────
                    let class_name = match object.as_ref() {
                        Expr::Ident(var_name, _) => {
                            builder.var_class.get(var_name.as_str()).cloned()
                        }
                        Expr::SelfExpr(_) => builder.current_class.clone(),
                        // String littérale : "hello".trim()
                        Expr::Literal(Literal::String(_), _) => Some("String".to_string()),
                        // Appel chainé : arr.sort().reverse() ou text.trim().lower()
                        Expr::Call { callee: inner_callee, .. } => {
                            if let Expr::Field { object: inner_obj, field: inner_method, .. } = inner_callee.as_ref() {
                                // Essayer de trouver la classe de l'objet interne
                                let inner_class = match inner_obj.as_ref() {
                                    Expr::Ident(name, _) => builder.var_class.get(name.as_str()).cloned(),
                                    _ => None,
                                };
                                // Si on a trouvé la classe, vérifier le type de retour de la méthode
                                if let Some(cls) = inner_class {
                                    let method_name = format!("{}_{}", cls, inner_method);
                                    // Si la méthode retourne un Ptr et que c'est la même classe, continuer avec elle
                                    if let Some(ret_ty) = builder.fn_ret_types.get(&method_name) {
                                        if matches!(ret_ty, IrType::Ptr) {
                                            Some(cls)
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }
                        // Appel de fonction retournant string : func().trim()
                        // Accès de champ retournant string : obj.name.trim()
                        _ => {
                            // Fallback : vérifier si c'est un type string via l'IR
                            let ir_ty = expr_ir_type(builder, object);
                            if matches!(ir_ty, IrType::Ptr) {
                                // Peut être une string, on essaye avec String
                                Some("String".to_string())
                            } else {
                                None
                            }
                        }
                    };
                    let func_mangled = if let Some(cls) = class_name {
                        format!("{}_{}", cls, field)
                    } else {
                        format!("_method_{}", field) // fallback (ne devrait pas arriver)
                    };
                    
                    // Compléter les arguments avec les valeurs par défaut si nécessaire
                    let completed_args = complete_args_with_defaults(builder, &func_mangled, args);
                    
                    let obj_val = lower_expr(builder, object);
                    let dest = builder.new_value();
                    let arg_vals: Vec<Value> = completed_args.iter().map(|a| lower_expr(builder, a)).collect();
                    let mut all_args = vec![obj_val];
                    all_args.extend(arg_vals);
                    // Résoudre le type de retour depuis fn_ret_types
                    let ret_ty = builder.fn_ret_types.get(&func_mangled).cloned().unwrap_or(IrType::Ptr);
                    builder.emit(Inst::Call {
                        dest:   Some(dest.clone()),
                        func:   func_mangled,
                        args:   all_args,
                        ret_ty,
                    });
                    return dest;
                }
                _ => "_unknown".into(),
            };

            // Compléter les arguments avec les valeurs par défaut si nécessaire
            let completed_args = complete_args_with_defaults(builder, &func_name, args);
            let args = &completed_args; // Remplacer args par completed_args pour le reste

            // Fonctions d'affichage : dispatch vers la variante typée
            const WRITE_FUNS: &[&str] = &["IO_write", "IO_writeln"];
            if WRITE_FUNS.contains(&func_name.as_str()) && args.len() == 1 {
                let arg_ty  = expr_ir_type(builder, &args[0]);
                let variant = write_variant(&func_name, &arg_ty);
                let arg_val = lower_expr(builder, &args[0]);
                builder.emit(Inst::Call {
                    dest:   None,
                    func:   variant,
                    args:   vec![arg_val],
                    ret_ty: IrType::Void,
                });
                // Les fonctions void ne retournent rien, donc on retourne une constante dummy
                let dummy = builder.new_value();
                builder.emit(Inst::ConstInt { dest: dummy.clone(), value: 0 });
                return dummy;
            }

            // Appel async : spawn un thread, retourne un task handle (i64)
            if builder.async_funcs.contains(func_name.as_str()) {
                let wrapper_name = format!("__async_wrap_{}", func_name);
                // Évaluer les arguments
                let arg_vals: Vec<Value> = args.iter().map(|a| lower_expr(builder, a)).collect();
                let n_args = arg_vals.len();
                // Allouer l'env heap : n_args * 8 octets
                let env_size = builder.new_value();
                builder.emit(Inst::ConstInt { dest: env_size.clone(), value: (n_args * 8) as i64 });
                let env_ptr = builder.new_value();
                builder.emit(Inst::Call {
                    dest:   Some(env_ptr.clone()),
                    func:   "__alloc_obj".into(),
                    args:   vec![env_size],
                    ret_ty: IrType::I64,
                });
                // Stocker chaque arg dans env[i*8]
                for (i, arg_val) in arg_vals.iter().enumerate() {
                    builder.emit(Inst::SetField {
                        obj:    env_ptr.clone(),
                        field:  format!("__arg{}", i),
                        src:    arg_val.clone(),
                        offset: (i * 8) as i32,
                    });
                }
                // Obtenir l'adresse du wrapper
                let func_addr = builder.new_value();
                builder.emit(Inst::FuncAddr { dest: func_addr.clone(), func: wrapper_name });
                // Appeler __task_spawn(func_addr, env_ptr) → task handle
                let task = builder.new_value();
                builder.emit(Inst::Call {
                    dest:   Some(task.clone()),
                    func:   "__task_spawn".into(),
                    args:   vec![func_addr, env_ptr],
                    ret_ty: IrType::I64,
                });
                return task;
            }

            // Boxer F64/Bool si le paramètre cible est `mixed` (Ptr)
            let param_types = builder.fn_param_types.get(func_name.as_str()).cloned();
            let arg_vals: Vec<Value> = args.iter().enumerate().map(|(i, a)| {
                let raw = lower_expr(builder, a);
                let arg_ty = expr_ir_type(builder, a);
                let param_ty = param_types.as_ref().and_then(|pts| pts.get(i)).cloned();
                if param_ty == Some(IrType::Ptr) {
                    match arg_ty {
                        IrType::F64 => {
                            let d = builder.new_value();
                            builder.emit(Inst::Call { dest: Some(d.clone()), func: "__box_float".into(), args: vec![raw], ret_ty: IrType::Ptr });
                            d
                        }
                        IrType::Bool => {
                            let d = builder.new_value();
                            builder.emit(Inst::Call { dest: Some(d.clone()), func: "__box_bool".into(), args: vec![raw], ret_ty: IrType::Ptr });
                            d
                        }
                        _ => raw,
                    }
                } else {
                    raw
                }
            }).collect();
            
            // Si fonction variadic, empaqueter les arguments excédentaires dans un tableau
            let final_args = if let Some(&(fixed_count, ref _elem_ty)) = builder.fn_variadic_info.get(func_name.as_str()) {
                if arg_vals.len() >= fixed_count {
                    let mut final_args = arg_vals[..fixed_count].to_vec();
                    
                    // Créer le tableau variadic
                    let arr = builder.new_value();
                    builder.emit(Inst::Call {
                        dest:   Some(arr.clone()),
                        func:   "__array_new".into(),
                        args:   vec![],
                        ret_ty: IrType::Ptr,
                    });
                    
                    // Pousser chaque argument variadic dans le tableau (avec boxing si nécessaire)
                    for (idx, variadic_arg) in arg_vals[fixed_count..].iter().enumerate() {
                        let arg_expr = &args[fixed_count + idx];
                        let arg_ty = expr_ir_type(builder, arg_expr);
                        
                        // Boxer uniquement F64 et Bool pour stockage dans mixed[]
                        // Les int (I64) sont stockés directement comme tagged values
                        let stored_val = match arg_ty {
                            IrType::F64 => {
                                let boxed = builder.new_value();
                                builder.emit(Inst::Call {
                                    dest:   Some(boxed.clone()),
                                    func:   "__box_float".into(),
                                    args:   vec![variadic_arg.clone()],
                                    ret_ty: IrType::Ptr,
                                });
                                boxed
                            }
                            IrType::Bool => {
                                let boxed = builder.new_value();
                                builder.emit(Inst::Call {
                                    dest:   Some(boxed.clone()),
                                    func:   "__box_bool".into(),
                                    args:   vec![variadic_arg.clone()],
                                    ret_ty: IrType::Ptr,
                                });
                                boxed
                            }
                            _ => variadic_arg.clone(),  // I64, Ptr, etc. → stockage direct
                        };
                        
                        builder.emit(Inst::Call {
                            dest:   None,
                            func:   "__array_push".into(),
                            args:   vec![arr.clone(), stored_val],
                            ret_ty: IrType::Void,
                        });
                    }
                    
                    // Ajouter le tableau comme dernier argument
                    final_args.push(arr);
                    final_args
                } else {
                    arg_vals
                }
            } else {
                arg_vals
            };
            
            let dest = builder.new_value();
            builder.emit(Inst::Call {
                dest:   Some(dest.clone()),
                func:   func_name,
                args:   final_args,
                ret_ty: IrType::Ptr,
            });
            dest
        }

        // ── Accès statique ──────────────────────────────────────────────────
        Expr::StaticCall { class, method, args, span } => {
            // Résoudre "<self>" vers la classe courante
            let self_class;
            let mut resolved_class: &str = if class == "<self>" {
                self_class = builder.current_class.clone().unwrap_or_default();
                &self_class
            } else {
                class.as_str()
            };
            
            // Pour self::method, chercher la méthode dans la chaîne d'héritage
            if class == "<self>" && !resolved_class.is_empty() {
                let func_name = format!("{}_{}", resolved_class, method);
                // Vérifier si la méthode existe dans la classe courante
                let method_exists = builder.module.functions.iter()
                    .any(|f| f.name == func_name);
                
                if !method_exists {
                    // Chercher dans la chaîne d'héritage
                    let mut current = resolved_class;
                    loop {
                        if let Some(parent) = builder.module.class_parents.get(current) {
                            let parent_func_name = format!("{}_{}", parent, method);
                            // Vérifier dans les fonctions du module ET dans les builtins
                            let parent_method_exists = builder.module.functions.iter()
                                .any(|f| f.name == parent_func_name)
                                || builtins().iter().any(|b| b.name == parent_func_name);
                            if parent_method_exists {
                                resolved_class = parent.as_str();
                                break;
                            }
                            current = parent.as_str();
                        } else {
                            break;
                        }
                    }
                }
            }
            
            let func_name = format!("{}_{}", resolved_class, method);

            // Vérification de l'import : les modules ocara builtins doivent être importés
            // SAUF si la classe est définie localement dans le programme
            const BUILTIN_MODULES: &[&str] = &[
                "String", "Math", "Array", "Map", "IO", "JSON",
                "Convert", "System", "Regex", "HTTPRequest", "HTTPServer", "Thread",
                "Mutex", "HTML", "HTMLComponent", "UnitTest", "File", "Directory",
                "Date", "Time", "DateTime",
            ];
            let is_local_class = builder.module.class_layouts.contains_key(resolved_class);
            if BUILTIN_MODULES.contains(&resolved_class) && !is_local_class {
                let imported = builder.module.imports.iter().any(|m| m == resolved_class);
                if !imported {
                    eprintln!(
                        "{}:{}:{}: error: using `{}::{}` without `import ocara.{}`",
                        builder.module.source_file, span.line, span.col, resolved_class, method, resolved_class
                    );
                    std::process::exit(1);
                }
            }

            // Dispatch typé pour IO::write / IO::writeln
            const IO_WRITE_METHODS: &[&str] = &["IO_write", "IO_writeln"];
            if IO_WRITE_METHODS.contains(&func_name.as_str()) && args.len() == 1 {
                let arg_ty  = expr_ir_type(builder, &args[0]);
                let variant = write_variant(&func_name, &arg_ty);
                let arg_val = lower_expr(builder, &args[0]);
                builder.emit(Inst::Call {
                    dest:   None,
                    func:   variant,
                    args:   vec![arg_val],
                    ret_ty: IrType::Void,
                });
                // Les fonctions void ne retournent rien, donc on retourne une constante dummy
                let dummy = builder.new_value();
                builder.emit(Inst::ConstInt { dest: dummy.clone(), value: 0 });
                return dummy;
            }

            let arg_vals: Vec<Value> = args.iter().map(|a| lower_expr(builder, a)).collect();
            
            // Vérifier si le builtin retourne void
            if is_void_builtin(&func_name) {
                builder.emit(Inst::Call {
                    dest:   None,
                    func:   func_name,
                    args:   arg_vals,
                    ret_ty: IrType::Void,
                });
                // Les fonctions void ne retournent rien, donc on retourne une constante dummy
                let dummy = builder.new_value();
                builder.emit(Inst::ConstInt { dest: dummy.clone(), value: 0 });
                return dummy;
            }
            
            let dest = builder.new_value();
            builder.emit(Inst::Call {
                dest:   Some(dest.clone()),
                func:   func_name,
                args:   arg_vals,
                ret_ty: IrType::Ptr,
            });
            dest
        }

        // ── Lecture de constante de classe : `Class::NAME` ────────────────────
        Expr::StaticConst { class, name, .. } => {
            // Résoudre "<self>" vers la classe courante
            let self_class;
            let class: &str = if class == "<self>" {
                self_class = builder.current_class.clone().unwrap_or_default();
                &self_class
            } else {
                class.as_str()
            };
            let key = format!("{}__{}" , class, name);
            let dest = builder.new_value();
            if let Some((ty, lit)) = builder.module.class_consts.get(&key).cloned() {
                match lit {
                    Literal::Int(n)    => builder.emit(Inst::ConstInt   { dest: dest.clone(), value: n }),
                    Literal::Float(f)  => builder.emit(Inst::ConstFloat { dest: dest.clone(), value: f }),
                    Literal::Bool(b)   => builder.emit(Inst::ConstBool  { dest: dest.clone(), value: b }),
                    Literal::String(s) => {
                        let idx = builder.module.intern_string(&s);
                        builder.emit(Inst::ConstStr { dest: dest.clone(), idx });
                        let _ = ty;
                    }
                    Literal::Null => builder.emit(Inst::ConstInt { dest: dest.clone(), value: 0 }),
                }
            } else if class == "System" && (name == "OS" || name == "ARCH") {
                // Constantes de plateforme — déterminées à la compilation du runtime
                let func_name = if name == "OS" { "__system_os" } else { "__system_arch" };
                builder.emit(Inst::Call {
                    dest:   Some(dest.clone()),
                    func:   func_name.into(),
                    args:   vec![],
                    ret_ty: IrType::Ptr,
                });
            } else {
                // Référence à une méthode statique sans appel → fat pointer
                let method_key = format!("{}_{}", class, name);
                if builder.fn_param_types.contains_key(&method_key) {
                    let wrapper_name = format!("__fn_wrap_{}", method_key);
                    let func_addr = builder.new_value();
                    builder.emit(Inst::FuncAddr { dest: func_addr.clone(), func: wrapper_name });
                    let zero = builder.new_value();
                    builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
                    builder.emit(Inst::Alloc { dest: dest.clone(), class: "__fat_ptr".into() });
                    builder.emit(Inst::SetField { obj: dest.clone(), field: "func".into(), src: func_addr, offset: 0 });
                    builder.emit(Inst::SetField { obj: dest.clone(), field: "env".into(),  src: zero,      offset: 8 });
                } else {
                    // Fallback : charge le global manglé comme pointeur
                    let idx = builder.module.intern_string(&key);
                    builder.emit(Inst::ConstStr { dest: dest.clone(), idx });
                }
            }
            dest
        }

        // ── Instanciation ─────────────────────────────────────────────────────
        Expr::New { class, args, .. } => {
            let dest = builder.new_value();
            builder.emit(Inst::Alloc { dest: dest.clone(), class: class.clone() });
            // Récupère les types de params du constructeur pour boxer F64/Bool → mixed (Ptr)
            let ctor_params = builder.module.ctor_param_types
                .get(class.as_str())
                .cloned()
                .unwrap_or_default();
            let mut ctor_args = vec![dest.clone()];
            for (i, a) in args.iter().enumerate() {
                let arg_ty   = expr_ir_type(builder, a);
                let val      = lower_expr(builder, a);
                let param_ty = ctor_params.get(i).cloned().unwrap_or(IrType::I64);
                // Si le paramètre est `mixed` (Ptr) mais la valeur est F64 ou Bool → boxer
                let boxed = if param_ty == IrType::Ptr {
                    match arg_ty {
                        IrType::F64 => {
                            let d = builder.new_value();
                            builder.emit(Inst::Call {
                                dest:   Some(d.clone()),
                                func:   "__box_float".into(),
                                args:   vec![val],
                                ret_ty: IrType::Ptr,
                            });
                            d
                        }
                        IrType::Bool => {
                            let d = builder.new_value();
                            builder.emit(Inst::Call {
                                dest:   Some(d.clone()),
                                func:   "__box_bool".into(),
                                args:   vec![val],
                                ret_ty: IrType::Ptr,
                            });
                            d
                        }
                        _ => val,
                    }
                } else {
                    val
                };
                ctor_args.push(boxed);
            }
            // Appel du constructeur
            builder.emit(Inst::Call {
                dest:   None,
                func:   format!("{}_init", class),
                args:   ctor_args,
                ret_ty: IrType::Void,
            });
            dest
        }

        // ── Opération binaire ─────────────────────────────────────────────────
        Expr::Binary { op, left, right, .. } => {
            // Détection de la concaténation string : au moins un opérande est Ptr/String
            if matches!(op, BinOp::Add) {
                let left_ty  = expr_ir_type(builder, left);
                let right_ty = expr_ir_type(builder, right);
                if matches!(left_ty, IrType::Ptr) || matches!(right_ty, IrType::Ptr) {
                    let lv_raw = lower_expr(builder, left);
                    let rv_raw = lower_expr(builder, right);
                    // Convertir F64/Bool en string avant __str_concat (sinon les bits du float
                    // sont interprétés comme pointeur et causent une segfault)
                    let lv = match left_ty {
                        IrType::F64 => {
                            let d = builder.new_value();
                            builder.emit(Inst::Call { dest: Some(d.clone()), func: "__str_from_float".into(), args: vec![lv_raw], ret_ty: IrType::Ptr });
                            d
                        }
                        IrType::Bool => {
                            let d = builder.new_value();
                            builder.emit(Inst::Call { dest: Some(d.clone()), func: "__str_from_bool".into(), args: vec![lv_raw], ret_ty: IrType::Ptr });
                            d
                        }
                        _ => lv_raw,
                    };
                    let rv = match right_ty {
                        IrType::F64 => {
                            let d = builder.new_value();
                            builder.emit(Inst::Call { dest: Some(d.clone()), func: "__str_from_float".into(), args: vec![rv_raw], ret_ty: IrType::Ptr });
                            d
                        }
                        IrType::Bool => {
                            let d = builder.new_value();
                            builder.emit(Inst::Call { dest: Some(d.clone()), func: "__str_from_bool".into(), args: vec![rv_raw], ret_ty: IrType::Ptr });
                            d
                        }
                        _ => rv_raw,
                    };
                    let dest = builder.new_value();
                    builder.emit(Inst::Call {
                        dest:   Some(dest.clone()),
                        func:   "__str_concat".into(),
                        args:   vec![lv, rv],
                        ret_ty: IrType::Ptr,
                    });
                    return dest;
                }
            }

            let left_ty  = expr_ir_type(builder, left);
            let right_ty = expr_ir_type(builder, right);
            // Arithmétique float si au moins un côté est F64
            let ty = if matches!(left_ty, IrType::F64) || matches!(right_ty, IrType::F64) {
                IrType::F64
            } else {
                IrType::I64
            };
            let lv = lower_expr(builder, left);
            let rv = lower_expr(builder, right);
            let dest = builder.new_value();
            
            // Opérateurs stricts : appel aux fonctions runtime
            match op {
                BinOp::EqEqEq => {
                    builder.emit(Inst::Call {
                        dest: Some(dest.clone()),
                        func: "__cmp_eq_strict".to_string(),
                        args: vec![lv, rv],
                        ret_ty: IrType::Bool,
                    });
                    return dest;
                }
                BinOp::NotEqEq => {
                    builder.emit(Inst::Call {
                        dest: Some(dest.clone()),
                        func: "__cmp_ne_strict".to_string(),
                        args: vec![lv, rv],
                        ret_ty: IrType::Bool,
                    });
                    return dest;
                }
                BinOp::LtEqEq => {
                    builder.emit(Inst::Call {
                        dest: Some(dest.clone()),
                        func: "__cmp_le_strict".to_string(),
                        args: vec![lv, rv],
                        ret_ty: IrType::Bool,
                    });
                    return dest;
                }
                BinOp::GtEqEq => {
                    builder.emit(Inst::Call {
                        dest: Some(dest.clone()),
                        func: "__cmp_ge_strict".to_string(),
                        args: vec![lv, rv],
                        ret_ty: IrType::Bool,
                    });
                    return dest;
                }
                _ => {}
            }
            
            let inst = match op {
                BinOp::Add   => Inst::Add { dest: dest.clone(), lhs: lv, rhs: rv, ty },
                BinOp::Sub   => Inst::Sub { dest: dest.clone(), lhs: lv, rhs: rv, ty },
                BinOp::Mul   => Inst::Mul { dest: dest.clone(), lhs: lv, rhs: rv, ty },
                BinOp::Div   => Inst::Div { dest: dest.clone(), lhs: lv, rhs: rv, ty },
                BinOp::Mod   => Inst::Mod { dest: dest.clone(), lhs: lv, rhs: rv, ty },
                BinOp::EqEq  => Inst::CmpEq { dest: dest.clone(), lhs: lv, rhs: rv, ty },
                BinOp::NotEq => Inst::CmpNe { dest: dest.clone(), lhs: lv, rhs: rv, ty },
                BinOp::Lt    => Inst::CmpLt { dest: dest.clone(), lhs: lv, rhs: rv, ty },
                BinOp::LtEq  => Inst::CmpLe { dest: dest.clone(), lhs: lv, rhs: rv, ty },
                BinOp::Gt    => Inst::CmpGt { dest: dest.clone(), lhs: lv, rhs: rv, ty },
                BinOp::GtEq  => Inst::CmpGe { dest: dest.clone(), lhs: lv, rhs: rv, ty },
                BinOp::And   => Inst::And { dest: dest.clone(), lhs: lv, rhs: rv },
                BinOp::Or    => Inst::Or  { dest: dest.clone(), lhs: lv, rhs: rv },
                _ => unreachable!("strict operators handled above"),
            };
            builder.emit(inst);
            dest
        }

        // ── Opération unaire ──────────────────────────────────────────────────
        Expr::Unary { op, operand, .. } => {
            let src = lower_expr(builder, operand);
            let dest = builder.new_value();
            let inst = match op {
                UnaryOp::Neg => Inst::Neg { dest: dest.clone(), src, ty: IrType::I64 },
                UnaryOp::Not => Inst::Not { dest: dest.clone(), src },
            };
            builder.emit(inst);
            dest
        }

        // ── Tableau littéral ─────────────────────────────────────────────────
        Expr::Array { elements, .. } => {
            // Alloue un tableau via __array_new(), pousse chaque élément
            let arr = builder.new_value();
            builder.emit(Inst::Call {
                dest:   Some(arr.clone()),
                func:   "__array_new".into(),
                args:   vec![],
                ret_ty: IrType::Ptr,
            });
            for elem in elements {
                let elem_ty = expr_ir_type(builder, elem);
                let v = lower_expr(builder, elem);
                // Convertir F64 et Bool en string pour stockage uniforme dans mixed[]
                let stored = match elem_ty {
                    IrType::F64 => {
                        let s = builder.new_value();
                        builder.emit(Inst::Call {
                            dest:   Some(s.clone()),
                            func:   "__str_from_float".into(),
                            args:   vec![v],
                            ret_ty: IrType::Ptr,
                        });
                        s
                    }
                    IrType::Bool => {
                        let s = builder.new_value();
                        builder.emit(Inst::Call {
                            dest:   Some(s.clone()),
                            func:   "__str_from_bool".into(),
                            args:   vec![v],
                            ret_ty: IrType::Ptr,
                        });
                        s
                    }
                    _ => v,
                };
                builder.emit(Inst::Call {
                    dest:   None,
                    func:   "__array_push".into(),
                    args:   vec![arr.clone(), stored],
                    ret_ty: IrType::Void,
                });
            }
            arr
        }

        // ── Map littéral ──────────────────────────────────────────────────────
        Expr::Map { entries, .. } => {
            // Alloue une map via __map_new(), puis insère chaque entrée
            let map = builder.new_value();
            builder.emit(Inst::Call {
                dest:   Some(map.clone()),
                func:   "__map_new".into(),
                args:   vec![],
                ret_ty: IrType::Ptr,
            });
            for (key, val) in entries {
                let kv = lower_expr(builder, key);
                // Convertit F64/Bool en string avant stockage (comme pour les arrays)
                let val_ty = expr_ir_type(builder, val);
                let vv_raw = lower_expr(builder, val);
                let vv = match val_ty {
                    IrType::F64 => {
                        let s = builder.new_value();
                        builder.emit(Inst::Call {
                            dest:   Some(s.clone()),
                            func:   "__str_from_float".into(),
                            args:   vec![vv_raw],
                            ret_ty: IrType::Ptr,
                        });
                        s
                    }
                    IrType::Bool => {
                        let s = builder.new_value();
                        builder.emit(Inst::Call {
                            dest:   Some(s.clone()),
                            func:   "__str_from_bool".into(),
                            args:   vec![vv_raw],
                            ret_ty: IrType::Ptr,
                        });
                        s
                    }
                    _ => vv_raw,
                };
                builder.emit(Inst::Call {
                    dest:   None,
                    func:   "__map_set".into(),
                    args:   vec![map.clone(), kv, vv],
                    ret_ty: IrType::Void,
                });
            }
            map
        }

        // ── Accès par index ───────────────────────────────────────────────────
        Expr::Index { object, index, .. } => {
            let obj_val = lower_expr(builder, object);
            let idx_val = lower_expr(builder, index);
            let dest = builder.new_value();
            // Détermine si c'est un accès map ou array selon le type de la variable
            let is_map = match object.as_ref() {
                Expr::Ident(name, _) => builder.map_vars.contains(name.as_str()),
                _ => false,
            };
            let func = if is_map { "__map_get" } else { "__array_get" };
            builder.emit(Inst::Call {
                dest:   Some(dest.clone()),
                func:   func.into(),
                args:   vec![obj_val, idx_val],
                ret_ty: IrType::Ptr,
            });
            dest
        }

        // ── Plage ─────────────────────────────────────────────────────────────
        Expr::Range { start, end, .. } => {
            let sv = lower_expr(builder, start);
            let ev = lower_expr(builder, end);
            let dest = builder.new_value();
            // Appel d'un builtin __range(start, end) → tableau d'entiers
            builder.emit(Inst::Call {
                dest:   Some(dest.clone()),
                func:   "__range".into(),
                args:   vec![sv, ev],
                ret_ty: IrType::Ptr,
            });
            dest
        }

        // ── Match expression ──────────────────────────────────────────────────
        Expr::Match { subject, arms, .. } => {
            let subj = lower_expr(builder, subject);
            let result_slot = builder.new_value();
            builder.emit(Inst::Alloca { dest: result_slot.clone(), ty: IrType::Ptr });

            let merge_bb = builder.new_block();
            let mut arm_blocks: Vec<(crate::ir::inst::BlockId, Value)> = Vec::new();

            for arm in arms {
                let arm_bb = builder.new_block();
                // Test du pattern (sauf default)
                if let Some(pat) = &arm.pattern {
                    match pat {
                        MatchPattern::Literal(lit) => {
                            // Pattern littéral : comparaison directe
                            let pat_val = lower_literal(builder, lit);
                            let test = builder.new_value();
                            builder.emit(Inst::CmpEq {
                                dest: test.clone(),
                                lhs:  subj.clone(),
                                rhs:  pat_val,
                                ty:   IrType::I64,
                            });
                            let next_bb = builder.new_block();
                            builder.emit(Inst::Branch {
                                cond:    test,
                                then_bb: arm_bb.clone(),
                                else_bb: next_bb.clone(),
                            });
                            builder.switch_to(&arm_bb);
                            let arm_val = lower_expr(builder, &arm.body);
                            builder.emit(Inst::Store { ptr: result_slot.clone(), src: arm_val.clone() });
                            if !builder.is_terminated() {
                                builder.emit(Inst::Jump { target: merge_bb.clone() });
                            }
                            arm_blocks.push((arm_bb, arm_val));
                            builder.switch_to(&next_bb);
                        }
                        MatchPattern::IsType(ty) => {
                            // Pattern de type : test runtime avec `is Type`
                            let test = lower_is_check(builder, &subj, ty);
                            let next_bb = builder.new_block();
                            builder.emit(Inst::Branch {
                                cond:    test,
                                then_bb: arm_bb.clone(),
                                else_bb: next_bb.clone(),
                            });
                            builder.switch_to(&arm_bb);
                            let arm_val = lower_expr(builder, &arm.body);
                            builder.emit(Inst::Store { ptr: result_slot.clone(), src: arm_val.clone() });
                            if !builder.is_terminated() {
                                builder.emit(Inst::Jump { target: merge_bb.clone() });
                            }
                            arm_blocks.push((arm_bb, arm_val));
                            builder.switch_to(&next_bb);
                        }
                    }
                } else {
                    // default
                    builder.emit(Inst::Jump { target: arm_bb.clone() });
                    builder.switch_to(&arm_bb);
                    let arm_val = lower_expr(builder, &arm.body);
                    builder.emit(Inst::Store { ptr: result_slot.clone(), src: arm_val.clone() });
                    if !builder.is_terminated() {
                        builder.emit(Inst::Jump { target: merge_bb.clone() });
                    }
                }
            }

            if !builder.is_terminated() {
                builder.emit(Inst::Jump { target: merge_bb.clone() });
            }
            builder.switch_to(&merge_bb);

            let dest = builder.new_value();
            builder.emit(Inst::Load { dest: dest.clone(), ptr: result_slot, ty: IrType::Ptr });
            dest
        }

        // ── Chaîne template `${expr}` ─────────────────────────────────────
        Expr::Template { parts, .. } => {
            // Dérouler en concaténations successives via __str_concat
            let mut acc = {
                let idx = builder.module.intern_string("");
                let d = builder.new_value();
                builder.emit(Inst::ConstStr { dest: d.clone(), idx });
                d
            };
            for part in parts {
                // Valeur brute de la partie + détection tableau
                let (raw_val, part_ty, is_arr) = match part {
                    TemplatePartExpr::Literal(s) => {
                        let idx = builder.module.intern_string(s);
                        let d = builder.new_value();
                        builder.emit(Inst::ConstStr { dest: d.clone(), idx });
                        (d, IrType::Ptr, false)
                    }
                    TemplatePartExpr::Expr(e) => {
                        let ty = expr_ir_type(builder, e);
                        let is_arr = is_array_expr(builder, e);
                        let v = lower_expr(builder, e);
                        (v, ty, is_arr)
                    }
                };

                // Convertir en string si nécessaire
                let str_val = if is_arr {
                    // Tableau → formatage [a, b, c]
                    let d = builder.new_value();
                    builder.emit(Inst::Call {
                        dest:   Some(d.clone()),
                        func:   "__array_to_str".into(),
                        args:   vec![raw_val],
                        ret_ty: IrType::Ptr,
                    });
                    d
                } else { match part_ty {
                    IrType::F64 => {
                        // Float stocké en I64 bitcasté → rebitcast en F64 puis __str_from_float
                        let as_f64 = builder.new_value();
                        builder.emit(Inst::Call {
                            dest:   Some(as_f64.clone()),
                            func:   "__str_from_float".into(),
                            args:   vec![raw_val],
                            ret_ty: IrType::Ptr,
                        });
                        as_f64
                    }
                    IrType::Bool => {
                        let as_str = builder.new_value();
                        builder.emit(Inst::Call {
                            dest:   Some(as_str.clone()),
                            func:   "__str_from_bool".into(),
                            args:   vec![raw_val],
                            ret_ty: IrType::Ptr,
                        });
                        as_str
                    }
                    IrType::I64 => {
                        // Entier → __str_from_int (sans heuristique pointeur)
                        let as_str = builder.new_value();
                        builder.emit(Inst::Call {
                            dest:   Some(as_str.clone()),
                            func:   "__str_from_int".into(),
                            args:   vec![raw_val],
                            ret_ty: IrType::Ptr,
                        });
                        as_str
                    }
                    _ => raw_val, // Ptr : déjà une string
                }};

                let dest = builder.new_value();
                builder.emit(Inst::Call {
                    dest:   Some(dest.clone()),
                    func:   "__str_concat".into(),
                    args:   vec![acc, str_val],
                    ret_ty: IrType::Ptr,
                });
                acc = dest;
            }
            acc
        }

        // ── Fonction anonyme (closure) ─────────────────────────────────────
        Expr::Nameless { params, ret_ty, body, .. } => {
            let actual_ret_ty = ret_ty.as_ref()
                .map(|t| IrType::from_ast(t))
                .unwrap_or(IrType::Ptr);

            // Analyser les captures
            let param_names: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
            let captures = collect_captures(body, &param_names, &builder.locals);
            
            // Collecter les valeurs par défaut des paramètres
            let has_defaults = params.iter().any(|p| p.default_value.is_some());

            // Générer un nom unique
            let anon_name = {
                let count = builder.module.anon_counter;
                builder.module.anon_counter += 1;
                format!("__anon_{}", count)
            };

            // Cloner les données nécessaires avant d'emprunter builder.module
            let fn_ret_types_clone   = builder.fn_ret_types.clone();
            let fn_param_types_clone = builder.fn_param_types.clone();
            let current_class        = builder.current_class.clone();
            let var_class_snap       = builder.var_class.clone();
            let func_vars_snap       = builder.func_vars.clone();

            // Collecter les heap pointers des captures.
            // Pour chaque variable capturée :
            //   - si déjà promue (heap_promoted) → le slot EST déjà le heap pointer
            //   - si c'est un slot stack (Alloca) → allouer une cellule sur le tas, y copier
            //     la valeur courante, rediriger `locals[name]` vers le heap pointer.
            // Ainsi le scope extérieur et la closure partagent la même cellule heap :
            // toute mutation ultérieure de la variable dans le scope extérieur sera
            // visible depuis la closure, et vice-versa.
            let capture_vals: Vec<Value> = captures.iter().map(|(cap_name, _)| {
                // Déjà promu par une closure précédente dans la même fonction
                if builder.heap_promoted.contains(cap_name.as_str()) {
                    return builder.slot_of_local(cap_name)
                        .unwrap_or_else(|| { let d = builder.new_value(); builder.emit(Inst::Nop); d });
                }
                // Variable locale stack → promouvoir au tas
                if let Some((slot, ty, mutable)) = builder.locals.get(cap_name.as_str()).cloned() {
                    let size = builder.new_value();
                    builder.emit(Inst::ConstInt { dest: size.clone(), value: 8 });
                    let heap_ptr = builder.new_value();
                    builder.emit(Inst::Call {
                        dest:   Some(heap_ptr.clone()),
                        func:   "__alloc_obj".into(),
                        args:   vec![size],
                        ret_ty: IrType::Ptr,
                    });
                    // Copier la valeur courante (stack → heap)
                    let cur_val = builder.new_value();
                    builder.emit(Inst::Load { dest: cur_val.clone(), ptr: slot, ty: ty.clone() });
                    builder.emit(Inst::Store { ptr: heap_ptr.clone(), src: cur_val });
                    // Rediriger les futurs accès dans le scope extérieur vers le heap
                    builder.locals.insert(cap_name.clone(), (heap_ptr.clone(), ty, mutable));
                    builder.heap_promoted.insert(cap_name.clone());
                    heap_ptr
                } else if let Some((env_val, idx, _)) = builder.captured_vars.get(cap_name.as_str()).cloned() {
                    // Closure imbriquée : récupérer le heap pointer depuis l'env parent
                    let ptr = builder.new_value();
                    builder.emit(Inst::GetField {
                        dest:   ptr.clone(),
                        obj:    env_val,
                        field:  format!("__cap_{}", idx),
                        ty:     IrType::Ptr,
                        offset: (idx * 8) as i32,
                    });
                    ptr
                } else {
                    let d = builder.new_value();
                    builder.emit(Inst::Nop);
                    d
                }
            }).collect();

            // Générer la fonction anonyme (emprunt temporaire de builder.module)
            lower_nameless_fn(
                builder.module,
                &anon_name,
                params,
                actual_ret_ty.clone(),
                body,
                &captures,
                &fn_ret_types_clone,
                &fn_param_types_clone,
                &current_class,
                &var_class_snap,
                &func_vars_snap,
                has_defaults,
            );

            // Créer l'env avec les valeurs capturées ET les valeurs par défaut
            let env_ptr_val = if captures.is_empty() && !has_defaults {
                let z = builder.new_value();
                builder.emit(Inst::ConstInt { dest: z.clone(), value: 0 });
                z
            } else {
                let env_class = format!("__env_{}", anon_name);
                let env = builder.new_value();
                builder.emit(Inst::Alloc { dest: env.clone(), class: env_class });
                
                // Stocker les captures
                for (i, _) in captures.iter().enumerate() {
                    builder.emit(Inst::SetField {
                        obj:    env.clone(),
                        field:  format!("__cap_{}", i),
                        src:    capture_vals[i].clone(),
                        offset: (i * 8) as i32,
                    });
                }
                
                // Stocker les valeurs par défaut des paramètres
                if has_defaults {
                    let default_offset = captures.len();
                    for (i, param) in params.iter().enumerate() {
                        if let Some(ref default_expr) = param.default_value {
                            let default_val = lower_expr(builder, default_expr);
                            builder.emit(Inst::SetField {
                                obj:    env.clone(),
                                field:  format!("__default_{}", i),
                                src:    default_val,
                                offset: ((default_offset + i) * 8) as i32,
                            });
                        }
                    }
                }
                
                env
            };

            // Créer le fat pointer {func_addr, env_ptr}
            let func_addr = builder.new_value();
            builder.emit(Inst::FuncAddr { dest: func_addr.clone(), func: anon_name });
            let fat_ptr = builder.new_value();
            builder.emit(Inst::Alloc { dest: fat_ptr.clone(), class: "__fat_ptr".into() });
            builder.emit(Inst::SetField { obj: fat_ptr.clone(), field: "func".into(), src: func_addr,    offset: 0 });
            builder.emit(Inst::SetField { obj: fat_ptr.clone(), field: "env".into(),  src: env_ptr_val, offset: 8 });
            fat_ptr
        }

        Expr::Resolve { expr, .. } => {
            // Déterminer le type de retour original de la fonction async
            let orig_ty = match expr.as_ref() {
                Expr::Ident(var_name, _) => {
                    builder.async_var_ret.get(var_name).cloned().unwrap_or(IrType::I64)
                }
                Expr::Call { callee, .. } => {
                    if let Expr::Ident(fn_name, _) = callee.as_ref() {
                        if builder.async_funcs.contains(fn_name.as_str()) {
                            builder.fn_ret_types.get(fn_name.as_str()).cloned().unwrap_or(IrType::I64)
                        } else {
                            IrType::I64
                        }
                    } else {
                        IrType::I64
                    }
                }
                _ => IrType::I64,
            };

            let task_ptr = lower_expr(builder, expr);
            let raw = builder.new_value();
            builder.emit(Inst::Call {
                dest:   Some(raw.clone()),
                func:   "__task_resolve".into(),
                args:   vec![task_ptr],
                ret_ty: IrType::I64,
            });

            // Unboxer si nécessaire
            match orig_ty {
                IrType::F64 => {
                    let unboxed = builder.new_value();
                    builder.emit(Inst::Call {
                        dest:   Some(unboxed.clone()),
                        func:   "__unbox_float".into(),
                        args:   vec![raw],
                        ret_ty: IrType::F64,
                    });
                    unboxed
                }
                IrType::Bool => {
                    let unboxed = builder.new_value();
                    builder.emit(Inst::Call {
                        dest:   Some(unboxed.clone()),
                        func:   "__unbox_bool".into(),
                        args:   vec![raw],
                        ret_ty: IrType::Bool,
                    });
                    unboxed
                }
                // I64, Ptr (string, array, map, Function, object) : le i64 EST la valeur
                _ => raw,
            }
        }

        Expr::IsCheck { expr, ty, .. } => {
            // Shortcut statique pour `is float` :
            // Les floats directs (f64 bits dans i64) ne portent aucun tag runtime
            // distinguable d'un int. On exploite le type statique connu à la compilation.
            if matches!(ty, Type::Float) {
                let static_ty = expr_ir_type(builder, expr);
                match static_ty {
                    IrType::F64 => {
                        // Statiquement float → toujours vrai
                        let dest = builder.new_value();
                        builder.emit(Inst::ConstBool { dest: dest.clone(), value: true });
                        return dest;
                    }
                    IrType::I64 | IrType::Bool => {
                        // Statiquement int/bool → jamais un float
                        let dest = builder.new_value();
                        builder.emit(Inst::ConstBool { dest: dest.clone(), value: false });
                        return dest;
                    }
                    _ => {
                        // Ptr (mixed) → fallback runtime : détecte les floats boxés
                    }
                }
            }
            let val = lower_expr(builder, expr);
            lower_is_check(builder, &val, ty)
        }
    }
}

