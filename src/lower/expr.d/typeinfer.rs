/// Inférence de types pour les expressions

use crate::parsing::ast::*;
use crate::ir::types::IrType;
use crate::lower::builder::LowerBuilder;

/// Détermine le type IR d'une expression sans générer de code.
/// Utilisé pour le dispatch typé de `write` et la détection de concat string.
pub fn expr_ir_type(builder: &LowerBuilder, expr: &Expr) -> IrType {
    match expr {
        Expr::Literal(Literal::Int(_), _)    => IrType::I64,
        Expr::Literal(Literal::Float(_), _)  => IrType::F64,
        Expr::Literal(Literal::Bool(_), _)   => IrType::Bool,
        Expr::Literal(Literal::String(_), _) => IrType::Ptr,
        Expr::Literal(Literal::Null, _)      => IrType::Ptr,
        Expr::Ident(name, _) => {
            // Variable d'un générateur (voir `crate::lower::builder::message_gen`) :
            // vérifiée EN PREMIER, sinon son vrai type (ex: I64 pour `i`)
            // resterait invisible ici (elle n'est jamais dans `locals`) et
            // retomberait à tort sur `Ptr` — confirmé faux par reproduction
            // (`i smaller 3` dans un `while` imbriqué dans un générateur :
            // `i` traité comme `Ptr` faisait comparer un entier brut à un
            // pointeur "mixed" boxé, via `__cmp_lt_strict`).
            if let Some((_, _, ty)) = builder.frame_vars.get(name.as_str()) {
                ty.clone()
            } else if let Some((_, ty, _)) = builder.locals.get(name.as_str()) {
                ty.clone()
            } else if let Some((_, _, ty)) = builder.captured_vars.get(name.as_str()) {
                ty.clone()
            } else {
                IrType::Ptr
            }
        }
        // Appels de méthodes String_* et IO_read* retournent des strings
        Expr::StaticCall { class, method, args, .. } => {
            // Consommation scalaire directe d'un `message<T>` — voir la
            // même note dans le bras `Expr::Call` ci-dessus.
            if let Some((_, elem_ty)) = crate::lower::builder::message_gen::detect_message_call(builder, expr) {
                return elem_ty;
            }
            // Résoudre "<parent>" et "<self>" vers les classes appropriées
            let resolved_class = if class == "<parent>" {
                builder.parent_class.as_deref().unwrap_or(class.as_str())
            } else if class == "<self>" {
                builder.current_class.as_deref().unwrap_or(class.as_str())
            } else {
                class.as_str()
            };
            let fname = format!("{}_{}", resolved_class, method);
            // `Array::get`/`first`/`last`/`pop` (et `Map::get`) sur un
            // conteneur à élément CONCRET (`int`/`float`/`bool`) :
            // `fn_ret_types` dit toujours `Ptr` (correct seulement pour un
            // conteneur `mixed`, dont les éléments sont déjà boxés) —
            // consulter `elem_types` de l'objet en premier argument, comme
            // le fait déjà `Expr::Index` pour un accès direct (`arr[i]`),
            // AVANT de faire confiance à ce `Ptr` générique. Confirmé faux
            // par reproduction : `IO::writeln(Array::get(arr, i))` sur un
            // `array<int>` affichait "null" pour l'élément valant `0`
            // (interprété comme un pointeur nul) — voir
            // docs/roadmap.d/langage-array-get-display-bug.md.
            if (resolved_class == "Array" && matches!(method.as_str(), "get" | "first" | "last" | "pop"))
                || (resolved_class == "Map" && method == "get")
            {
                if let Some(Expr::Ident(obj_name, _)) = args.first() {
                    if let Some(ty) = builder.elem_types.get(obj_name.as_str()) {
                        return ty.clone();
                    }
                }
            }
            // D'abord consulter fn_ret_types (classes locales et builtins enregistrés)
            if let Some(ty) = builder.fn_ret_types.get(&fname) {
                return ty.clone();
            }
            if fname.starts_with("String_")
                || fname == "__str_concat"
                || fname == "Array_join"
                || fname == "Array_reverse"
                || fname == "Array_slice"
                || fname == "Array_sort"
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
                // Filet de sécurité : ni `fn_ret_types` ni la liste ci-dessus
                // ne connaissent ce nom (souvent un constructeur/factory
                // builtin oublié de `fn_ret_types`, ex. `SQLite::open`,
                // absent avant ce correctif — voir program.rs). Retomber sur
                // `Ptr` (jamais sur `I64`) est le choix sûr : un consommateur
                // qui traite ensuite cette valeur comme `mixed`
                // (`box_for_any`/`box_for_dyn_arith`) ne la boxera PAS s'il la
                // croit déjà `Ptr` — inoffensif pour un vrai pointeur objet
                // (le cas confirmé par reproduction : `SQLite::open` mal
                // classé en I64 faisait boxer le pointeur de connexion
                // lui-même, corrompant `self` et bloquant `db.execute()` dans
                // une boucle infinie). Un `I64` par défaut aurait l'effet
                // inverse ET dangereux : n'importe quel pointeur objet
                // provenant d'un appel non répertorié ici serait boxé comme
                // un entier. Contrepartie acceptée : un builtin qui retourne
                // réellement un `int` et n'est PAS répertorié ici ne profite
                // simplement pas du boxing anti-SEGFAULT pour un `mixed` —
                // identique au comportement d'avant ce chantier, pas une
                // régression.
                IrType::Ptr
            }
        }
        // Opérations binaires : propager Ptr si c'est une concat string
        Expr::Binary { op, left, right, .. } => {
            // Comparaisons → Bool
            if matches!(op, BinOp::Equal | BinOp::NotEqual |
                        BinOp::Smaller | BinOp::Greater | BinOp::SmallerOrEqual | BinOp::GreaterOrEqual) {
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
            // `-`/`*`/`/` : même règle que `+` ci-dessus — propager `Ptr` si
            // un opérande est `mixed` (voir `__dyn_sub`/`__dyn_mul`/`__dyn_div`
            // dans lower::expr::lower, qui décident dynamiquement entier vs
            // flottant plutôt que de figer ça au type statique de l'AUTRE
            // opérande). `%` reste toujours I64/F64 selon le type statique
            // connu : `Inst::Mod` n'a de toute façon aucun support flottant
            // (voir emit_arithmetic), un opérande `mixed` y est simplement
            // déballé en entier.
            if matches!(op, BinOp::Sub | BinOp::Mul | BinOp::Div) {
                let lt = expr_ir_type(builder, left);
                let rt = expr_ir_type(builder, right);
                if matches!(lt, IrType::Ptr) || matches!(rt, IrType::Ptr) {
                    return IrType::Ptr;
                }
                if matches!(lt, IrType::F64) || matches!(rt, IrType::F64) {
                    return IrType::F64;
                }
            }
            if matches!(op, BinOp::Mod) {
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
            // Filet de sécurité : même raison que pour `Expr::StaticCall`/
            // `Expr::Call` ci-dessus. `elem_types` peut manquer une entrée
            // (ex. un paramètre `map<string,mixed>` d'une closure `nameless`,
            // pas enregistré par le même chemin qu'un paramètre de fonction
            // top-level) — confirmé faux par reproduction : `attrs["title"]`
            // (une vraie string) classée I64 se faisait boxer comme un
            // entier par `box_for_any`, puis affichait l'ADRESSE du pointeur
            // string comme un nombre une fois déballée (voir
            // examples/advanced/httpserver/configs/components/Layout.oc).
            IrType::Ptr
        }
        // Accès champ : utilise class_layouts pour connaître le type
        Expr::Field { object, field, .. } => {
            let class_name = match object.as_ref() {
                Expr::Ident(name, _) => builder.var_class.get(name.as_str()).cloned(),
                Expr::SelfExpr(_)    => builder.current_class.clone(),
                // Accès chaîné (`a.b.c`) — voir resolve_chained_field_class.
                Expr::Field { object: inner, field: inner_field, .. } => {
                    super::helpers::resolve_chained_field_class(builder, inner, inner_field)
                }
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
        Expr::Call { .. } => {
            // Consommation scalaire directe d'un `message<T>` (générateur —
            // voir docs/roadmap.d/langage-emit-iterable.md) : le type réel
            // est celui de `T`, pas `IrType::Ptr` (ce que donnerait
            // `fn_ret_types`, qui réduit `message<T>` à `Ptr` comme tout
            // pointeur de frame — voir `IrType::from_ast`).
            if let Some((_, elem_ty)) = crate::lower::builder::message_gen::detect_message_call(builder, expr) {
                return elem_ty;
            }
            let Expr::Call { callee, .. } = expr else { unreachable!() };
            // Appel indirect : variable de type Function<ReturnType>
            if let Expr::Ident(fname, _) = callee.as_ref() {
                if let Some(ty) = builder.func_ret_types.get(fname.as_str()) {
                    return ty.clone();
                }
            }
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
                    Expr::Literal(Literal::String(_), _) => Some("String".to_string()),
                    // Appel chaîné : obj.method1().method2()
                    Expr::Call { callee: inner_callee, .. } => {
                        if let Expr::Field { object: inner_obj, field: inner_method, .. } = inner_callee.as_ref() {
                            // Essayer de trouver la classe de l'objet interne
                            if let Expr::Ident(name, _) = inner_obj.as_ref() {
                                if let Some(cls) = builder.var_class.get(name.as_str()).cloned() {
                                    let method_name = format!("{}_{}", cls, inner_method);
                                    // Si la méthode retourne un Ptr, continuer avec la même classe
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
                        } else {
                            None
                        }
                    }
                    // Accès chaîné : w.inner.methode() où `inner` est
                    // elle-même une instance de classe — voir
                    // resolve_chained_field_class.
                    Expr::Field { object: inner_obj, field: inner_field, .. } => {
                        super::helpers::resolve_chained_field_class(builder, inner_obj, inner_field)
                    }
                    _ => None,
                };
                // `arr.get(i)`/`m.get(k)` (sucre d'instance) — même bug et
                // même correctif que `Array::get(arr, i)`/`Map::get(m, k)`
                // (voir la même note dans le bras `Expr::StaticCall`
                // ci-dessus) : `fn_ret_types` dit toujours `Ptr`, faux pour
                // un conteneur à élément concret. Objet = `object` lui-même
                // ici (le récepteur), pas `args[0]` comme pour la forme
                // statique.
                if let Some(cls) = &class_name {
                    let is_concrete_get = (cls == "Array" && matches!(field.as_str(), "get" | "first" | "last" | "pop"))
                        || (cls == "Map" && field == "get");
                    if is_concrete_get {
                        if let Expr::Ident(obj_name, _) = object.as_ref() {
                            if let Some(ty) = builder.elem_types.get(obj_name.as_str()) {
                                return ty.clone();
                            }
                        }
                    }
                }
                if let Some(cls) = class_name {
                    let mangled = format!("{}_{}", cls, field);
                    if let Some(ty) = builder.fn_ret_types.get(&mangled) {
                        return ty.clone();
                    }
                }
            }
            // Filet de sécurité : même raison que pour `Expr::StaticCall`
            // ci-dessus — `Ptr` (jamais `I64`) est le choix sûr quand la
            // classe/méthode réelle n'a pas pu être résolue (`var_class` ne
            // connaît pas le récepteur, ex. objet dans un `mixed`, ou
            // méthode absente de `fn_ret_types`) : un vrai pointeur objet/
            // string retourné ici et classé par erreur `I64` serait boxé
            // comme un entier par `box_for_any` (corruption confirmée par
            // reproduction : `JSON::encode`/`obj.encode()` d'instance
            // affichait l'adresse du pointeur au lieu du JSON).
            IrType::Ptr
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
        Expr::IsCheck { .. } => IrType::Bool,
        Expr::Resolve { expr, .. } => {
            // Retourne le type IR original de la fonction async sous-jacente.
            match expr.as_ref() {
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
            }
        }
        Expr::Nameless { .. } => IrType::Ptr,
        // Littéraux `array`/`map`, instanciation, `self`/`parent` : toujours
        // des pointeurs tas — un oubli ici retombait sur le `_ => IrType::I64`
        // final, resté inoffensif tant que rien n'agissait différemment
        // selon I64 vs Ptr ; devenu dangereux depuis que `box_for_any`/
        // `box_for_dyn_arith` boxent RÉELLEMENT un I64 assez grand pour un
        // `mixed` (voir `box_int_if_needed`) — un pointeur tas (toujours
        // "assez grand") aurait alors été boxé comme si c'était un entier,
        // corrompant la valeur (confirmé par reproduction : `var a:array<int>
        // = [1,2,3]` cassait déjà `Array::len(a)` avant ce correctif).
        Expr::Array { .. } | Expr::Map { .. } | Expr::New { .. }
        | Expr::SelfExpr(_) | Expr::ParentExpr(_) => IrType::Ptr,
        _ => IrType::I64,
    }
}

/// Version publique de expr_ir_type (utilisée par stmt.rs pour le boxing mixed)
pub fn expr_ir_type_pub(builder: &LowerBuilder, expr: &Expr) -> IrType {
    expr_ir_type(builder, expr)
}
