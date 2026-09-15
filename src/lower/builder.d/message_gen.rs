/// Lowering de `emit`/`message<T>` (générateurs) — transformation en machine
/// à états à la compilation, voir docs/roadmap.d/langage-emit-iterable.md.
///
/// Une fonction/méthode `emit`-contenante ne produit PAS une seule
/// `IrFunction` mais DEUX :
///   - `<nom>__new(params...) -> Ptr`    alloue le frame heap et y stocke
///     les arguments, état initial 0.
///   - `<nom>__resume(frame:Ptr) -> Bool` la vraie machine à états : lit
///     l'état sauvegardé, saute au bon endroit, exécute jusqu'au prochain
///     `emit` (stocke la valeur dans le frame, avance l'état, retourne
///     `true`) ou jusqu'à épuisement (`false`).
///
/// Le frame (`__gen_<nom>`, alloué via `Inst::Alloc` — préfixe `__` ⇒
/// allocation SANS tag, voir `codegen/emit.d/instructions.d/memory.rs`) est
/// une "classe" synthétique dont TOUS les champs sont à plat (8 octets
/// chacun, `field_offset`) : `__state`, `__value`, puis un champ par
/// paramètre et par variable locale déclarée n'importe où dans le corps
/// (dédupliqué par nom — même simplification que `LowerBuilder::locals`,
/// déjà non scopé aujourd'hui pour les fonctions normales).
///
/// Pourquoi TOUS les locaux, pas seulement ceux qui survivent à un `emit` :
/// un `emit` fait un vrai `Return` natif (voir `Stmt::Emit` dans
/// `crate::lower::stmt::statements`), qui déroule complètement la pile —
/// rien de stack-résident (Alloca) ne survivrait à une reprise. Promouvoir
/// tout, sans analyse de vivacité, est une simplification volontaire pour
/// cette première implémentation (voir la fiche roadmap).
use std::collections::HashMap;
use crate::parsing::ast::*;
use crate::ir::func::IrParam;
use crate::ir::inst::{BlockId, Inst, Value};
use crate::ir::module::IrModule;
use crate::ir::types::IrType;
use crate::lower::builder::LowerBuilder;

pub fn is_message_func(ret_ty: &Type) -> bool {
    matches!(ret_ty, Type::Message(_))
}

/// Un `try` actuellement ouvert pendant le lowering du corps d'un
/// générateur (voir `lower_try_in_generator`, §4 "Cas A" de la fiche
/// roadmap) — une entrée par niveau d'imbrication, poussée à l'entrée du
/// `try`, dépilée à la sortie. Cloné dans chaque entrée de
/// `LowerBuilder::message_resume_blocks` au moment d'un `emit` : nécessaire
/// pour rejouer, à la prochaine reprise, exactement les `try` encore actifs
/// à ce point de suspension (`__ocara_try_enter` + `setjmp` + branchement),
/// AVANT de sauter au bloc de reprise réel.
#[derive(Clone)]
pub struct GenTryCtx {
    /// Champ frame portant le pointeur `TryFrame` de ce niveau (voir
    /// `try_frame_field`) — relu par le handler PARTAGÉ (une seule copie
    /// lowered, référencée par chaque rejeu) pour retrouver `error_val`/
    /// `error_type`, jamais une valeur SSA brute (invalide d'un rejeu à
    /// l'autre).
    pub frame_field: String,
    /// Bloc du handler INLINE (déjà lowered UNE SEULE FOIS) — la première
    /// exécution ET chaque rejeu y sautent tous en cas de `setjmp` non nul.
    pub handler_bb: BlockId,
}

/// Nom du champ réservé qui porte l'état courant du générateur (I64).
pub const STATE_FIELD: &str = "__state";
/// Nom du champ réservé qui porte la dernière valeur émise (type `T`).
pub const VALUE_FIELD: &str = "__value";
/// État sentinel : générateur épuisé, plus jamais aucune valeur.
pub const STATE_DONE: i64 = -1;

/// Nom de la classe frame synthétique d'un générateur donné (préfixe `__`
/// ⇒ allocation/libération sans tag, voir `memory.rs`/`__free_obj`).
pub fn frame_class_name(mangled_func_name: &str) -> String {
    format!("__gen_{}", mangled_func_name)
}

pub fn new_func_name(mangled_func_name: &str) -> String {
    format!("{}__new", mangled_func_name)
}

pub fn resume_func_name(mangled_func_name: &str) -> String {
    format!("{}__resume", mangled_func_name)
}

