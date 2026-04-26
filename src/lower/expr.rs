use std::collections::HashMap;
use crate::ast::*;
use crate::ir::inst::{Inst, Value};
use crate::ir::types::IrType;
use crate::lower::builder::LowerBuilder;

// ─────────────────────────────────────────────────────────────────────────────
// Helpers pour les champs de classes
// ─────────────────────────────────────────────────────────────────────────────

/// Calcule l'offset en bytes d'un champ dans une classe (8 bytes par champ).
fn field_offset(layouts: &HashMap<String, Vec<(String, IrType)>>, class: &str, field: &str) -> i32 {
    if let Some(fields) = layouts.get(class) {
        if let Some(idx) = fields.iter().position(|(f, _)| f == field) {
            return (idx as i32) * 8;
        }
    }
    0
}

/// Retourne le type IR d'un champ depuis le class_layout.
fn field_ir_type(layouts: &HashMap<String, Vec<(String, IrType)>>, class: &str, field: &str) -> IrType {
    if let Some(fields) = layouts.get(class) {
        if let Some((_, ty)) = fields.iter().find(|(f, _)| f == field) {
            return ty.clone();
        }
    }
    IrType::Ptr
}

// ─────────────────────────────────────────────────────────────────────────────
// Lowering d'expressions → Value HIR
// ─────────────────────────────────────────────────────────────────────────────

/// Retourne true si l'expression produit un tableau (OcaraArray*).
/// Utilisé dans les templates pour appeler __array_to_str au lieu de ptr_to_str.
fn is_array_expr(builder: &LowerBuilder, expr: &Expr) -> bool {
    match expr {
        Expr::Ident(name, _) => builder.elem_types.contains_key(name.as_str()),
        Expr::StaticCall { class, method, .. } => {
            matches!(
                format!("{}_{}", class, method).as_str(),
                "System_args"
                | "Array_sort"
                | "Array_reverse"
                | "Array_slice"
                | "Map_keys_to_array"
                | "Map_values_to_array"
            )
        }
        _ => false,
    }
}

