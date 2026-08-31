/// Lowering spécial de `ui.handler(name, ClassName::method)` (et, par extension,
/// de chaque entrée de `ui.handlers({...})` — voir lower.rs, elles sont désucrées
/// vers des appels individuels à ce même mécanisme).
///
/// Le compilateur connaît statiquement la signature réelle de la méthode ciblée
/// (via fn_param_types/fn_param_names/fn_ret_types, déjà peuplés pour toute
/// fonction/méthode déclarée — voir program.rs). Il génère donc, pour CE couple
/// (nom, méthode) précis, un trampoline `extern "C" fn(json_args: Ptr) -> Ptr` qui :
///   1. vérifie que l'objet JSON reçu contient bien une clé par nom de paramètre,
///   2. vérifie que chaque valeur a le type attendu pour ce paramètre,
///   3. si tout est valide, décode chaque valeur puis appelle la vraie méthode,
///   4. encode son retour dans une enveloppe `{"ok":true,"value":...}`,
///   5. sinon, renvoie `{"ok":false,"error":"..."}` sans jamais appeler la méthode.
///
/// Convention JS : `invoke("cmd", {a: v1, b: v2})`, matché par NOM de paramètre
/// Ocara — c'est la seule forme que le pont IPC natif de Tauri délivre fidèlement
/// (voir la note dans runtime/src/tauri.rs : un payload *array* est intercepté par
/// le JS interne de Tauri et traité comme du binaire brut avant même d'atteindre
/// le native, donc jamais reçu comme JSON ici). Pour préserver malgré tout la
/// syntaxe `invoke("cmd", [v1, v2])` côté JS utilisateur, runtime/src/tauri.rs
/// injecte un shim JS (au chargement de la page) qui réécrit un payload array en
/// objet nommé — grâce à param_names, transmis à Tauri_handler_register — avant
/// que Tauri lui-même ne le voie.
///
/// Types de paramètres/retour supportés : int, float, bool, string.
use crate::parsing::ast::Expr;
use crate::ir::inst::{Inst, Value};
use crate::ir::func::IrParam;
use crate::ir::types::IrType;
use crate::lower::builder::LowerBuilder;
use crate::lower::expr::lower_expr;

/// Types Ocara qu'un handler JS peut actuellement accepter/retourner.
fn is_supported_handler_type(ty: &IrType) -> bool {
    matches!(ty, IrType::I64 | IrType::F64 | IrType::Bool | IrType::Ptr)
}

/// Tente de lowerer `object.field(args)` comme un `ui.handler(name, Class::method)`.
/// Retourne `Some(dest)` si c'est bien ce cas (dest = valeur factice, l'appel est
/// void côté Ocara) ; `None` sinon (le caller retombe sur le dispatch générique).
pub fn try_lower_tauri_handler_call(
    builder:    &mut LowerBuilder,
    class_name: &Option<String>,
    field:      &str,
    object:     &Expr,
    args:       &[Expr],
) -> Option<Value> {
    if field != "handler" || class_name.as_deref() != Some("Tauri") {
        return None;
    }
    if args.len() != 2 {
        eprintln!("error: ui.handler(name, methode) attend exactement 2 arguments");
        std::process::exit(1);
    }

    let (target_class, target_method) = match &args[1] {
        Expr::StaticConst { class, name, .. } => {
            let resolved = if class == "<self>" {
                builder.current_class.clone().unwrap_or_default()
            } else if class == "<parent>" {
                builder.parent_class.clone().unwrap_or_default()
            } else {
                class.clone()
            };
            (resolved, name.clone())
        }
        _ => {
            eprintln!(
                "error: ui.handler() attend une référence de méthode statique en second \
                 argument (ex: MaClasse::maMethode), pas un appel ni une expression calculée"
            );
            std::process::exit(1);
        }
    };

    let method_key = format!("{}_{}", target_class, target_method);
    let param_types = builder.fn_param_types.get(&method_key).cloned().unwrap_or_default();
    let param_names = builder.fn_param_names.get(&method_key).cloned().unwrap_or_default();
    let ret_ty = builder.fn_ret_types.get(&method_key).cloned().unwrap_or(IrType::Ptr);

    if param_names.len() != param_types.len() {
        eprintln!(
            "error: ui.handler(\"{}\"): impossible de résoudre les noms de paramètres de {} \
             (méthode introuvable ou non statique ?)",
            target_method, method_key
        );
        std::process::exit(1);
    }

    for ty in &param_types {
        if !is_supported_handler_type(ty) {
            eprintln!(
                "error: ui.handler(\"{}\"): un handler JS ne peut avoir que des paramètres \
                 int/float/bool/string (Array/Map pas encore supportés)",
                target_method
            );
            std::process::exit(1);
        }
    }
    if !matches!(ret_ty, IrType::Void) && !is_supported_handler_type(&ret_ty) {
        eprintln!(
            "error: ui.handler(\"{}\"): un handler JS ne peut retourner que \
             void/int/float/bool/string (Array/Map pas encore supportés)",
            target_method
        );
        std::process::exit(1);
    }

    // ── Génère le trampoline dans une passe isolée (reborrow de builder.module) ──
    let trampoline_id = builder.module.functions.len();
    let trampoline_name = format!("__tauri_handler_{}", trampoline_id);
    let trampoline_fn = generate_trampoline(&mut builder.module, &trampoline_name, &method_key, &param_types, &param_names, &ret_ty);
    builder.module.add_function(trampoline_fn);

    // ── Site d'appel : ui.handler("name", Class::method) → enregistrement ──
    // On transmet aussi la liste ordonnée des noms de paramètres (JSON, ex: ["a","b","c"])
    // pour que le shim JS injecté côté Tauri sache convertir un payload array en objet nommé.
    let param_names_json = serde_json_like_string_array(&param_names);
    let name_val = lower_expr(builder, &args[0]);
    let obj_val  = lower_expr(builder, object);
    let func_addr = builder.new_value();
    builder.emit(Inst::FuncAddr { dest: func_addr.clone(), func: trampoline_name });
    let names_idx = builder.module.intern_string(&param_names_json);
    let names_val = builder.new_value();
    builder.emit(Inst::ConstStr { dest: names_val.clone(), idx: names_idx });
    builder.emit(Inst::Call {
        dest: None,
        func: "Tauri_handler_register".into(),
        args: vec![obj_val, name_val, func_addr, names_val],
        ret_ty: IrType::Void,
    });

    let dummy = builder.new_value();
    builder.emit(Inst::ConstInt { dest: dummy.clone(), value: 0 });
    Some(dummy)
}