/// Calcule le layout du frame (`__state`, `__value`, params, locales
/// dédupliquées) et peuple `module.class_layouts`/`module.message_funcs` —
/// SANS générer le moindre corps de fonction. Doit tourner pour TOUTES les
/// fonctions/méthodes `emit`-contenantes AVANT le lowering du moindre corps
/// (voir `register_all_message_funcs`, appelée très tôt dans
/// `lower_program`, même contrainte d'ordre que
/// `class_dispatch::compute_classes_with_subclasses`) : un site de
/// consommation (`for`, scalaire directe) peut se trouver dans une fonction
/// lowered AVANT celle qui déclare le générateur (ex: `main` avant une
/// méthode de classe) — sans ce pré-passage, `detect_message_call` ne
/// trouverait rien pour un tel appel (confirmé par reproduction : un
/// générateur MÉTHODE consommé depuis `main`, lowered en premier dans
/// `lower_program`, silencieusement traité comme un appel normal).
pub fn register_message_func(module: &mut IrModule, func: &FuncDecl) {
    let Type::Message(elem_ast_ty) = &func.ret_ty else { return };
    let elem_ty = IrType::from_ast(elem_ast_ty);
    let frame_class = frame_class_name(&func.name);

    let mut fields: Vec<(String, IrType)> = vec![
        (STATE_FIELD.to_string(), IrType::I64),
        (VALUE_FIELD.to_string(), elem_ty.clone()),
    ];
    for p in &func.params {
        if !fields.iter().any(|(n, _)| n == &p.name) {
            fields.push((p.name.clone(), IrType::from_ast(&p.ty)));
        }
    }
    for (name, ty) in collect_local_decls(&func.body) {
        if !fields.iter().any(|(n, _)| n == &name) {
            fields.push((name, ty));
        }
    }
    module.class_layouts.insert(frame_class, fields);
    module.message_funcs.insert(func.name.clone(), elem_ty);
}

/// Pré-passage : enregistre TOUTES les fonctions libres et méthodes
/// `emit`-contenantes du programme (voir la doc de `register_message_func`).
/// À appeler AVANT le lowering de tout corps de fonction/méthode —
/// `super::program::lower_program`, aux côtés de
/// `class_dispatch::compute_classes_with_subclasses`.
pub fn register_all_message_funcs(module: &mut IrModule, program: &Program) {
    for func in &program.functions {
        register_message_func(module, func);
    }
    for class in &program.classes {
        for member in &class.members {
            let ClassMember::Method { decl, is_static, .. } = member else { continue };
            let mangled_name = format!("{}_{}", class.name, decl.name);
            let mangled = if *is_static {
                FuncDecl { name: mangled_name, ..decl.clone() }
            } else {
                let self_param = crate::parsing::ast::Param {
                    name: "self".into(),
                    ty:   Type::Mixed,
                    is_variadic: false,
                    default_value: None,
                    span: decl.span.clone(),
                };
                let mut params = vec![self_param];
                params.extend(decl.params.clone());
                FuncDecl { name: mangled_name, params, ..decl.clone() }
            };
            register_message_func(module, &mangled);
        }
    }
}

/// Point d'entrée : construit `<nom>__new`/`<nom>__resume` pour la fonction/
/// méthode `emit`-contenante `func` (déjà mangled — `Classe_methode` pour une
/// méthode, `self` inclus dans `func.params` s'il y a lieu, même convention
/// que `lower_func`/`lower_class`). Le layout du frame est déjà connu (voir
/// `register_message_func`, appelée en pré-passage) — ne le recalcule pas.
pub fn lower_message_func(
    module: &mut IrModule,
    func: &FuncDecl,
    fn_ret_types: &HashMap<String, IrType>,
    fn_param_types: &HashMap<String, Vec<IrType>>,
    fn_param_names: &HashMap<String, Vec<String>>,
) {
    if !is_message_func(&func.ret_ty) { return; }
    let frame_class = frame_class_name(&func.name);
    let fields = module.class_layouts.get(&frame_class).cloned()
        .unwrap_or_else(|| {
            register_message_func(module, func);
            module.class_layouts.get(&frame_class).cloned().unwrap_or_default()
        });

    generate_new_fn(module, func, &frame_class, &fields);
    generate_resume_fn(module, func, &fields, fn_ret_types, fn_param_types, fn_param_names);
}