/// Détermine le type IR d'une expression sans générer de code.
/// Utilisé pour le dispatch typé de `write` et la détection de concat string.
fn expr_ir_type(builder: &LowerBuilder, expr: &Expr) -> IrType {
    match expr {
        Expr::Literal(Literal::Int(_), _)    => IrType::I64,
        Expr::Literal(Literal::Float(_), _)  => IrType::F64,
        Expr::Literal(Literal::Bool(_), _)   => IrType::Bool,
        Expr::Literal(Literal::String(_), _) => IrType::Ptr,
        Expr::Literal(Literal::Null, _)      => IrType::Ptr,
        Expr::Ident(name, _) => {
            if let Some((_, ty, _)) = builder.locals.get(name.as_str()) {
                ty.clone()
            } else {
                IrType::Ptr
            }
        }
        // Appels de méthodes String_* et IO_read* retournent des strings
        Expr::StaticCall { class, method, .. } => {
            let fname = format!("{}_{}", class, method);
            // D'abord consulter fn_ret_types (classes locales et builtins enregistrés)
            if let Some(ty) = builder.fn_ret_types.get(&fname) {
                return ty.clone();
            }
            if fname.starts_with("String_")
                || fname.starts_with("IO_read")
                || fname == "__str_concat"
                || fname == "Array_join"
                || fname == "Array_reverse"
                || fname == "Array_slice"
                || fname == "Array_sort"
                || fname.starts_with("Convert_")
                || fname.starts_with("Map_keys")
                || fname.starts_with("Map_values")
                || fname == "System_cwd"
                || fname == "System_exec"
                || fname == "System_env"
                || fname == "HTTPRequest_body"
                || fname == "HTTPRequest_header"
                || fname == "HTTPRequest_error"
            {
                IrType::Ptr
            } else {
                IrType::I64
            }
        }
        // Opérations binaires : propager Ptr si c'est une concat string
        Expr::Binary { op, left, right, .. } => {
            // Comparaisons → Bool
            if matches!(op, BinOp::EqEq | BinOp::NotEq | BinOp::Lt | BinOp::LtEq | BinOp::Gt | BinOp::GtEq) {
                return IrType::Bool;
            }
            // Logiques && || → Bool
            if matches!(op, BinOp::And | BinOp::Or) {
                return IrType::Bool;
            }
            if matches!(op, BinOp::Add) {
                let lt = expr_ir_type(builder, left);
                let rt = expr_ir_type(builder, right);
                if matches!(lt, IrType::Ptr) || matches!(rt, IrType::Ptr) {
                    return IrType::Ptr;
                }
                // Float si un des deux est F64
                if matches!(lt, IrType::F64) || matches!(rt, IrType::F64) {
                    return IrType::F64;
                }
            }
            if matches!(op, BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod) {
                let lt = expr_ir_type(builder, left);
                let rt = expr_ir_type(builder, right);
                if matches!(lt, IrType::F64) || matches!(rt, IrType::F64) {
                    return IrType::F64;
                }
            }
            IrType::I64
        }
        // Les templates produisent toujours une string
        Expr::Template { .. } => IrType::Ptr,
        // Match : type déterminé par le premier bras
        Expr::Match { arms, .. } => {
            if let Some(arm) = arms.first() {
                expr_ir_type(builder, &arm.body)
            } else {
                IrType::I64
            }
        }
        // Accès tableau : utilise elem_types si disponible
        Expr::Index { object, .. } => {
            if let Expr::Ident(name, _) = object.as_ref() {
                if let Some(ty) = builder.elem_types.get(name.as_str()) {
                    return ty.clone();
                }
            }
            IrType::I64
        }
        // Accès champ : utilise class_layouts pour connaître le type
        Expr::Field { object, field, .. } => {
            let class_name = match object.as_ref() {
                Expr::Ident(name, _) => builder.var_class.get(name.as_str()).cloned(),
                Expr::SelfExpr(_)    => builder.current_class.clone(),
                _ => None,
            };
            if let Some(cls) = class_name {
                if let Some(fields) = builder.module.class_layouts.get(cls.as_str()) {
                    if let Some((_, ty)) = fields.iter().find(|(f, _)| f == field) {
                        return ty.clone();
                    }
                }
            }
            IrType::Ptr
        }
        // Exception : fonctions utilisateur dont on connaît le type de retour
        Expr::Call { callee, .. } => {
            // Callee = Ident (fonction libre)
            if let Expr::Ident(fname, _) = callee.as_ref() {
                if let Some(ty) = builder.fn_ret_types.get(fname.as_str()) {
                    return ty.clone();
                }
            }
            // Callee = méthode obj.method() ou self.method()
            if let Expr::Field { object, field, .. } = callee.as_ref() {
                let class_name = match object.as_ref() {
                    Expr::Ident(name, _) => builder.var_class.get(name.as_str()).cloned(),
                    Expr::SelfExpr(_)    => builder.current_class.clone(),
                    _ => None,
                };
                if let Some(cls) = class_name {
                    let mangled = format!("{}_{}", cls, field);
                    if let Some(ty) = builder.fn_ret_types.get(&mangled) {
                        return ty.clone();
                    }
                }
            }
            IrType::I64
        }
        Expr::StaticConst { class, name, .. } => {
            let key = format!("{}__{}", class, name);
            if let Some((ty, _)) = builder.module.class_consts.get(&key) {
                ty.clone()
            } else {
                IrType::Ptr
            }
        }
        Expr::Unary { op, operand, .. } => {
            match op {
                UnaryOp::Not => IrType::Bool,
                UnaryOp::Neg => expr_ir_type(builder, operand),
            }
        }
        _ => IrType::I64,
    }
}

/// Retourne le nom de la variante typée de `write` selon le type de l'argument.
/// "write"       → string/mixed (pas de conversion, write(ptr) direct)
/// "write_int"   → entiers
/// "write_float" → flottants (prend f64)
/// "write_bool"  → booléens
fn write_variant(base: &str, ty: &IrType) -> String {
    let suffix = match ty {
        IrType::F64  => "_float",
        IrType::Bool => "_bool",
        IrType::I64  => "_int",
        _            => "",   // Ptr / Mixed → write directement
    };
    format!("{}{}", base, suffix)
}

