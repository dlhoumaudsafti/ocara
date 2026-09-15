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

/// Convertit un opérande CONNU (I64/F64/Bool/Ptr) vers la représentation
/// "mixed" attendue par `__dyn_add` (voir sa doc, `runtime/src/lib.rs`) — un
/// `Ptr` (mixed, ou un vrai string/array/map/objet) est déjà dans cette
/// représentation tel quel ; un `F64`/`Bool` connu STATIQUEMENT doit être
/// boxé au préalable (comme `box_for_any` le fait à l'affectation) ; un
/// `I64` doit l'être aussi, mais seulement si assez grand pour être ambigu
/// avec un pointeur heap — décision prise au runtime par
/// `__box_int_for_mixed` (voir `box_int_if_needed`), pas ici, pour ne pas
/// payer une allocation sur le cas courant d'un petit entier. Aussi utilisée
/// pour boxer un élément `F64`/`Bool`/`I64` d'un littéral `array<mixed>`/
/// `map<K,mixed>` (voir `lower_array_literal`/`lower_map_literal`).
fn box_for_dyn_arith(builder: &mut LowerBuilder, ty: &IrType, val: Value) -> Value {
    match ty {
        IrType::F64 => {
            let d = builder.new_value();
            builder.emit(Inst::Call { dest: Some(d.clone()), func: "__box_float".into(), args: vec![val], ret_ty: IrType::Ptr });
            d
        }
        IrType::Bool => {
            let d = builder.new_value();
            builder.emit(Inst::Call { dest: Some(d.clone()), func: "__box_bool".into(), args: vec![val], ret_ty: IrType::Ptr });
            d
        }
        IrType::I64 => {
            let d = builder.new_value();
            builder.emit(Inst::Call { dest: Some(d.clone()), func: "__box_int_for_mixed".into(), args: vec![val], ret_ty: IrType::Ptr });
            d
        }
        _ => val,
    }
}

/// Déballe un opérande Ptr (mixed) de `-`/`*`/`/`/`%` via `func`
/// (`__mixed_to_int`/`__mixed_to_float`, voir `runtime/src/lib.rs`) —
/// `target_ty` détermine le type IR logique du résultat (purement pour le
/// suivi ; l'ABI réel de l'appel est piloté par le `BuiltinDesc` enregistré,
/// voir `src/codegen/emit.d/instructions.d/calls.rs`).
fn unbox_mixed_operand(builder: &mut LowerBuilder, func: &str, target_ty: &IrType, val: Value) -> Value {
    let d = builder.new_value();
    builder.emit(Inst::Call { dest: Some(d.clone()), func: func.to_string(), args: vec![val], ret_ty: target_ty.clone() });
    d
}

/// Comment traiter un élément `F64`/`Bool` en construisant un littéral
/// `array`/`map` (voir `lower_array_literal`/`lower_map_literal`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LiteralElemKind {
    /// Élément(s) de type `mixed` (ou type de destination inconnu à cet
    /// endroit — nested/argument/retour, voir les sites d'appel) : un
    /// consommateur générique (`JSON::encode`, `__dyn_add`, `Map::forEach`,
    /// `is float`/`is bool`...) doit pouvoir distinguer un `float`/`bool` d'un
    /// entier au runtime — boxé (`__box_float`/`__box_bool`), jamais stringifié.
    Mixed,
    /// Type de destination concret et CONNU (`array<float>`, `map<K,bool>`,
    /// ...) : aucun consommateur n'a besoin de deviner le type, stocké BRUT
    /// sans la moindre conversion — exactement comme `int` (qui n'est jamais
    /// boxé nulle part dans ce compilateur, voir `box_for_any`).
    Concrete,
}

/// Construit un littéral `array` : alloue via `__array_new`, pousse chaque
/// élément. `kind` décide comment un élément `F64`/`Bool` est stocké — voir
/// `LiteralElemKind`. Avant ce correctif, TOUT élément `F64`/`Bool` était
/// systématiquement stringifié (`__str_from_float`/`__str_from_bool`),
/// quel que soit `kind` — un `array<float>` littéral (pas seulement
/// `array<mixed>`) produisait donc un résultat numériquement faux à la
/// lecture (`arr[0]` retournait le pointeur de la string, réinterprété comme
/// bits flottants) : voir docs/roadmap.d/langage-mixed-literal-stringification.md.
pub fn lower_array_literal(builder: &mut LowerBuilder, elements: &[Expr], kind: LiteralElemKind) -> Value {
    let arr = builder.new_value();
    builder.emit(Inst::Call { dest: Some(arr.clone()), func: "__array_new".into(), args: vec![], ret_ty: IrType::Ptr });
    for elem in elements {
        let elem_ty = expr_ir_type(builder, elem);
        let v = lower_expr(builder, elem);
        let stored = match kind {
            LiteralElemKind::Mixed    => box_for_dyn_arith(builder, &elem_ty, v),
            LiteralElemKind::Concrete => v,
        };
        builder.emit(Inst::Call { dest: None, func: "__array_push".into(), args: vec![arr.clone(), stored], ret_ty: IrType::Void });
    }
    arr
}