/// `<nom>__new(params...) -> Ptr` : alloue le frame, y stocke chaque
/// argument, initialise `__state` à 0.
fn generate_new_fn(
    module: &mut IrModule,
    func: &FuncDecl,
    frame_class: &str,
    fields: &[(String, IrType)],
) {
    let ir_params: Vec<IrParam> = func.params.iter().enumerate()
        .map(|(i, p)| IrParam { name: p.name.clone(), ty: IrType::from_ast(&p.ty), slot: Value(i as u32) })
        .collect();
    let mut builder = LowerBuilder::new(module, new_func_name(&func.name), ir_params, IrType::Ptr);

    // Setup params (même patron receiver que `lower_func`) : chaque
    // paramètre atterrit dans un Alloca stack ordinaire ICI (cette fonction
    // n'est PAS elle-même un générateur, juste un constructeur one-shot).
    let mut updated_params = Vec::with_capacity(func.params.len());
    for p in &func.params {
        let ir_ty = IrType::from_ast(&p.ty);
        let slot = builder.declare_local(&p.name, ir_ty.clone(), false);
        let recv = builder.new_value();
        builder.emit(Inst::Store { ptr: slot, src: recv.clone() });
        updated_params.push(IrParam { name: p.name.clone(), ty: ir_ty, slot: recv });
    }
    builder.func.params = updated_params;

    let frame = builder.new_value();
    builder.emit(Inst::Alloc { dest: frame.clone(), class: frame_class.to_string() });

    let state_off = field_index(fields, STATE_FIELD) as i32 * 8;
    let zero = builder.new_value();
    builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
    builder.emit(Inst::SetField { obj: frame.clone(), field: STATE_FIELD.to_string(), src: zero, offset: state_off });

    for p in &func.params {
        let (val, _ty) = builder.load_local(&p.name).expect("paramètre déclaré juste au-dessus");
        let off = field_index(fields, &p.name) as i32 * 8;
        builder.emit(Inst::SetField { obj: frame.clone(), field: p.name.clone(), src: val, offset: off });
    }

    builder.emit(Inst::Return { value: Some(frame) });
    let ir_func = builder.func;
    module.add_function(ir_func);
}

/// `<nom>__resume(frame:Ptr) -> Bool` : la machine à états elle-même.
fn generate_resume_fn(
    module: &mut IrModule,
    func: &FuncDecl,
    fields: &[(String, IrType)],
    fn_ret_types: &HashMap<String, IrType>,
    fn_param_types: &HashMap<String, Vec<IrType>>,
    fn_param_names: &HashMap<String, Vec<String>>,
) {
    let ir_params = vec![IrParam { name: "__frame".into(), ty: IrType::Ptr, slot: Value(0) }];
    let mut builder = LowerBuilder::new(module, resume_func_name(&func.name), ir_params, IrType::Bool);
    builder.fn_ret_types   = fn_ret_types.clone();
    builder.fn_param_types = fn_param_types.clone();
    builder.fn_param_names = fn_param_names.clone();

    // Réception du paramètre __frame (même patron receiver que partout
    // ailleurs) — bloc 0, deviendra ensuite le prologue de dispatch.
    let frame_slot = builder.declare_local("__frame", IrType::Ptr, false);
    let frame_recv = builder.new_value();
    builder.emit(Inst::Store { ptr: frame_slot, src: frame_recv.clone() });
    builder.func.params = vec![IrParam { name: "__frame".into(), ty: IrType::Ptr, slot: frame_recv }];
    let (frame_val, _) = builder.load_local("__frame").unwrap();

    // Chaque champ du frame devient une "variable" accessible directement
    // en GetField/SetField (voir `LowerBuilder::frame_vars`) — inclut les
    // pseudo-champs réservés `__state`/`__value`.
    for (idx, (name, ty)) in fields.iter().enumerate() {
        builder.frame_vars.insert(name.clone(), (frame_val.clone(), idx, ty.clone()));
    }

    let start_bb = builder.new_block();
    builder.switch_to(&start_bb);
    crate::lower::stmt::lower_block(&mut builder, &func.body);

    // Fin naturelle du corps (pas de `return`/`emit` terminal) : générateur épuisé.
    if !builder.is_terminated() {
        emit_generator_exhausted(&mut builder);
    }

    // ── Prologue de dispatch (bloc 0) : construit APRÈS coup, une fois
    // `message_resume_blocks` entièrement connu (un par `emit` rencontré). ──
    let entry = BlockId(0);
    builder.switch_to(&entry);
    let state_val = builder.load_local(STATE_FIELD).unwrap().0;

    // état 0 → début du corps (première reprise, juste après __new)
    let zero = builder.new_value();
    builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
    let is_zero = builder.new_value();
    builder.emit(Inst::CmpEq { dest: is_zero.clone(), lhs: state_val.clone(), rhs: zero, ty: IrType::I64 });
    let mut next_check = builder.new_block();
    builder.emit(Inst::Branch { cond: is_zero, then_bb: start_bb.clone(), else_bb: next_check.clone() });

    for (i, (resume_bb, try_ctxs)) in builder.message_resume_blocks.clone().iter().enumerate() {
        // Cas A : rejoue `__ocara_try_enter`+`setjmp` pour chaque `try`
        // encore actif à ce point de suspension (voir §4 de la fiche
        // roadmap), AVANT de sauter au bloc de reprise réel — sinon un
        // `raise` survenant après la reprise ne serait jamais rattrapé (le
        // `setjmp` d'origine appartenait à l'appel `__resume` PRÉCÉDENT,
        // déjà retourné).
        let reentry_bb = build_reentry_stub(&mut builder, resume_bb, try_ctxs);
        builder.switch_to(&next_check);
        let k = (i + 1) as i64;
        let k_val = builder.new_value();
        builder.emit(Inst::ConstInt { dest: k_val.clone(), value: k });
        let is_k = builder.new_value();
        builder.emit(Inst::CmpEq { dest: is_k.clone(), lhs: state_val.clone(), rhs: k_val, ty: IrType::I64 });
        let after = builder.new_block();
        builder.emit(Inst::Branch { cond: is_k, then_bb: reentry_bb, else_bb: after.clone() });
        next_check = after;
    }

    // Ni 0 ni aucun état de reprise connu : épuisé (STATE_DONE, ou toute
    // valeur invalide — défensif) → false immédiat, sans toucher au frame.
    builder.switch_to(&next_check);
    let false_val = builder.new_value();
    builder.emit(Inst::ConstBool { dest: false_val.clone(), value: false });
    builder.emit(Inst::Return { value: Some(false_val) });

    let ir_func = builder.func;
    module.add_function(ir_func);
}