/// Tente de lowerer `object.field(args)` comme un `ui.handlers({"nom": Class::methode, ...})`.
/// Désucre chaque entrée du littéral map vers un appel individuel à
/// `try_lower_tauri_handler_call` (même mécanisme, même registre anti-doublon
/// que ui.handler — voir Tauri_handler_register côté runtime).
pub fn try_lower_tauri_handlers_call(
    builder:    &mut LowerBuilder,
    class_name: &Option<String>,
    field:      &str,
    object:     &Expr,
    args:       &[Expr],
) -> Option<Value> {
    if field != "handlers" || class_name.as_deref() != Some("Tauri") {
        return None;
    }
    if args.len() != 1 {
        eprintln!("error: ui.handlers(map) attend exactement 1 argument (un littéral map)");
        std::process::exit(1);
    }
    let entries = match &args[0] {
        Expr::Map { entries, .. } => entries,
        _ => {
            eprintln!(
                "error: ui.handlers() attend un littéral map {{\"nom\": Classe::methode, ...}}, \
                 pas une expression calculée"
            );
            std::process::exit(1);
        }
    };

    let mut last_dest: Option<Value> = None;
    for (key_expr, value_expr) in entries {
        let synthetic_args = [key_expr.clone(), value_expr.clone()];
        last_dest = try_lower_tauri_handler_call(builder, class_name, "handler", object, &synthetic_args);
    }

    // Toujours retourner Some(...) même si le map est vide : on a bien traité ce
    // cas spécial (ui.handlers), il ne faut pas retomber sur le dispatch générique.
    Some(last_dest.unwrap_or_else(|| {
        let dummy = builder.new_value();
        builder.emit(Inst::ConstInt { dest: dummy.clone(), value: 0 });
        dummy
    }))
}

/// Encode une liste de noms de paramètres en littéral JSON `["a","b","c"]`
/// (les noms de paramètres Ocara sont des identifiants simples : pas d'échappement à gérer).
fn serde_json_like_string_array(names: &[String]) -> String {
    let quoted: Vec<String> = names.iter().map(|n| format!("\"{}\"", n)).collect();
    format!("[{}]", quoted.join(","))
}

fn is_getter_fn(ty: &IrType) -> (&'static str, &'static str) {
    match ty {
        IrType::I64  => ("__tauri_obj_is_int",    "__tauri_obj_get_int"),
        IrType::F64  => ("__tauri_obj_is_float",  "__tauri_obj_get_float"),
        IrType::Bool => ("__tauri_obj_is_bool",   "__tauri_obj_get_bool"),
        _            => ("__tauri_obj_is_string", "__tauri_obj_get_string"),
    }
}

fn ok_fn_for(ty: &IrType) -> &'static str {
    match ty {
        IrType::I64  => "__tauri_ok_int",
        IrType::F64  => "__tauri_ok_float",
        IrType::Bool => "__tauri_ok_bool",
        _            => "__tauri_ok_string",
    }
}

fn emit_error_return(tb: &mut LowerBuilder, msg: &str) {
    let idx = tb.module.intern_string(msg);
    let msg_val = tb.new_value();
    tb.emit(Inst::ConstStr { dest: msg_val.clone(), idx });
    let err_val = tb.new_value();
    tb.emit(Inst::Call { dest: Some(err_val.clone()), func: "__tauri_err".into(), args: vec![msg_val], ret_ty: IrType::Ptr });
    tb.emit(Inst::Return { value: Some(err_val) });
}

