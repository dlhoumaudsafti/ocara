/// Lowering des boucles (for in, for map, break, continue)

use crate::parsing::ast::*;
use crate::ir::types::IrType;
use crate::ir::inst::Inst;
use crate::lower::builder::LowerBuilder;
use crate::lower::expr::{lower_expr, hoist_closure_promotions_before_loop};
use super::super::super::block::lower_block;

pub fn lower_for_in(
    builder: &mut LowerBuilder,
    var: &str,
    iter: &Expr,
    body: &Block,
) {
    // `for x in truc(...)` où `truc` est un générateur (`emit`/`message<T>`,
    // voir docs/roadmap.d/langage-emit-iterable.md) : lowering entièrement
    // différent (machine à états, pas de tableau) — voir
    // `crate::lower::builder::message_gen::lower_for_message`.
    if let Some((mangled, elem_ty)) = crate::lower::builder::message_gen::detect_message_call(builder, iter) {
        crate::lower::builder::message_gen::lower_for_message(builder, var, iter, &mangled, elem_ty, body);
        return;
    }

    // Pré-promotion : voir docs/roadmap.d/langage-closure-promotion-in-loop.md.
    // AVANT de déclarer `var` (la variable d'itération elle-même n'existe pas
    // encore ici, donc jamais concernée par ce pré-scan — seules les
    // variables déjà existantes AVANT la boucle le sont).
    hoist_closure_promotions_before_loop(builder, body);

    // Lowering : __iter_init(iter), boucle sur __iter_next
    let iter_val  = lower_expr(builder, iter);
    let idx_slot  = builder.declare_local("__for_idx", IrType::I64, true);
    let zero = builder.new_value();
    builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
    builder.emit(Inst::Store { ptr: idx_slot.clone(), src: zero });

    // Type de l'élément : I64 pour les plages entières, Ptr pour les tableaux
    let elem_ty = match iter {
        Expr::Range { .. } => IrType::I64,
        Expr::Ident(name, _) => {
            builder.elem_types.get(name.as_str()).cloned().unwrap_or(IrType::Ptr)
        }
        _ => IrType::Ptr,
    };

    // Longueur du tableau
    let len_val = builder.new_value();
    builder.emit(Inst::Call {
        dest:   Some(len_val.clone()),
        func:   "__array_len".into(),
        args:   vec![iter_val.clone()],
        ret_ty: IrType::I64,
    });

    let cond_bb  = builder.new_block();
    let body_bb  = builder.new_block();
    let incr_bb  = builder.new_block();
    let merge_bb = builder.new_block();

    builder.emit(Inst::Jump { target: cond_bb.clone() });
    builder.switch_to(&cond_bb);

    let idx = builder.new_value();
    builder.emit(Inst::Load { dest: idx.clone(), ptr: idx_slot.clone(), ty: IrType::I64 });
    let cond = builder.new_value();
    builder.emit(Inst::CmpLt {
        dest: cond.clone(),
        lhs:  idx.clone(),
        rhs:  len_val.clone(),
        ty:   IrType::I64,
    });
    builder.emit(Inst::Branch {
        cond:    cond,
        then_bb: body_bb.clone(),
        else_bb: merge_bb.clone(),
    });

    builder.switch_to(&body_bb);
    // Charge l'élément courant
    let elem = builder.new_value();
    
    // Pour les paramètres variadic, le tableau IR est Ptr (mixed[]) donc on doit traiter
    // différemment : récupérer comme I64 puis caster/unboxer si nécessaire
    let is_variadic = if let Expr::Ident(name, _) = iter {
        builder.variadic_params.contains(name.as_str())
    } else {
        false
    };
    
    if is_variadic {
        // Variadic : le tableau est mixed[], donc __array_get retourne un i64 brut
        builder.emit(Inst::Call {
            dest:   Some(elem.clone()),
            func:   "__array_get".into(),
            args:   vec![iter_val.clone(), idx.clone()],
            ret_ty: IrType::I64,  // Le tableau mixed contient des i64
        });
        // Les int sont déjà corrects en i64, pas besoin d'unboxing
        // Les float/bool nécessiteraient unboxing mais pour l'instant on les laisse
    } else {
        // Tableau normal : utiliser le type d'élément
        builder.emit(Inst::Call {
            dest:   Some(elem.clone()),
            func:   "__array_get".into(),
            args:   vec![iter_val.clone(), idx.clone()],
            ret_ty: elem_ty.clone(),
        });
    }
    
    builder.declare_local(var, elem_ty.clone(), false);
    builder.store_local(var, elem);
    
    // Si l'itérateur est une variable dont le type d'élément est connu
    // statiquement (`elem_ast_types`, alimenté par `lower_var`/`lower_const`
    // pour toute variable `array<T>` — voir variables.rs), enregistrer les
    // métadonnées adéquates pour la variable de boucle `var`.
    //
    // Bug historique corrigé ici : seul le cas `Type::Map` (élément map,
    // `array<map<K,V>>`) était géré — `array<Classe>` (ou `array<Classe|null>`)
    // ne l'était PAS, donc `var` (la variable de boucle) n'avait AUCUNE
    // entrée `var_class`. Un accès de champ (`Expr::Field`) sur `var` dans le
    // corps de la boucle (`for it in items { ... it.champ ... }`) résolvait
    // alors `class_name = None`, ce qui retombe sur `offset = 0` pour
    // N'IMPORTE QUEL champ — silencieusement, TOUJOURS la valeur du premier
    // champ déclaré. Même famille de bug, même correctif que
    // `register_var_class`/`union_named_class` — voir
    // docs/roadmap.d/langage-union-class-null-field-access.md.
    if let Expr::Ident(iter_name, _) = iter {
        if let Some(elem_ast_ty) = builder.elem_ast_types.get(iter_name.as_str()).cloned() {
            if let Type::Map(_, val_ty) = &elem_ast_ty {
                // L'élément est un map, enregistrer la variable d'itération comme map
                builder.map_vars.insert(var.to_string());
                builder.elem_types.insert(var.to_string(), IrType::from_ast(val_ty));
                builder.var_class.insert(var.to_string(), "Map".to_string());
            } else if let Some(class_name) = resolved_named_class(&elem_ast_ty) {
                // `array<Classe>` (ou `array<Classe|null>`) : la variable de
                // boucle est une instance de Classe.
                builder.var_class.insert(var.to_string(), class_name);
            }
        }
    }

    // continue → incr_bb, break → merge_bb
    builder.loop_stack.push((incr_bb.clone(), merge_bb.clone(), builder.block_scope_stack.len()));
    builder.loop_depth += 1;
    lower_block(builder, body);
    builder.loop_depth -= 1;
    builder.loop_stack.pop();

    if !builder.is_terminated() {
        builder.emit(Inst::Jump { target: incr_bb.clone() });
    }

    // Bloc incrément
    builder.switch_to(&incr_bb);
    let one = builder.new_value();
    builder.emit(Inst::ConstInt { dest: one.clone(), value: 1 });
    let idx2 = builder.new_value();
    builder.emit(Inst::Load { dest: idx2.clone(), ptr: idx_slot.clone(), ty: IrType::I64 });
    let next_idx = builder.new_value();
    builder.emit(Inst::Add { dest: next_idx.clone(), lhs: idx2, rhs: one, ty: IrType::I64 });
    builder.emit(Inst::Store { ptr: idx_slot, src: next_idx });
    builder.emit(Inst::Jump { target: cond_bb.clone() });

    builder.switch_to(&merge_bb);
}