/// Séquence "générateur épuisé" : `__state = STATE_DONE; return false`.
/// Utilisée à la fin naturelle du corps ET par `return;` anticipé (voir
/// `Stmt::Return` dans `crate::lower::stmt::statements`).
pub fn emit_generator_exhausted(builder: &mut LowerBuilder) {
    let done = builder.new_value();
    builder.emit(Inst::ConstInt { dest: done.clone(), value: STATE_DONE });
    builder.store_local(STATE_FIELD, done);
    let false_val = builder.new_value();
    builder.emit(Inst::ConstBool { dest: false_val.clone(), value: false });
    builder.emit(Inst::Return { value: Some(false_val) });
}

fn field_index(fields: &[(String, IrType)], name: &str) -> usize {
    fields.iter().position(|(n, _)| n == name).unwrap_or(0)
}

// ─────────────────────────────────────────────────────────────────────────────
// Cas A : `try { ... emit ... } on e { ... }` DANS un générateur (voir §4 de
// la fiche roadmap) — `try` est lowered INLINE (jamais en fonction séparée
// `__try_body_N`/`__try_handler_N` comme `lower_try`), pour que le `Return`
// natif d'un `emit` sorte bien de `__resume` elle-même. `setjmp` est rejoué
// à chaque reprise (voir le prologue dans `generate_resume_fn`) — seule
// façon de rester valable puisque `__resume` est un nouvel appel natif à
// chaque fois (voir la doc de `__ocara_try_enter`, runtime/src/lib.rs).
// ─────────────────────────────────────────────────────────────────────────────

/// Construit, pour un état de reprise donné, la chaîne de blocs qui rejoue
/// `__ocara_try_enter`+`setjmp` pour chaque `try` encore actif à ce point de
/// suspension (voir la doc de `LowerBuilder::message_resume_blocks`), puis
/// saute au bloc de reprise réel `resume_bb`. Chaîne vide (aucun `try`
/// actif) ⇒ un simple passage direct. Retourne le PREMIER bloc de la
/// chaîne (celui référencé par le prologue de dispatch).
fn build_reentry_stub(builder: &mut LowerBuilder, resume_bb: &BlockId, try_ctxs: &[GenTryCtx]) -> BlockId {
    let entry_bb = builder.new_block();
    let mut current = entry_bb.clone();
    for ctx in try_ctxs {
        builder.switch_to(&current);
        let frame_ptr = call_try_enter(builder);
        builder.store_local(&ctx.frame_field, frame_ptr.clone());
        let jmp_ret = call_raw_setjmp(builder, frame_ptr);
        let next = builder.new_block();
        builder.emit(Inst::Branch { cond: jmp_ret, then_bb: ctx.handler_bb.clone(), else_bb: next.clone() });
        current = next;
    }
    builder.switch_to(&current);
    builder.emit(Inst::Jump { target: resume_bb.clone() });
    entry_bb
}

fn call_try_enter(builder: &mut LowerBuilder) -> Value {
    let dest = builder.new_value();
    builder.emit(Inst::Call { dest: Some(dest.clone()), func: "__ocara_try_enter".into(), args: vec![], ret_ty: IrType::Ptr });
    dest
}

fn call_try_exit(builder: &mut LowerBuilder) {
    builder.emit(Inst::Call { dest: None, func: "__ocara_try_exit".into(), args: vec![], ret_ty: IrType::Void });
}