/// Construit la fonction IR du trampoline : `extern "C" fn(json: Ptr) -> Ptr`.
/// `json` est un OBJET JSON (`{"a":..,"b":..}`), les arguments sont retrouvés par
/// nom de paramètre (pas par position — voir note en tête de fichier).
fn generate_trampoline(
    module:         &mut crate::ir::module::IrModule,
    trampoline_name: &str,
    target_fn:      &str,
    param_types:    &[IrType],
    param_names:    &[String],
    ret_ty:         &IrType,
) -> crate::ir::func::IrFunction {
    let ir_params = vec![IrParam { name: "__json".into(), ty: IrType::Ptr, slot: Value(0) }];
    let mut tb = LowerBuilder::new(module, trampoline_name.to_string(), ir_params, IrType::Ptr);

    let json_slot = tb.declare_local("__json", IrType::Ptr, false);
    let json_recv = tb.new_value();
    tb.emit(Inst::Store { ptr: json_slot, src: json_recv.clone() });
    tb.func.params = vec![IrParam { name: "__json".into(), ty: IrType::Ptr, slot: json_recv.clone() }];

    // 1. Vérifier que chaque clé attendue est présente ET du bon type (ET logique)
    let mut type_ok = tb.new_value();
    tb.emit(Inst::ConstInt { dest: type_ok.clone(), value: 1 });
    for (name, ty) in param_names.iter().zip(param_types.iter()) {
        let key_idx = tb.module.intern_string(name);
        let key_val = tb.new_value();
        tb.emit(Inst::ConstStr { dest: key_val.clone(), idx: key_idx });
        let (is_fn, _) = is_getter_fn(ty);
        let check = tb.new_value();
        tb.emit(Inst::Call { dest: Some(check.clone()), func: is_fn.into(), args: vec![json_recv.clone(), key_val], ret_ty: IrType::I64 });
        let new_acc = tb.new_value();
        tb.emit(Inst::And { dest: new_acc.clone(), lhs: type_ok.clone(), rhs: check });
        type_ok = new_acc;
    }
    let zero = tb.new_value();
    tb.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
    let types_ok_flag = tb.new_value();
    tb.emit(Inst::CmpNe { dest: types_ok_flag.clone(), lhs: type_ok, rhs: zero, ty: IrType::I64 });

    let call_bb = tb.new_block();
    let type_err_bb = tb.new_block();
    tb.emit(Inst::Branch { cond: types_ok_flag, then_bb: call_bb.clone(), else_bb: type_err_bb.clone() });

    tb.switch_to(&type_err_bb);
    let expected: Vec<String> = param_names.iter().zip(param_types.iter())
        .map(|(n, t)| format!("{}:{}", n, ir_type_label(t)))
        .collect();
    emit_error_return(&mut tb, &format!(
        "{} attend {{{}}} (argument manquant ou mal typé)",
        target_fn, expected.join(", ")
    ));

    tb.switch_to(&call_bb);

    // 2. Décoder chaque argument (par nom) puis appeler la vraie méthode Ocara
    let mut call_args: Vec<Value> = Vec::new();
    for (name, ty) in param_names.iter().zip(param_types.iter()) {
        let key_idx = tb.module.intern_string(name);
        let key_val = tb.new_value();
        tb.emit(Inst::ConstStr { dest: key_val.clone(), idx: key_idx });
        let (_, get_fn) = is_getter_fn(ty);
        let val = tb.new_value();
        tb.emit(Inst::Call { dest: Some(val.clone()), func: get_fn.into(), args: vec![json_recv.clone(), key_val], ret_ty: ty.clone() });
        call_args.push(val);
    }

    // 3. Appel réel + encodage du retour
    if matches!(ret_ty, IrType::Void) {
        tb.emit(Inst::Call { dest: None, func: target_fn.to_string(), args: call_args, ret_ty: IrType::Void });
        let encoded = tb.new_value();
        tb.emit(Inst::Call { dest: Some(encoded.clone()), func: "__tauri_ok_void".into(), args: vec![], ret_ty: IrType::Ptr });
        tb.emit(Inst::Return { value: Some(encoded) });
    } else {
        let result = tb.new_value();
        tb.emit(Inst::Call { dest: Some(result.clone()), func: target_fn.to_string(), args: call_args, ret_ty: ret_ty.clone() });
        let encoded = tb.new_value();
        tb.emit(Inst::Call { dest: Some(encoded.clone()), func: ok_fn_for(ret_ty).into(), args: vec![result], ret_ty: IrType::Ptr });
        tb.emit(Inst::Return { value: Some(encoded) });
    }

    tb.func
}

fn ir_type_label(ty: &IrType) -> &'static str {
    match ty {
        IrType::I64  => "int",
        IrType::F64  => "float",
        IrType::Bool => "bool",
        _            => "string",
    }
}