pub fn lower_for_map(
    builder: &mut LowerBuilder,
    key: &str,
    value: &str,
    iter: &Expr,
    body: &Block,
) {
    // Pré-promotion : voir docs/roadmap.d/langage-closure-promotion-in-loop.md.
    hoist_closure_promotions_before_loop(builder, body);

    let iter_val = lower_expr(builder, iter);

    // Récupère le tableau des clés
    let keys_arr = builder.new_value();
    builder.emit(Inst::Call {
        dest:   Some(keys_arr.clone()),
        func:   "Map_keys".into(),
        args:   vec![iter_val.clone()],
        ret_ty: IrType::Ptr,
    });

    // Longueur
    let len_val = builder.new_value();
    builder.emit(Inst::Call {
        dest:   Some(len_val.clone()),
        func:   "__array_len".into(),
        args:   vec![keys_arr.clone()],
        ret_ty: IrType::I64,
    });

    // Index
    let idx_slot = builder.declare_local("__map_idx", IrType::I64, true);
    let zero = builder.new_value();
    builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
    builder.emit(Inst::Store { ptr: idx_slot.clone(), src: zero });

    let cond_bb  = builder.new_block();
    let body_bb  = builder.new_block();
    let incr_bb  = builder.new_block();
    let merge_bb = builder.new_block();

    builder.emit(Inst::Jump { target: cond_bb.clone() });
    builder.switch_to(&cond_bb);

    let idx = builder.new_value();
    builder.emit(Inst::Load { dest: idx.clone(), ptr: idx_slot.clone(), ty: IrType::I64 });
    let cond = builder.new_value();
    builder.emit(Inst::CmpLt {
        dest: cond.clone(), lhs: idx.clone(), rhs: len_val.clone(), ty: IrType::I64,
    });
    builder.emit(Inst::Branch { cond, then_bb: body_bb.clone(), else_bb: merge_bb.clone() });

    builder.switch_to(&body_bb);

    // Clé courante
    let k = builder.new_value();
    builder.emit(Inst::Call {
        dest:   Some(k.clone()),
        func:   "__array_get".into(),
        args:   vec![keys_arr.clone(), idx.clone()],
        ret_ty: IrType::Ptr,
    });
    builder.declare_local(key, IrType::Ptr, false);
    builder.store_local(key, k.clone());

    // Valeur correspondante
    let v = builder.new_value();
    builder.emit(Inst::Call {
        dest:   Some(v.clone()),
        func:   "__map_get".into(),
        args:   vec![iter_val.clone(), k],
        ret_ty: IrType::I64,
    });
    builder.declare_local(value, IrType::I64, false);
    builder.store_local(value, v);

    // Si l'itérateur est une variable `map<K,V>` dont le type de VALEUR est
    // connu statiquement (`elem_ast_types`, alimenté par `lower_var`/
    // `lower_const` pour toute variable map — voir variables.rs), enregistrer
    // `value` (la variable liée à la valeur courante) comme instance de
    // Classe si `V` (ou `V` dans `Classe|null`) en est une — même bug/même
    // correctif que la variable d'itération de `for x in array<Classe>`
    // ci-dessus (`lower_for_in`), jamais couvert du tout ici avant ce
    // correctif (`value` n'avait AUCUNE entrée `var_class`, quel que soit le
    // type de valeur de la map). Voir
    // docs/roadmap.d/langage-union-class-null-field-access.md.
    if let Expr::Ident(map_name, _) = iter {
        if let Some(val_ast_ty) = builder.elem_ast_types.get(map_name.as_str()).cloned() {
            if let Some(class_name) = resolved_named_class(&val_ast_ty) {
                builder.var_class.insert(value.to_string(), class_name);
            }
        }
    }

    // continue → incr_bb, break → merge_bb
    builder.loop_stack.push((incr_bb.clone(), merge_bb.clone(), builder.block_scope_stack.len()));
    builder.loop_depth += 1;
    lower_block(builder, body);
    builder.loop_depth -= 1;
    builder.loop_stack.pop();

    if !builder.is_terminated() {
        builder.emit(Inst::Jump { target: incr_bb.clone() });
    }

    // Bloc incrément
    builder.switch_to(&incr_bb);
    let one = builder.new_value();
    builder.emit(Inst::ConstInt { dest: one.clone(), value: 1 });
    let idx2 = builder.new_value();
    builder.emit(Inst::Load { dest: idx2.clone(), ptr: idx_slot.clone(), ty: IrType::I64 });
    let next_idx = builder.new_value();
    builder.emit(Inst::Add { dest: next_idx.clone(), lhs: idx2, rhs: one, ty: IrType::I64 });
    builder.emit(Inst::Store { ptr: idx_slot, src: next_idx });
    builder.emit(Inst::Jump { target: cond_bb.clone() });

    builder.switch_to(&merge_bb);
}