/// Appelle `setjmp` DIRECTEMENT (pas de wrapper Rust — voir la doc de
/// `BuiltinDesc { name: "setjmp", ... }`, `src/codegen/desc.d/lowlevel.rs`) :
/// la frame qui doit "rester vivante" pour un `longjmp` ultérieur est celle
/// de `__resume` elle-même, l'appelante directe ICI.
fn call_raw_setjmp(builder: &mut LowerBuilder, frame_ptr: Value) -> Value {
    let dest = builder.new_value();
    builder.emit(Inst::Call { dest: Some(dest.clone()), func: "setjmp".into(), args: vec![frame_ptr], ret_ty: IrType::I64 });
    dest
}

/// Lit `error_val`/`error_type` depuis un pointeur `TryFrame` (offsets
/// 200/208 — voir la doc de `__ocara_try_enter`, runtime/src/lib.rs : champ
/// `env` = 25×u64 = 200 octets, puis les deux `i64`).
fn read_try_error(builder: &mut LowerBuilder, frame_ptr: Value) -> (Value, Value) {
    let err_val = builder.new_value();
    builder.emit(Inst::GetField { dest: err_val.clone(), obj: frame_ptr.clone(), field: "error_val".into(), ty: IrType::I64, offset: 200 });
    let err_type = builder.new_value();
    builder.emit(Inst::GetField { dest: err_type.clone(), obj: frame_ptr, field: "error_type".into(), ty: IrType::I64, offset: 208 });
    (err_val, err_type)
}

/// Lowering INLINE de `try { body } on e [is Foo] { ... } ...` dans le corps
/// d'un générateur — voir la doc de section ci-dessus. Contrairement à
/// `lower_try` (fonctions séparées + trampoline C), le corps ET les
/// gestionnaires sont lowered directement dans les blocs de `__resume`, via
/// le `lower_block` générique (donc `emit` y fonctionne normalement).
pub fn lower_try_in_generator(builder: &mut LowerBuilder, body: &Block, handlers: &[OnClause]) {
    let depth = builder.gen_try_stack.len();
    let frame_field = try_frame_field(depth);

    let frame_ptr = call_try_enter(builder);
    builder.store_local(&frame_field, frame_ptr.clone());
    let jmp_ret = call_raw_setjmp(builder, frame_ptr);

    let handler_bb = builder.new_block();
    let body_start_bb = builder.new_block();
    let after_bb = builder.new_block();
    // `setjmp` : 0 = chemin normal (corps), non-nul = un `longjmp` est arrivé
    // ici (voir la doc de `Inst::Branch` : accepte un I64 directement, même
    // patron que `__ocara_type_matches` dans `lower_try`).
    builder.emit(Inst::Branch { cond: jmp_ret, then_bb: handler_bb.clone(), else_bb: body_start_bb.clone() });

    // ── Corps du try (chemin normal) ─────────────────────────────────────
    builder.switch_to(&body_start_bb);
    builder.gen_try_stack.push(GenTryCtx { frame_field: frame_field.clone(), handler_bb: handler_bb.clone() });
    crate::lower::stmt::lower_block(builder, body);
    builder.gen_try_stack.pop();
    if !builder.is_terminated() {
        call_try_exit(builder);
        builder.emit(Inst::Jump { target: after_bb.clone() });
    }

    // ── Gestionnaire(s) (chemin `longjmp`) — lowered UNE SEULE FOIS, partagé
    // par la première exécution ET tout rejeu ultérieur (voir le prologue de
    // `generate_resume_fn`) ────────────────────────────────────────────────
    builder.switch_to(&handler_bb);
    call_try_exit(builder);
    let (frame_for_handler, _) = builder.load_local(&frame_field).unwrap();
    let (err_val, err_type) = read_try_error(builder, frame_for_handler);

    let mut next_check = builder.new_block();
    builder.emit(Inst::Jump { target: next_check.clone() });

    for handler in handlers {
        builder.switch_to(&next_check);
        let clause_bb = builder.new_block();
        let after_check = builder.new_block();
        if let Some(class_filter) = &handler.class_filter {
            let filter_idx = builder.module.intern_string(class_filter);
            let filter_val = builder.new_value();
            builder.emit(Inst::ConstStr { dest: filter_val.clone(), idx: filter_idx });
            let match_result = builder.new_value();
            builder.emit(Inst::Call {
                dest:   Some(match_result.clone()),
                func:   "__ocara_type_matches".into(),
                args:   vec![err_type.clone(), filter_val],
                ret_ty: IrType::I64,
            });
            builder.emit(Inst::Branch { cond: match_result, then_bb: clause_bb.clone(), else_bb: after_check.clone() });
        } else {
            builder.emit(Inst::Jump { target: clause_bb.clone() });
        }

        builder.switch_to(&clause_bb);
        builder.store_local(&handler.binding, err_val.clone());
        let exception_class = handler.class_filter.clone().unwrap_or_else(|| "Exception".to_string());
        builder.var_class.insert(handler.binding.clone(), exception_class);
        crate::lower::stmt::lower_block(builder, &handler.body);
        if !builder.is_terminated() {
            builder.emit(Inst::Jump { target: after_bb.clone() });
        }

        next_check = after_check;
    }

    // Aucun handler ne correspond (ou liste vide) — re-propager l'erreur :
    // `__ocara_fail` retrouvera le PROCHAIN `try` actif (celui-ci vient
    // d'être dépilé ci-dessus), exactement comme le fait `lower_try`.
    builder.switch_to(&next_check);
    builder.emit(Inst::Call {
        dest:   None,
        func:   "__ocara_fail".into(),
        args:   vec![err_val, err_type],
        ret_ty: IrType::Void,
    });
    let dead_false = builder.new_value();
    builder.emit(Inst::ConstBool { dest: dead_false.clone(), value: false });
    builder.emit(Inst::Return { value: Some(dead_false) });

    builder.switch_to(&after_bb);
}