/// Version publique de expr_ir_type (utilisée par stmt.rs pour le boxing mixed)
pub fn expr_ir_type_pub(builder: &LowerBuilder, expr: &Expr) -> IrType {
    expr_ir_type(builder, expr)
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
            if let Expr::Ident(name, _) = callee.as_ref() {
                if name.starts_with("__") {
                    eprintln!("error: `{}` est une fonction interne du compilateur et ne peut pas être appelée directement", name);
                    std::process::exit(1);
                }
            }

            let func_name = match callee.as_ref() {
                Expr::Ident(name, _) => name.clone(),
                Expr::Field { object, field, .. } => {
                    // méthode → appel manglé ClassName_method
                    // On résout le nom de classe depuis var_class ou current_class (self)
                    let class_name = match object.as_ref() {
                        Expr::Ident(var_name, _) => {
                            builder.var_class.get(var_name.as_str()).cloned()
                        }
                        Expr::SelfExpr(_) => builder.current_class.clone(),
                        _ => None,
                    };
                    let func_mangled = if let Some(cls) = class_name {
                        format!("{}_{}", cls, field)
                    } else {
                        format!("_method_{}", field) // fallback (ne devrait pas arriver)
                    };
                    let obj_val = lower_expr(builder, object);
                    let dest = builder.new_value();
                    let arg_vals: Vec<Value> = args.iter().map(|a| lower_expr(builder, a)).collect();
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

            // Fonctions d'affichage : dispatch vers la variante typée
            const WRITE_FUNS: &[&str] = &["IO_write", "IO_writeln"];
            if WRITE_FUNS.contains(&func_name.as_str()) && args.len() == 1 {
                let arg_ty  = expr_ir_type(builder, &args[0]);
                let variant = write_variant(&func_name, &arg_ty);
                let arg_val = lower_expr(builder, &args[0]);
                let dest    = builder.new_value();
                builder.emit(Inst::Call {
                    dest:   Some(dest.clone()),
                    func:   variant,
                    args:   vec![arg_val],
                    ret_ty: IrType::Void,
                });
                return dest;
            }

            let arg_vals: Vec<Value> = args.iter().map(|a| lower_expr(builder, a)).collect();
            let dest = builder.new_value();
            builder.emit(Inst::Call {
                dest:   Some(dest.clone()),
                func:   func_name,
                args:   arg_vals,
                ret_ty: IrType::Ptr,
            });
            dest
        }

        // ── Accès statique ──────────────────────────────────────────────────
        Expr::StaticCall { class, method, args, .. } => {
            let func_name = format!("{}_{}", class, method);

            // Vérification de l'import : les modules ocara builtins doivent être importés
            // SAUF si la classe est définie localement dans le programme
            const BUILTIN_MODULES: &[&str] = &[
                "String", "Math", "Array", "Map", "IO",
                "Convert", "System", "Regex", "HTTPRequest",
            ];
            let is_local_class = builder.module.class_layouts.contains_key(class.as_str());
            if BUILTIN_MODULES.contains(&class.as_str()) && !is_local_class {
                let imported = builder.module.imports.iter().any(|m| m == class);
                if !imported {
                    eprintln!(
                        "error: utilisation de `{}::{}` sans `import ocara.{}`",
                        class, method, class
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
                let dest    = builder.new_value();
                builder.emit(Inst::Call {
                    dest:   Some(dest.clone()),
                    func:   variant,
                    args:   vec![arg_val],
                    ret_ty: IrType::Void,
                });
                return dest;
            }

            let arg_vals: Vec<Value> = args.iter().map(|a| lower_expr(builder, a)).collect();
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
            let key = format!("{}__{}", class, name);
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
                // Fallback : charge le global manglé comme pointeur
                let idx = builder.module.intern_string(&key);
                builder.emit(Inst::ConstStr { dest: dest.clone(), idx });
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
                    let pat_val = lower_literal(builder, pat);
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
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Littéral → Value (inline, sans slot)
// ─────────────────────────────────────────────────────────────────────────────

pub fn lower_literal(builder: &mut LowerBuilder, lit: &Literal) -> Value {
    match lit {
        Literal::Int(n) => {
            let dest = builder.new_value();
            builder.emit(Inst::ConstInt { dest: dest.clone(), value: *n });
            dest
        }
        Literal::Float(f) => {
            let dest = builder.new_value();
            builder.emit(Inst::ConstFloat { dest: dest.clone(), value: *f });
            dest
        }
        Literal::Bool(b) => {
            let dest = builder.new_value();
            builder.emit(Inst::ConstBool { dest: dest.clone(), value: *b });
            dest
        }
        Literal::String(s) => {
            let idx = builder.module.intern_string(s);
            let dest = builder.new_value();
            builder.emit(Inst::ConstStr { dest: dest.clone(), idx });
            dest
        }
        Literal::Null => {
            let dest = builder.new_value();
            builder.emit(Inst::ConstInt { dest: dest.clone(), value: 0 });
            dest
        }
    }
}