pub fn lower_break(builder: &mut LowerBuilder) {
    if let Some((_, break_bb, depth)) = builder.loop_stack.last().cloned() {
        // Détruit les scoped/consumed encore vivantes entre ici et l'entrée
        // de la boucle (corps de boucle inclus) avant de sauter dehors —
        // voir crate::lower::stmt::ownership::emit_early_exit_drops.
        crate::lower::stmt::ownership::emit_early_exit_drops(builder, depth);
        builder.emit(Inst::Jump { target: break_bb });
    }
}

pub fn lower_continue(builder: &mut LowerBuilder) {
    if let Some((continue_bb, _, depth)) = builder.loop_stack.last().cloned() {
        // Même destruction que `break` : `continue` quitte aussi le corps
        // de boucle actuellement ouvert (et tout ce qu'il contient), juste
        // pour reboucler plutôt que sortir complètement.
        crate::lower::stmt::ownership::emit_early_exit_drops(builder, depth);
        builder.emit(Inst::Jump { target: continue_bb });
    }
}

/// Tests unitaires — `for x in array<Classe>` / `for k => v in map<K,Classe>`
/// (docs/roadmap.d/langage-union-class-null-field-access.md) : la variable
/// de boucle/valeur n'avait AUCUNE entrée `var_class` dès que l'élément
/// itéré était une classe utilisateur (seul l'élément `map` était géré) —
/// tout accès de champ sur la variable de boucle résolvait alors
/// `offset = 0`, silencieusement toujours la valeur du premier champ
/// déclaré, quel que soit le champ réellement demandé.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::module::IrModule;
    use crate::parsing::token::Span;

    fn span() -> Span { Span::new(0, 0) }
    fn ident(name: &str) -> Expr { Expr::Ident(name.to_string(), span()) }
    fn empty_block() -> Block { Block { stmts: vec![], span: span() } }

    /// Cas exact du second repro rapporté : `for it in items` où
    /// `items:array<Foo>` — `it` doit être enregistrée comme instance de Foo.
    #[test]
    fn for_in_array_of_class_registers_var_class() {
        let mut module = IrModule::new("test");
        let mut builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);
        builder.elem_ast_types.insert("items".to_string(), Type::Named("Foo".to_string()));

        lower_for_in(&mut builder, "it", &ident("items"), &empty_block());

        assert_eq!(builder.var_class.get("it"), Some(&"Foo".to_string()));
    }

    /// `array<Foo|null>` — même dépliage que pour un `var`/`const` de type
    /// union (voir `register_var_class`, variables.rs), via `resolved_named_class`.
    #[test]
    fn for_in_array_of_nullable_class_registers_var_class() {
        let mut module = IrModule::new("test");
        let mut builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);
        builder.elem_ast_types.insert(
            "items".to_string(),
            Type::Union(vec![Type::Named("Foo".to_string()), Type::Null]),
        );

        lower_for_in(&mut builder, "it", &ident("items"), &empty_block());

        assert_eq!(builder.var_class.get("it"), Some(&"Foo".to_string()));
    }

    /// Non-régression : le chemin `array<map<K,V>>` déjà géré avant ce
    /// correctif doit continuer à enregistrer "Map", pas une classe.
    #[test]
    fn for_in_array_of_map_still_registers_map_unaffected() {
        let mut module = IrModule::new("test");
        let mut builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);
        builder.elem_ast_types.insert(
            "rows".to_string(),
            Type::Map(Box::new(Type::String), Box::new(Type::Mixed)),
        );

        lower_for_in(&mut builder, "row", &ident("rows"), &empty_block());

        assert_eq!(builder.var_class.get("row"), Some(&"Map".to_string()));
        assert!(builder.map_vars.contains("row"));
    }

    /// Non-régression : `array<int>` (aucune classe impliquée) ne doit
    /// produire aucune entrée `var_class`.
    #[test]
    fn for_in_array_of_primitive_registers_nothing() {
        let mut module = IrModule::new("test");
        let mut builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);
        builder.elem_ast_types.insert("nums".to_string(), Type::Int);

        lower_for_in(&mut builder, "n", &ident("nums"), &empty_block());

        assert_eq!(builder.var_class.get("n"), None);
    }

    /// `for k => v in m` où `m:map<string, Foo>` — la variable VALEUR doit
    /// être enregistrée comme instance de Foo (jamais géré du tout avant ce
    /// correctif : `lower_for_map` n'enregistrait aucune métadonnée de
    /// classe pour `value`, quel que soit le type de valeur de la map).
    #[test]
    fn for_map_value_class_registers_var_class() {
        let mut module = IrModule::new("test");
        let mut builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);
        builder.elem_ast_types.insert("m".to_string(), Type::Named("Foo".to_string()));

        lower_for_map(&mut builder, "k", "v", &ident("m"), &empty_block());

        assert_eq!(builder.var_class.get("v"), Some(&"Foo".to_string()));
    }

    /// `map<string, Foo|null>` — même dépliage union que pour `for x in`.
    #[test]
    fn for_map_value_nullable_class_registers_var_class() {
        let mut module = IrModule::new("test");
        let mut builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);
        builder.elem_ast_types.insert(
            "m".to_string(),
            Type::Union(vec![Type::Named("Foo".to_string()), Type::Null]),
        );

        lower_for_map(&mut builder, "k", "v", &ident("m"), &empty_block());

        assert_eq!(builder.var_class.get("v"), Some(&"Foo".to_string()));
    }
}