// ─────────────────────────────────────────────────────────────────────────────
// Côté consommateur : les 2 formes câblées pour cette étape (`for`, scalaire
// direct) — `Array::fromMessage` est l'Étape 6 (Runtime), pas encore ici.
// ─────────────────────────────────────────────────────────────────────────────

/// Détecte si `expr` est un appel à une fonction/méthode générateur connue
/// (`nom(...)`, `obj.nom(...)`, `Classe::nom(...)`) — retourne son nom
/// mangled et le type IR de `T` si oui. Ne consomme rien, ne modifie aucun
/// état : juste une consultation de `module.message_funcs`.
pub fn detect_message_call(builder: &LowerBuilder, expr: &Expr) -> Option<(String, IrType)> {
    match expr {
        Expr::Call { callee, .. } => match callee.as_ref() {
            Expr::Ident(name, _) => builder.module.message_funcs.get(name).cloned().map(|ty| (name.clone(), ty)),
            Expr::Field { object, field, .. } => {
                let class_name = match object.as_ref() {
                    Expr::Ident(n, _)  => builder.var_class.get(n.as_str()).cloned(),
                    Expr::SelfExpr(_)  => builder.current_class.clone(),
                    _ => None,
                };
                class_name.and_then(|cls| {
                    let mangled = format!("{}_{}", cls, field);
                    builder.module.message_funcs.get(&mangled).cloned().map(|ty| (mangled, ty))
                })
            }
            _ => None,
        },
        Expr::StaticCall { class, method, .. } => {
            let resolved = if class == "<self>" {
                builder.current_class.clone().unwrap_or_default()
            } else {
                class.clone()
            };
            let mangled = format!("{}_{}", resolved, method);
            builder.module.message_funcs.get(&mangled).cloned().map(|ty| (mangled, ty))
        }
        _ => None,
    }
}

/// Évalue les arguments de `expr` (+ le récepteur en tête pour un appel de
/// méthode d'instance `obj.nom(...)`), dans l'ordre attendu par `<mangled>__new`.
fn collect_call_args(builder: &mut LowerBuilder, expr: &Expr) -> Vec<Value> {
    match expr {
        Expr::Call { callee, args, .. } => {
            let mut vals = Vec::new();
            if let Expr::Field { object, .. } = callee.as_ref() {
                vals.push(crate::lower::expr::lower_expr(builder, object));
            }
            for a in args {
                vals.push(crate::lower::expr::lower_expr(builder, a));
            }
            vals
        }
        Expr::StaticCall { args, .. } => {
            args.iter().map(|a| crate::lower::expr::lower_expr(builder, a)).collect()
        }
        _ => Vec::new(),
    }
}

fn call_new(builder: &mut LowerBuilder, mangled: &str, args: Vec<Value>) -> Value {
    let dest = builder.new_value();
    builder.emit(Inst::Call { dest: Some(dest.clone()), func: new_func_name(mangled), args, ret_ty: IrType::Ptr });
    dest
}

fn call_resume(builder: &mut LowerBuilder, mangled: &str, frame: Value) -> Value {
    let dest = builder.new_value();
    builder.emit(Inst::Call { dest: Some(dest.clone()), func: resume_func_name(mangled), args: vec![frame], ret_ty: IrType::Bool });
    dest
}

/// `__value` est TOUJOURS le champ d'index 1 (juste après `__state`), quel
/// que soit le générateur — convention fixe de `lower_message_func`, pas
/// besoin de consulter le layout ici.
fn get_value_field(builder: &mut LowerBuilder, frame: Value, elem_ty: IrType) -> Value {
    let dest = builder.new_value();
    builder.emit(Inst::GetField { dest: dest.clone(), obj: frame, field: VALUE_FIELD.to_string(), ty: elem_ty, offset: 8 });
    dest
}