/// Comme `lower_array_literal`, pour un littéral `map` — `kind` s'applique à
/// la VALEUR de chaque entrée (jamais à la clé, toujours `string`).
pub fn lower_map_literal(builder: &mut LowerBuilder, entries: &[(Expr, Expr)], kind: LiteralElemKind) -> Value {
    let map = builder.new_value();
    builder.emit(Inst::Call { dest: Some(map.clone()), func: "__map_new".into(), args: vec![], ret_ty: IrType::Ptr });
    for (key, val) in entries {
        let kv = lower_expr(builder, key);
        let val_ty = expr_ir_type(builder, val);
        let vv_raw = lower_expr(builder, val);
        let vv = match kind {
            LiteralElemKind::Mixed    => box_for_dyn_arith(builder, &val_ty, vv_raw),
            LiteralElemKind::Concrete => vv_raw,
        };
        builder.emit(Inst::Call { dest: None, func: "__map_set".into(), args: vec![map.clone(), kv, vv], ret_ty: IrType::Void });
    }
    map
}

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

        // ── parent ───────────────────────────────────────────────────────────
        Expr::ParentExpr(_) => {
            // `parent` référence aussi self (même objet, champs hérités inclus)
            if let Some((dest, _)) = builder.load_local("self") {
                dest
            } else {
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
                Expr::ParentExpr(_)  => builder.parent_class.clone(),
                // Accès chaîné (`a.b.c` où `b` est elle-même une instance de
                // classe) — voir resolve_chained_field_class pour le bug que
                // ça corrige.
                Expr::Field { object: inner, field: inner_field, .. } => {
                    resolve_chained_field_class(builder, inner, inner_field)
                }
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
                        Expr::ParentExpr(_) => builder.parent_class.clone(),
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
                        // Accès chaîné : w.inner.sum() où `inner` est elle-même
                        // une instance de classe/string/array/map — DOIT être
                        // vérifié avant le fallback générique ci-dessous, qui
                        // devinait "String" à tort pour ce cas (bug historique,
                        // voir resolve_chained_field_class).
                        Expr::Field { object: inner_obj, field: inner_field, .. } => {
                            resolve_chained_field_class(builder, inner_obj, inner_field)
                        }
                        // Appel de fonction retournant string : func().trim()
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
                    // Cas spécial : ui.handler(name, Class::method) — génère un trampoline
                    // dédié plutôt que de passer par le dispatch générique ci-dessous
                    // (voir tauri_handler.rs pour le détail du mécanisme).
                    if let Some(dest) = crate::lower::expr::tauri_handler::try_lower_tauri_handler_call(
                        builder, &class_name, field, object, args,
                    ) {
                        return dest;
                    }
                    // Cas spécial : ui.handlers({"nom": Class::method, ...}) — désucré vers
                    // plusieurs try_lower_tauri_handler_call (voir tauri_handler.rs).
                    if let Some(dest) = crate::lower::expr::tauri_handler::try_lower_tauri_handlers_call(
                        builder, &class_name, field, object, args,
                    ) {
                        return dest;
                    }

                    let mut func_mangled = if let Some(ref cls) = class_name {
                        format!("{}_{}", cls, field)
                    } else {
                        format!("_method_{}", field) // fallback (ne devrait pas arriver)
                    };
                    
                    // Chercher la méthode dans la chaîne d'héritage si elle n'existe pas dans la classe
                    if let Some(cls) = &class_name {
                        let method_exists = builder.module.functions.iter()
                            .any(|f| f.name == func_mangled);
                        
                        if !method_exists {
                            // Chercher dans la chaîne d'héritage
                            let mut current = cls.as_str();
                            loop {
                                if let Some(parent) = builder.module.class_parents.get(current) {
                                    let parent_func_name = format!("{}_{}", parent, field);
                                    // Vérifier dans les fonctions du module ET dans les builtins
                                    let parent_method_exists = builder.module.functions.iter()
                                        .any(|f| f.name == parent_func_name)
                                        || builtins().iter().any(|b| b.name == parent_func_name);
                                    if parent_method_exists {
                                        func_mangled = parent_func_name;
                                        break;
                                    }
                                    current = parent.as_str();
                                } else {
                                    break;
                                }
                            }
                        }
                    }
                    
                    // Compléter les arguments avec les valeurs par défaut si nécessaire
                    let completed_args = complete_args_with_defaults(builder, &func_mangled, args);

                    let obj_val = lower_expr(builder, object);
                    let dest = builder.new_value();
                    // Boxer F64/Bool/I64 si le paramètre cible est `mixed`
                    // (Ptr) — voir `box_arg_for_mixed_param`. Basé sur
                    // `func_mangled` (la méthode CONCRÈTE résolue) : même
                    // signature qu'un éventuel dispatcher dynamique
                    // (`call_target` ci-dessous), qui se contente de la
                    // relayer sans jamais la modifier.
                    let arg_vals: Vec<Value> = completed_args.iter().enumerate().map(|(i, a)| {
                        let raw = lower_expr(builder, a);
                        let arg_ty = expr_ir_type(builder, a);
                        let param_ty = param_type_for_call_arg(builder, &func_mangled, i);
                        box_arg_for_mixed_param(builder, param_ty, &arg_ty, raw)
                    }).collect();
                    let mut all_args = vec![obj_val];
                    all_args.extend(arg_vals);
                    // Résoudre le type de retour depuis fn_ret_types
                    let ret_ty = builder.fn_ret_types.get(&func_mangled).cloned().unwrap_or(IrType::Ptr);
                    // Dispatch dynamique réel (héritage de classe) : un appel
                    // EXTERNE (jamais `self`/`parent`, qui visent toujours
                    // l'implémentation exacte de la classe courante/parente)
                    // sur une classe qui a des sous-classes est redirigé vers
                    // son dispatcher `__dispatch_Classe_méthode` — sans quoi
                    // l'appel résoudrait TOUJOURS vers `class_name`, jamais
                    // vers une éventuelle surcharge du type réel de l'objet
                    // (voir docs/roadmap.d/langage-interfaces.md).
                    let is_self_or_parent = matches!(object.as_ref(), Expr::SelfExpr(_) | Expr::ParentExpr(_));
                    let call_target = if is_self_or_parent {
                        func_mangled.clone()
                    } else {
                        class_name.as_deref()
                            .and_then(|cls| crate::lower::builder::class_dispatch::class_dispatcher_name(builder.module, cls, field))
                            .unwrap_or_else(|| func_mangled.clone())
                    };
                    builder.emit(Inst::Call {
                        dest:   Some(dest.clone()),
                        func:   call_target,
                        args:   all_args,
                        ret_ty,
                    });
                    // Finalisation manuelle d'une ressource `scoped`/`consumed`
                    // suivie (Mutex::destroy, SQLite/MySQL/MariaDB::close) :
                    // marquer comme déjà détruite pour que la destruction
                    // automatique de fin de bloc ne la libère pas une seconde
                    // fois (double-free confirmé sans cette marque — voir
                    // docs/roadmap.d/memoire-double-free-et-fuites-scoped.md).
                    if let Expr::Ident(var_name, _) = object.as_ref() {
                        let is_manual_finalizer = matches!(
                            (class_name.as_deref(), field.as_str()),
                            (Some("Mutex"), "destroy")
                                | (Some("SQLite"), "close")
                                | (Some("MySQL"), "close")
                                | (Some("MariaDB"), "close")
                        );
                        if is_manual_finalizer {
                            if let Some(info) = builder.owned_locals.get_mut(var_name.as_str()) {
                                info.dropped = true;
                            }
                        }
                    }
                    return dest;
                }
                _ => "_unknown".into(),
            };

            // Compléter les arguments avec les valeurs par défaut si nécessaire
            let completed_args = complete_args_with_defaults(builder, &func_name, args);
            let args = &completed_args; // Remplacer args par completed_args pour le reste

            // Pour les builtins avec paramètres optionnels (surcharges),
            // ajouter un suffixe _N où N est le nombre d'arguments réels
            let func_name = {
                let original_name = func_name.clone();
                // Chercher si c'est un builtin
                if let Some(builtin) = builtins().iter().find(|b| b.name == original_name) {
                    // Vérifier si c'est un builtin avec surcharge (required < fixed)
                    // Pour l'instant, on détecte les surcharges en cherchant si une variante _0 existe
                    let has_overload = builtins().iter().any(|b| b.name == format!("{}_0", original_name));
                    if has_overload && args.len() < builtin.params.len() {
                        // Utiliser la variante surchargée
                        format!("{}_{}", original_name, args.len())
                    } else {
                        original_name
                    }
                } else {
                    original_name
                }
            };

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
                // Allouer l'env heap : n_args * 8 octets (min 8 pour éviter null pointer)
                let env_size = builder.new_value();
                builder.emit(Inst::ConstInt { dest: env_size.clone(), value: ((n_args * 8).max(8)) as i64 });
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

            // Boxer F64/Bool/I64 si le paramètre cible est `mixed` (Ptr) —
            // voir `box_arg_for_mixed_param`.
            let arg_vals: Vec<Value> = args.iter().enumerate().map(|(i, a)| {
                let raw = lower_expr(builder, a);
                let arg_ty = expr_ir_type(builder, a);
                let param_ty = param_type_for_call_arg(builder, &func_name, i);
                box_arg_for_mixed_param(builder, param_ty, &arg_ty, raw)
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
                        
                        // Boxer F64/Bool/I64 (si assez grand, voir
                        // `__box_int_for_mixed`) pour stockage dans mixed[]
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
                            IrType::I64 => {
                                let boxed = builder.new_value();
                                builder.emit(Inst::Call {
                                    dest:   Some(boxed.clone()),
                                    func:   "__box_int_for_mixed".into(),
                                    args:   vec![variadic_arg.clone()],
                                    ret_ty: IrType::Ptr,
                                });
                                boxed
                            }
                            _ => variadic_arg.clone(),  // Ptr, etc. → stockage direct
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
            // Résoudre "<parent>" et "<self>" vers les classes appropriées
            let self_class;
            let parent_class;
            let mut resolved_class: &str = if class == "<parent>" {
                parent_class = builder.parent_class.clone().unwrap_or_default();
                &parent_class
            } else if class == "<self>" {
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

            // Pour les builtins avec paramètres optionnels (surcharges),
            // ajouter un suffixe _N où N est le nombre d'arguments réels
            let func_name = {
                let original_name = func_name.clone();
                // Chercher si c'est un builtin
                if let Some(builtin) = builtins().iter().find(|b| b.name == original_name) {
                    // Vérifier si c'est un builtin avec surcharge (une variante _0 existe)
                    let has_overload = builtins().iter().any(|b| b.name == format!("{}_0", original_name));
                    if has_overload && args.len() < builtin.params.len() {
                        // Utiliser la variante surchargée
                        format!("{}_{}", original_name, args.len())
                    } else {
                        original_name
                    }
                } else {
                    original_name
                }
            };

            // Vérifier si c'est une méthode async
            if builder.async_funcs.contains(func_name.as_str()) {
                let wrapper_name = format!("__async_wrap_{}", func_name);
                // Évaluer les arguments
                let mut arg_vals: Vec<Value> = args.iter().map(|a| lower_expr(builder, a)).collect();
                
                // Si c'est un appel parent::method(), il faut ajouter self comme premier argument
                if class == "<parent>" {
                    if let Some((self_val, _)) = builder.load_local("self") {
                        arg_vals.insert(0, self_val);
                    }
                }
                
                let n_args = arg_vals.len();
                // Allouer l'env heap : n_args * 8 octets (min 8 pour éviter null pointer)
                let env_size = builder.new_value();
                builder.emit(Inst::ConstInt { dest: env_size.clone(), value: ((n_args * 8).max(8)) as i64 });
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

            // Boxer F64/Bool/I64 si le paramètre cible est `mixed` (Ptr) —
            // voir `box_arg_for_mixed_param`. Un appel statique (builtin OU
            // classe utilisateur) ne boxait jusqu'ici AUCUN argument, quel
            // que soit le paramètre visé (confirmé faux par reproduction) —
            // voir docs/roadmap.d/memoire-fiabilite-runtime-bas-niveau.md.
            let arg_vals: Vec<Value> = args.iter().enumerate().map(|(i, a)| {
                let raw = lower_expr(builder, a);
                let arg_ty = expr_ir_type(builder, a);
                let param_ty = param_type_for_call_arg(builder, &func_name, i);
                box_arg_for_mixed_param(builder, param_ty, &arg_ty, raw)
            }).collect();

            // Si c'est un appel parent::method(), il faut ajouter self comme premier argument
            // car les méthodes d'instance prennent toujours self en premier paramètre
            let final_args = if class == "<parent>" {
                // Charger self et l'insérer en tête des arguments
                if let Some((self_val, _)) = builder.load_local("self") {
                    let mut all_args = vec![self_val];
                    all_args.extend(arg_vals);
                    all_args
                } else {
                    arg_vals
                }
            } else {
                arg_vals
            };
            
            // Vérifier si le builtin retourne void
            if is_void_builtin(&func_name) {
                builder.emit(Inst::Call {
                    dest:   None,
                    func:   func_name,
                    args:   final_args,
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
                args:   final_args,
                ret_ty: IrType::Ptr,
            });
            dest
        }

        // ── Lecture de constante de classe : `Class::NAME` ────────────────────
        Expr::StaticConst { class, name, .. } => {
            // Résoudre "<parent>" et "<self>" vers les classes appropriées
            let self_class;
            let parent_class;
            let class: &str = if class == "<parent>" {
                parent_class = builder.parent_class.clone().unwrap_or_default();
                &parent_class
            } else if class == "<self>" {
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
                let param_ty = ctor_params.get(i).cloned();
                ctor_args.push(box_arg_for_mixed_param(builder, param_ty, &arg_ty, val));
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
            let left_ty  = expr_ir_type(builder, left);
            let right_ty = expr_ir_type(builder, right);

            // `+` avec au moins un opérande Ptr (string/array/map/objet, ou
            // `mixed` — indistinguable de ces derniers au niveau du type IR
            // statique) : décidé DYNAMIQUEMENT par `__dyn_add` (concaténation
            // si un côté est réellement un objet tas, addition numérique
            // sinon) plutôt que de supposer systématiquement une
            // concaténation comme avant ce correctif — ce qui produisait un
            // résultat numérique faux pour un `mixed` contenant un nombre
            // (voir docs/roadmap.d/langage-mixed-arithmetic.md). Un opérande
            // à type CONNU F64/Bool (jamais taggé, contrairement à la même
            // valeur stockée dans un `mixed`) est d'abord boxé pour rejoindre
            // la même représentation "mixed" que `__dyn_add` attend des deux
            // côtés — I64/Ptr sont déjà dans cette représentation tels quels.
            if matches!(op, BinOp::Add) && (matches!(left_ty, IrType::Ptr) || matches!(right_ty, IrType::Ptr)) {
                let lv_raw = lower_expr(builder, left);
                let rv_raw = lower_expr(builder, right);
                let lv = box_for_dyn_arith(builder, &left_ty, lv_raw);
                let rv = box_for_dyn_arith(builder, &right_ty, rv_raw);
                let dest = builder.new_value();
                builder.emit(Inst::Call {
                    dest:   Some(dest.clone()),
                    func:   "__dyn_add".into(),
                    args:   vec![lv, rv],
                    ret_ty: IrType::Ptr,
                });
                return dest;
            }

            // ── Comparaisons (equal/not equal/smaller/greater/smaller or equal/
            //    greater or equal) : toujours typées à la compilation (sema a
            //    déjà rejeté toute paire incompatible sauf int/float et mixed).
            if matches!(op, BinOp::Equal | BinOp::NotEqual |
                        BinOp::Smaller | BinOp::Greater | BinOp::SmallerOrEqual | BinOp::GreaterOrEqual)
            {
                let lv_raw = lower_expr(builder, left);
                let rv_raw = lower_expr(builder, right);
                let dest = builder.new_value();

                // `mixed` des deux côtés (représenté en Ptr, comme string/array/
                // map/objet) : sema n'a pas pu vérifier statiquement — on retombe
                // sur le contrôle de type au runtime (mêmes fonctions déjà
                // utilisées côté strings pour equal/not equal).
                let mixed_or_heap_side = matches!(left_ty, IrType::Ptr) || matches!(right_ty, IrType::Ptr);
                if mixed_or_heap_side {
                    let func = match op {
                        BinOp::Equal          => "__cmp_eq_strict",
                        BinOp::NotEqual        => "__cmp_ne_strict",
                        BinOp::Smaller         => "__cmp_lt_strict",
                        BinOp::Greater          => "__cmp_gt_strict",
                        BinOp::SmallerOrEqual   => "__cmp_le_strict",
                        BinOp::GreaterOrEqual   => "__cmp_ge_strict",
                        _ => unreachable!(),
                    };
                    // Un opérande à type CONNU F64/Bool/I64 comparé à un
                    // `mixed` de l'autre côté doit d'abord rejoindre la même
                    // représentation "mixed" (voir `box_for_dyn_arith`) — sans
                    // ça, `__cmp_*_strict`/`get_value_type` reçoit un F64/Bool
                    // brut (jamais taggé, confondu avec un entier) ou un I64
                    // assez grand pour être confondu avec un pointeur heap
                    // (SEGFAULT confirmé par reproduction : `n equal 1000000`
                    // avec `n:mixed`, voir
                    // docs/roadmap.d/memoire-fiabilite-runtime-bas-niveau.md).
                    // Un opérande déjà Ptr (mixed, ou un vrai string/array/...)
                    // n'est pas affecté (`box_for_dyn_arith` no-op pour Ptr).
                    let lv = box_for_dyn_arith(builder, &left_ty, lv_raw);
                    let rv = box_for_dyn_arith(builder, &right_ty, rv_raw);
                    builder.emit(Inst::Call {
                        dest: Some(dest.clone()),
                        func: func.to_string(),
                        args: vec![lv, rv],
                        ret_ty: IrType::Bool,
                    });
                    return dest;
                }

                // int/float : widening explicite du côté entier (jamais un bitcast —
                // __int_to_float effectue une vraie conversion numérique) avant
                // de comparer en F64. Même type des deux côtés (int/int, float/float,
                // bool/bool) : comparaison directe, aucune conversion.
                let (lv, rv, ty) = match (&left_ty, &right_ty) {
                    (IrType::I64, IrType::F64) => {
                        let conv = builder.new_value();
                        builder.emit(Inst::Call {
                            dest: Some(conv.clone()), func: "__int_to_float".into(),
                            args: vec![lv_raw], ret_ty: IrType::F64,
                        });
                        (conv, rv_raw, IrType::F64)
                    }
                    (IrType::F64, IrType::I64) => {
                        let conv = builder.new_value();
                        builder.emit(Inst::Call {
                            dest: Some(conv.clone()), func: "__int_to_float".into(),
                            args: vec![rv_raw], ret_ty: IrType::F64,
                        });
                        (lv_raw, conv, IrType::F64)
                    }
                    (IrType::F64, IrType::F64) => (lv_raw, rv_raw, IrType::F64),
                    _                          => (lv_raw, rv_raw, IrType::I64),
                };

                let inst = match op {
                    BinOp::Equal          => Inst::CmpEq { dest: dest.clone(), lhs: lv, rhs: rv, ty },
                    BinOp::NotEqual        => Inst::CmpNe { dest: dest.clone(), lhs: lv, rhs: rv, ty },
                    BinOp::Smaller         => Inst::CmpLt { dest: dest.clone(), lhs: lv, rhs: rv, ty },
                    BinOp::Greater          => Inst::CmpGt { dest: dest.clone(), lhs: lv, rhs: rv, ty },
                    BinOp::SmallerOrEqual   => Inst::CmpLe { dest: dest.clone(), lhs: lv, rhs: rv, ty },
                    BinOp::GreaterOrEqual   => Inst::CmpGe { dest: dest.clone(), lhs: lv, rhs: rv, ty },
                    _ => unreachable!(),
                };
                builder.emit(inst);
                return dest;
            }

            // `-`/`*`/`/` avec un opérande Ptr (mixed) : même dispatch
            // dynamique que `+` (voir `__dyn_add`/`__dyn_sub`/`__dyn_mul`/
            // `__dyn_div`, runtime/src/lib.rs) — décide RÉELLEMENT au runtime
            // (`is_float_box`) si le résultat est entier ou flottant, plutôt
            // que de se fier au type statique de l'AUTRE opérande : un `int`
            // connu combiné à un `mixed` qui contient en réalité un `float`
            // aurait sinon figé `ty` à I64, tronquant silencieusement le
            // float (confirmé par reproduction — voir
            // docs/roadmap.d/langage-mixed-arithmetic.md). Retourne, comme
            // `__dyn_add`, une valeur "mixed" auto-décrite (entier brut ou
            // float boxé) — `expr_ir_type` rapporte donc aussi `Ptr` pour ce
            // cas (voir typeinfer.rs), le consommateur (`box_for_any` pour
            // une affectation vers une cible concrète, ou cette même fonction
            // récursivement pour un opérateur englobant) la déballe si besoin.
            if matches!(op, BinOp::Sub | BinOp::Mul | BinOp::Div)
                && (matches!(left_ty, IrType::Ptr) || matches!(right_ty, IrType::Ptr))
            {
                let lv_raw = lower_expr(builder, left);
                let rv_raw = lower_expr(builder, right);
                let lv = box_for_dyn_arith(builder, &left_ty, lv_raw);
                let rv = box_for_dyn_arith(builder, &right_ty, rv_raw);
                let func = match op {
                    BinOp::Sub => "__dyn_sub",
                    BinOp::Mul => "__dyn_mul",
                    BinOp::Div => "__dyn_div",
                    _ => unreachable!(),
                };
                let dest = builder.new_value();
                builder.emit(Inst::Call { dest: Some(dest.clone()), func: func.into(), args: vec![lv, rv], ret_ty: IrType::Ptr });
                return dest;
            }

            // `%` avec un opérande Ptr : toujours entier, `Inst::Mod` n'a de
            // toute façon aucun support flottant (voir `emit_arithmetic` —
            // `srem` inconditionnel) ; un opérande `mixed` est simplement
            // déballé en entier avant l'opération, comme pour `-`/`*`/`/`
            // avant que ce correctif ne les rende pleinement dynamiques.
            if matches!(op, BinOp::Mod) && (matches!(left_ty, IrType::Ptr) || matches!(right_ty, IrType::Ptr)) {
                let lv_raw = lower_expr(builder, left);
                let rv_raw = lower_expr(builder, right);
                let lv = if matches!(left_ty, IrType::Ptr) {
                    unbox_mixed_operand(builder, "__mixed_to_int", &IrType::I64, lv_raw)
                } else {
                    lv_raw
                };
                let rv = if matches!(right_ty, IrType::Ptr) {
                    unbox_mixed_operand(builder, "__mixed_to_int", &IrType::I64, rv_raw)
                } else {
                    rv_raw
                };
                let dest = builder.new_value();
                builder.emit(Inst::Mod { dest: dest.clone(), lhs: lv, rhs: rv, ty: IrType::I64 });
                return dest;
            }

            // Chemin normal : aucun opérande Ptr, types statiquement connus
            // (int/float/bool) des deux côtés — inchangé.
            let ty = if matches!(left_ty, IrType::F64) || matches!(right_ty, IrType::F64) {
                IrType::F64
            } else {
                IrType::I64
            };
            let lv = lower_expr(builder, left);
            let rv = lower_expr(builder, right);
            let dest = builder.new_value();

            let inst = match op {
                BinOp::Add   => Inst::Add { dest: dest.clone(), lhs: lv, rhs: rv, ty },
                BinOp::Sub   => Inst::Sub { dest: dest.clone(), lhs: lv, rhs: rv, ty },
                BinOp::Mul   => Inst::Mul { dest: dest.clone(), lhs: lv, rhs: rv, ty },
                BinOp::Div   => Inst::Div { dest: dest.clone(), lhs: lv, rhs: rv, ty },
                BinOp::Mod   => Inst::Mod { dest: dest.clone(), lhs: lv, rhs: rv, ty },
                BinOp::And   => Inst::And { dest: dest.clone(), lhs: lv, rhs: rv },
                BinOp::Or    => Inst::Or  { dest: dest.clone(), lhs: lv, rhs: rv },
                _ => unreachable!("comparisons handled above"),
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
        // Pas de type de destination connu ici (nested/argument/retour...) —
        // voir `lower_array_literal`/`LiteralElemKind::Mixed` pour pourquoi
        // c'est le choix par défaut sûr.
        Expr::Array { elements, .. } => lower_array_literal(builder, elements, LiteralElemKind::Mixed),

        // ── Map littéral ──────────────────────────────────────────────────────
        Expr::Map { entries, .. } => lower_map_literal(builder, entries, LiteralElemKind::Mixed),

        // ── Accès par index ───────────────────────────────────────────────────
        Expr::Index { object, index, .. } => {
            let obj_val = lower_expr(builder, object);
            let idx_val = lower_expr(builder, index);
            let dest = builder.new_value();
            // Détermine si c'est un accès map ou array selon le type de la variable
            // — ou, pour `self.champ[clé]`/`obj.champ[clé]`, selon le type déclaré
            // du CHAMP (module.class_map_fields, seule source fiable : class_layouts
            // réduit tout champ à IrType::Ptr, map/array/string indistinguables).
            let is_map = match object.as_ref() {
                Expr::Ident(name, _) => builder.map_vars.contains(name.as_str()),
                Expr::Field { object: inner, field, .. } => {
                    let class_name = match inner.as_ref() {
                        Expr::Ident(name, _) => builder.var_class.get(name.as_str()).cloned(),
                        Expr::SelfExpr(_)    => builder.current_class.clone(),
                        Expr::Field { object: inner2, field: inner2_field, .. } => {
                            resolve_chained_field_class(builder, inner2, inner2_field)
                        }
                        _ => None,
                    };
                    class_name
                        .and_then(|cls| builder.module.class_map_fields.get(&cls).cloned())
                        .map(|fields| fields.contains(field.as_str()))
                        .unwrap_or(false)
                }
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

            // Analyser les captures. Fusionner locals + captured_vars : une closure
            // imbriquée dans une autre closure référence des variables que la closure
            // englobante a déjà capturées (vivent dans captured_vars, pas locals) —
            // sans ça collect_captures les ignore silencieusement (même bug que dans
            // lower_try pour try/on imbriqué dans un nameless, voir exceptions.rs).
            let param_names: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
            let mut capture_scope: std::collections::HashMap<String, (Value, IrType, bool)> = builder.locals.clone();
            for (name, (_env_val, _idx, ty)) in builder.captured_vars.iter() {
                capture_scope.entry(name.clone()).or_insert_with(|| (Value(0), ty.clone(), false));
            }
            let captures = collect_captures(body, &param_names, &capture_scope);
            
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
            let fn_param_names_clone = builder.fn_param_names.clone();
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
                // Variable locale stack → promouvoir au tas. Cellule VERROUILLÉE
                // (`__alloc_locked_cell`, pas `__alloc_obj`) : cette valeur sera
                // désormais lue/écrite depuis le scope extérieur ET la closure,
                // potentiellement sur des threads différents (Thread::run,
                // workers HTTPServer) — voir docs/roadmap.d/memoire-concurrence-threads.md.
                if let Some((slot, ty, mutable)) = builder.locals.get(cap_name.as_str()).cloned() {
                    let heap_ptr = builder.new_value();
                    builder.emit(Inst::Call {
                        dest:   Some(heap_ptr.clone()),
                        func:   "__alloc_locked_cell".into(),
                        args:   vec![],
                        ret_ty: IrType::Ptr,
                    });
                    // Copier la valeur courante (stack → cellule verrouillée) —
                    // encore mono-thread à ce stade, mais __locked_cell_set reste
                    // sûr et cohérent avec tous les accès futurs.
                    let cur_val = builder.new_value();
                    builder.emit(Inst::Load { dest: cur_val.clone(), ptr: slot, ty: ty.clone() });
                    builder.emit(Inst::Call {
                        dest:   None,
                        func:   "__locked_cell_set".into(),
                        args:   vec![heap_ptr.clone(), cur_val],
                        ret_ty: IrType::Void,
                    });
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
                &fn_param_names_clone,
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
            // Shortcut statique pour `is ClassName`/`is InterfaceName` : un
            // opérande dont le type STATIQUE est un primitif brut (I64/F64/
            // Bool, jamais transporté en `Ptr`) ne peut JAMAIS être un objet
            // — `is_object`/le check de `class_id` réel (voir
            // `lower_is_check`) n'a alors rien à vérifier, et surtout ne
            // DOIT PAS être exécuté : `__is_object` (donc `read_tag`)
            // déréférence sans le savoir n'importe quel entier brut assez
            // grand pour ressembler à un pointeur heap — SEGFAULT confirmé
            // par reproduction sur `var n:int = 1000000; n is Shape`, même
            // classe de bug que docs/roadmap.d/memoire-fiabilite-runtime-bas-niveau.md
            // (ici sur un `int` CONCRET, jamais boxé puisque jamais `mixed`).
            if matches!(ty, Type::Named(_) | Type::Qualified(_)) {
                let static_ty = expr_ir_type(builder, expr);
                if matches!(static_ty, IrType::I64 | IrType::F64 | IrType::Bool) {
                    let val = lower_expr(builder, expr);
                    let _ = val; // évaluer pour les effets de bord éventuels, résultat ignoré
                    let dest = builder.new_value();
                    builder.emit(Inst::ConstBool { dest: dest.clone(), value: false });
                    return dest;
                }
            }
            let val = lower_expr(builder, expr);
            lower_is_check(builder, &val, ty)
        }
    }
}