fn free_frame(builder: &mut LowerBuilder, mangled: &str, frame: Value) {
    let n_fields = builder.module.class_layouts.get(&frame_class_name(mangled)).map(|f| f.len()).unwrap_or(0);
    let size = builder.new_value();
    builder.emit(Inst::ConstInt { dest: size.clone(), value: (n_fields as i64) * 8 });
    builder.emit(Inst::Call { dest: None, func: "__free_obj".into(), args: vec![frame, size], ret_ty: IrType::Void });
}

/// Lowering d'un argument d'appel qui peut être une consommation scalaire
/// directe d'un `message<T>` (`f(truc())`, voir §2 de la fiche roadmap) —
/// repli sur `lower_expr` normal sinon. Utilisé par les listes d'arguments
/// de `Expr::Call`/`Expr::StaticCall`, mêmes sites que la sema (voir
/// `crate::sema::typecheck::check_message_scalar_consumption`).
pub fn lower_arg_or_message(builder: &mut LowerBuilder, expr: &Expr) -> Value {
    if let Some((mangled, elem_ty)) = detect_message_call(builder, expr) {
        return lower_message_scalar(builder, expr, &mangled, elem_ty);
    }
    crate::lower::expr::lower_expr(builder, expr)
}

/// Consommation SCALAIRE directe d'un `message<T>` (voir §2 de la fiche
/// roadmap, gardée par la sema — `crate::sema::typecheck::check_message_scalar_consumption` —
/// qui garantit qu'au plus un `emit` est atteignable hors boucle) : un seul
/// `resume`, lecture de `__value`, libération immédiate du frame (usage
/// unique). Retourne la valeur `T` déjà déballée.
pub fn lower_message_scalar(builder: &mut LowerBuilder, expr: &Expr, mangled: &str, elem_ty: IrType) -> Value {
    let args = collect_call_args(builder, expr);
    let frame = call_new(builder, mangled, args);
    let _has_val = call_resume(builder, mangled, frame.clone());
    let val = get_value_field(builder, frame.clone(), elem_ty);
    free_frame(builder, mangled, frame);
    val
}

/// `for x in truc(...) { body }` où `truc` est un générateur (voir §2 de la
/// fiche roadmap) : AUCUNE restriction (contrairement à la consommation
/// scalaire), même avec un `emit` en boucle. Le frame est libéré au point de
/// fusion (`merge_bb`) — atteint aussi bien par épuisement naturel que par
/// `break` (même bloc cible, voir `crate::lower::stmt::ownership::lower_break`) ;
/// un `return`/`raise` anticipé depuis le corps saute directement hors de la
/// fonction sans jamais atteindre `merge_bb` et fuit donc le frame — limite
/// acceptée pour cette étape, voir docs/roadmap.d/langage-emit-iterable.md.
pub fn lower_for_message(
    builder: &mut LowerBuilder,
    var: &str,
    expr: &Expr,
    mangled: &str,
    elem_ty: IrType,
    body: &Block,
) {
    let args = collect_call_args(builder, expr);
    let frame = call_new(builder, mangled, args);

    let cond_bb  = builder.new_block();
    let body_bb  = builder.new_block();
    let merge_bb = builder.new_block();

    builder.emit(Inst::Jump { target: cond_bb.clone() });
    builder.switch_to(&cond_bb);
    let has_val = call_resume(builder, mangled, frame.clone());
    builder.emit(Inst::Branch { cond: has_val, then_bb: body_bb.clone(), else_bb: merge_bb.clone() });

    builder.switch_to(&body_bb);
    let val = get_value_field(builder, frame.clone(), elem_ty.clone());
    builder.declare_local(var, elem_ty, false);
    builder.store_local(var, val);

    builder.loop_stack.push((cond_bb.clone(), merge_bb.clone(), builder.block_scope_stack.len()));
    builder.loop_depth += 1;
    crate::lower::stmt::lower_block(builder, body);
    builder.loop_depth -= 1;
    builder.loop_stack.pop();

    if !builder.is_terminated() {
        builder.emit(Inst::Jump { target: cond_bb.clone() });
    }

    builder.switch_to(&merge_bb);
    free_frame(builder, mangled, frame);
}

/// Nom du champ frame portant le pointeur `TryFrame` courant au niveau
/// d'imbrication `depth` (0 = try le plus externe) — voir §4 "Cas A" de la
/// fiche roadmap et `lower_try_in_generator`. Des `try` FRÈRES (jamais
/// actifs simultanément) au même niveau partagent le même champ, comme
/// n'importe quel autre local dédupliqué par nom.
pub fn try_frame_field(depth: usize) -> String {
    format!("__try_frame_{}", depth)
}

/// `Array::fromMessage(truc())` (voir §2 de la fiche roadmap) : draine TOUS
/// les `emit` (aucune restriction, même avec `emit` en boucle) dans un
/// `array<T>` neuf — mêmes conventions de stockage qu'un littéral `array<T>`
/// à type concret connu (`LiteralElemKind::Concrete` — voir
/// `lower_array_literal`) : valeur brute, jamais boxée.
pub fn lower_array_from_message(builder: &mut LowerBuilder, expr: &Expr, mangled: &str, elem_ty: IrType) -> Value {
    let args = collect_call_args(builder, expr);
    let frame = call_new(builder, mangled, args);

    let arr = builder.new_value();
    builder.emit(Inst::Call { dest: Some(arr.clone()), func: "__array_new".into(), args: vec![], ret_ty: IrType::Ptr });

    let cond_bb  = builder.new_block();
    let body_bb  = builder.new_block();
    let after_bb = builder.new_block();

    builder.emit(Inst::Jump { target: cond_bb.clone() });
    builder.switch_to(&cond_bb);
    let has_val = call_resume(builder, mangled, frame.clone());
    builder.emit(Inst::Branch { cond: has_val, then_bb: body_bb.clone(), else_bb: after_bb.clone() });

    builder.switch_to(&body_bb);
    let val = get_value_field(builder, frame.clone(), elem_ty);
    builder.emit(Inst::Call { dest: None, func: "__array_push".into(), args: vec![arr.clone(), val], ret_ty: IrType::Void });
    builder.emit(Inst::Jump { target: cond_bb.clone() });

    builder.switch_to(&after_bb);
    free_frame(builder, mangled, frame);
    arr
}

/// Collecte (nom, type IR), dédupliqué par nom, de toute variable déclarée
/// n'importe où dans `body` (`var`/`const`/variable de `for`/`for-map`,
/// champ `__try_frame_N` par niveau de `try` imbriqué, binding `on e`) —
/// même ordre de parcours que `crate::sema::message_emit`, mais ne s'en sert
/// pas directement (celui-ci ne retourne que des booléens).
fn collect_local_decls(body: &Block) -> Vec<(String, IrType)> {
    let mut out = Vec::new();
    walk_block(body, 0, &mut out);
    out
}

fn push_unique(out: &mut Vec<(String, IrType)>, name: String, ty: IrType) {
    if !out.iter().any(|(n, _)| n == &name) {
        out.push((name, ty));
    }
}

fn walk_block(block: &Block, try_depth: usize, out: &mut Vec<(String, IrType)>) {
    walk_stmts(&block.stmts, try_depth, out);
}

fn walk_stmts(stmts: &[Stmt], try_depth: usize, out: &mut Vec<(String, IrType)>) {
    for stmt in stmts {
        match stmt {
            Stmt::Var { name, ty, .. } | Stmt::Const { name, ty, .. } => {
                push_unique(out, name.clone(), IrType::from_ast(ty));
            }
            Stmt::ForIn { var, iter, body, .. } => {
                // Même heuristique que `lower_for_in` : I64 pour un range,
                // Ptr par défaut sinon (voir sa doc — limitation acceptée
                // pour un `for` concret imbriqué dans un générateur, cas
                // marginal pour cette première implémentation).
                let ty = if matches!(iter, Expr::Range { .. }) { IrType::I64 } else { IrType::Ptr };
                push_unique(out, var.clone(), ty);
                walk_block(body, try_depth, out);
            }
            Stmt::ForMap { key, value, body, .. } => {
                push_unique(out, key.clone(), IrType::Ptr);
                push_unique(out, value.clone(), IrType::I64);
                walk_block(body, try_depth, out);
            }
            Stmt::If { then_block, elseif, else_block, .. } => {
                walk_block(then_block, try_depth, out);
                for (_, blk) in elseif { walk_block(blk, try_depth, out); }
                if let Some(blk) = else_block { walk_block(blk, try_depth, out); }
            }
            Stmt::Switch { cases, default, .. } => {
                for case in cases { walk_block(&case.body, try_depth, out); }
                if let Some(blk) = default { walk_block(blk, try_depth, out); }
            }
            Stmt::While { body, .. } => walk_block(body, try_depth, out),
            Stmt::Try { body, handlers, .. } => {
                push_unique(out, try_frame_field(try_depth), IrType::Ptr);
                walk_block(body, try_depth + 1, out);
                for h in handlers {
                    // Même type que le binding `on e` de `lower_try` (I64,
                    // pas Ptr — cosmétique au niveau du lowering, voir sa doc).
                    push_unique(out, h.binding.clone(), IrType::I64);
                    walk_block(&h.body, try_depth, out);
                }
            }
            Stmt::Expr(_) | Stmt::Return { .. } | Stmt::Result { .. } | Stmt::Break { .. }
            | Stmt::Continue { .. } | Stmt::Raise { .. } | Stmt::Assign { .. } | Stmt::Emit { .. } => {}
        }
    }
}
