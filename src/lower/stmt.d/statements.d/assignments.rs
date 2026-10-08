/// Lowering des affectations

use crate::parsing::ast::*;
use crate::ir::types::IrType;
use crate::ir::inst::Inst;
use crate::lower::builder::LowerBuilder;
use crate::ir::inst::Value;
use crate::lower::expr::{lower_expr, expr_ir_type_pub};
use super::helpers::box_for_any;
use crate::lower::expr::helpers::{resolve_chained_field_class, is_map_target, field_offset, field_ir_type, elem_type_after_index};

pub fn lower_assign(
    builder: &mut LowerBuilder,
    target: &Expr,
    value: &Expr,
) {
    let val_ty = expr_ir_type_pub(builder, value);
    // Littéral `array`/`map` affecté : typé par le type déclaré de la cible
    // (`self.counts = {"a": 0}` pour `map<string,int>`), comme pour `var`/
    // `return` — sinon ses scalaires étaient boxés alors que la cible les
    // relit bruts (`free(): invalid pointer` à la libération).
    let val = match declared_container_type(builder, target) {
        Some(ty) => super::variables::lower_literal_or_expr(builder, value, &ty),
        None => lower_expr(builder, value),
    };

    match target {
        Expr::Ident(name, _) => {
            // Boxing si la variable cible est mixed
            let target_ty = builder.frame_vars.get(name.as_str())
                .map(|(_, _, ty)| ty.clone())
                .or_else(|| builder.locals.get(name.as_str()).map(|(_, ty, _)| ty.clone()))
                .or_else(|| builder.captured_vars.get(name.as_str()).map(|(_, _, ty)| ty.clone()))
                .unwrap_or(IrType::I64);
            let boxed = box_for_any(builder, &target_ty, val_ty, val.clone());
            if crate::lower::stmt::rc::is_counted_local(builder, name) {
                crate::lower::stmt::rc::assign_local(builder, name, boxed.clone(), boxed != val);
            } else {
                builder.store_local(name, boxed);
            }
        }
        Expr::Field { object, field, .. } => {
            // Calculer l'offset du champ
            let class_name = match object.as_ref() {
                Expr::Ident(name, _) => builder.var_class.get(name.as_str()).cloned(),
                Expr::SelfExpr(_)    => builder.current_class.clone(),
                // Accès chaîné (`w.inner.x = ...`) — voir
                // resolve_chained_field_class pour le bug historique corrigé.
                Expr::Field { object: inner, field: inner_field, .. } => {
                    resolve_chained_field_class(builder, inner, inner_field)
                }
                // `use Classe(...).champ = valeur` — voir la doc du même cas
                // dans `lower.rs` (bloc `Expr::Call`) pour le bug corrigé.
                Expr::New { class, .. } => Some(class.clone()),
                _ => None,
            };
            let offset = class_name.as_deref()
                .map(|cls| field_offset(&builder.module.class_layouts, cls, field))
                .unwrap_or(0);
            let obj_val = lower_expr(builder, object);
            let field_ty = class_name.as_deref().and_then(|cls| declared_field_type(builder, cls, field));
            let counted = field_ty.as_ref().is_some_and(|ty| crate::lower::stmt::rc::counted(builder, ty));
            let (val, fresh) = match &field_ty {
                Some(Type::Mixed) => {
                    let boxed = box_for_any(builder, &IrType::Ptr, val_ty, val.clone());
                    let fresh = boxed != val;
                    (boxed, fresh)
                }
                _ => (val, false),
            };
            let old = counted.then(|| {
                let old = builder.new_value();
                builder.emit(Inst::GetField { dest: old.clone(), obj: obj_val.clone(), field: field.clone(), ty: IrType::Ptr, offset });
                if !fresh {
                    crate::lower::stmt::rc::take(builder, &val);
                }
                if let Some(ty) = &field_ty {
                    crate::lower::stmt::rc::mark_raw_if_primitive(builder, ty, &val);
                }
                old
            });
            builder.emit(Inst::SetField {
                obj:   obj_val,
                field: field.clone(),
                src:   val,
                offset,
            });
            if let Some(old) = old {
                crate::lower::stmt::rc::release(builder, &old);
            }
        }
        Expr::Index { object, index, .. } => {
            let obj_val = lower_expr(builder, object);
            let idx_val = lower_expr(builder, index);
            // Même détection map-vs-array que la lecture (Expr::Index dans
            // lower.rs, et lower_incdec ci-dessous) — sans elle, `map[clé] = v`
            // (variable OU self.champ) appelait toujours __array_set, qui
            // réinterprète le pointeur de map comme un tableau et corrompt sa
            // structure interne (crash au premier accès/itération suivant,
            // silencieux avant ça). Factorisé dans `is_map_target` (helpers.rs).
            let is_map = is_map_target(builder, object);
            let func = if is_map { "__map_set" } else { "__array_set" };
            // Boxer `val` si l'ÉLÉMENT du conteneur est `mixed` (ou tout type
            // qui se représente en `Ptr` auto-décrit — `mixed`/union/objet,
            // voir IrType::from_ast) — même logique que `box_for_any` déjà
            // appliquée à l'affectation d'une variable simple (`Expr::Ident`
            // ci-dessus), jamais appliquée ici avant ce correctif.
            //
            // Bug historique corrigé : `m["clé"] = 3.14` où `m:map<string,
            // mixed>` stockait la valeur F64 BRUTE (jamais boxée par
            // `__box_float`) dans le slot `mixed` (i64) de la map — un
            // bit-pattern IEEE-754 réinterprété comme un pointeur par tout
            // consommateur `mixed` générique (`var c:float = m["clé"]`,
            // `__mixed_to_float`), déréférencé à une adresse arbitraire :
            // SIGSEGV. Un `bool`/`int` assez grand pour être ambigu avec un
            // pointeur logé de la même façon dans un conteneur `mixed`
            // partageait exactement le même défaut (jamais boxé). Type
            // élément résolu via `elem_type_after_index` (même résolution
            // statique — Ident/Field chaîné/Index chaîné — que la lecture,
            // voir docs/roadmap.d/langage-index-chaine-sur-map.md) ; si le
            // type élément ne peut pas être résolu statiquement (conteneur
            // non couvert, ex. `getMap()[clé] = v`), `val` reste inchangé,
            // comportement identique à avant ce correctif — jamais de
            // régression sur un cas déjà correct (un conteneur CONCRET, dont
            // `val` a déjà le bon type IR, n'a de toute façon jamais besoin
            // de boxing : `box_for_any` est un no-op dès que `val_ty` est
            // déjà celui attendu). Voir
            // docs/roadmap.d/langage-mixed-container-indexed-assignment-boxing.md.
            let val = match elem_type_after_index(builder, object) {
                Some(elem_ast_ty) => {
                    let elem_ir_ty = IrType::from_ast(&elem_ast_ty);
                    box_for_any(builder, &elem_ir_ty, val_ty, val)
                }
                None => val,
            };
            builder.emit(Inst::Call {
                dest:   None,
                func:   func.into(),
                args:   vec![obj_val, idx_val, val],
                ret_ty: IrType::Void,
            });
        }
        _ => {
            // cible invalide — ignorée silencieusement (sema a déjà rapporté l'erreur)
        }
    }
}

/// Lowering de `i++`/`++i`/`i--`/`--i` (`Expr::IncDec`, voir
/// docs/roadmap.d/langage-increment-decrement.md) : charge l'ancienne valeur
/// de `target`, calcule `ancienne ± 1`, la stocke, et retourne l'ancienne
/// valeur (forme suffixe) ou la nouvelle (forme préfixe).
///
/// `object`/`index` ne sont JAMAIS évalués plus d'une fois — `arr[calc()]++`
/// n'appelle `calc()` qu'une seule fois (la `Value` obtenue est réutilisée
/// pour la lecture ET l'écriture), contrairement à un enchaînement naïf
/// "lecture puis `lower_assign`" qui réévaluerait `object`/`index`.
///
/// Sema (`src/sema/typecheck.rs`, cas `Expr::IncDec`) a déjà validé que
/// `target` est un `Ident`/`Field`/`Index` de type `int`/`float` — jamais
/// `mixed`/`scoped`/`consumed` (aucun boxing ni libération à faire ici,
/// contrairement à `lower_assign`).
pub fn lower_incdec(builder: &mut LowerBuilder, op: &IncDecOp, target: &Expr) -> Value {
    match target {
        Expr::Ident(name, _) => {
            let (old_val, ty) = builder.load_local(name).unwrap_or_else(|| {
                let d = builder.new_value();
                builder.emit(Inst::Nop);
                (d, IrType::I64)
            });
            let new_val = emit_incdec_step(builder, op, old_val.clone(), &ty);
            builder.store_local(name, new_val.clone());
            if op.is_prefix() { new_val } else { old_val }
        }
        Expr::Field { object, field, .. } => {
            let class_name = match object.as_ref() {
                Expr::Ident(name, _) => builder.var_class.get(name.as_str()).cloned(),
                Expr::SelfExpr(_)    => builder.current_class.clone(),
                Expr::Field { object: inner, field: inner_field, .. } => {
                    resolve_chained_field_class(builder, inner, inner_field)
                }
                Expr::New { class, .. } => Some(class.clone()),
                _ => None,
            };
            let (offset, ty) = match &class_name {
                Some(cls) => (
                    field_offset(&builder.module.class_layouts, cls, field),
                    field_ir_type(&builder.module.class_layouts, cls, field),
                ),
                None => (0, IrType::I64),
            };
            let obj_val = lower_expr(builder, object);
            let old_val = builder.new_value();
            builder.emit(Inst::GetField {
                dest: old_val.clone(), obj: obj_val.clone(), field: field.clone(), ty: ty.clone(), offset,
            });
            let new_val = emit_incdec_step(builder, op, old_val.clone(), &ty);
            builder.emit(Inst::SetField { obj: obj_val, field: field.clone(), src: new_val.clone(), offset });
            if op.is_prefix() { new_val } else { old_val }
        }
        Expr::Index { object, index, .. } => {
            let obj_val = lower_expr(builder, object);
            let idx_val = lower_expr(builder, index);
            let ty = expr_ir_type_pub(builder, target);
            let (get_func, set_func) = if is_map_target(builder, object) {
                ("__map_get", "__map_set")
            } else {
                ("__array_get", "__array_set")
            };
            let old_val = builder.new_value();
            builder.emit(Inst::Call {
                dest: Some(old_val.clone()), func: get_func.into(),
                args: vec![obj_val.clone(), idx_val.clone()], ret_ty: IrType::Ptr,
            });
            let new_val = emit_incdec_step(builder, op, old_val.clone(), &ty);
            builder.emit(Inst::Call {
                dest: None, func: set_func.into(),
                args: vec![obj_val, idx_val, new_val.clone()], ret_ty: IrType::Void,
            });
            if op.is_prefix() { new_val } else { old_val }
        }
        _ => unreachable!("sema a déjà rejeté toute autre forme de cible pour Expr::IncDec"),
    }
}

/// Calcule `old_val ± 1` — `ty` est `I64` ou `F64` (jamais autre chose, sema
/// a déjà validé que la cible d'un `Expr::IncDec` est `int`/`float`).
fn emit_incdec_step(builder: &mut LowerBuilder, op: &IncDecOp, old_val: Value, ty: &IrType) -> Value {
    let one = builder.new_value();
    match ty {
        IrType::F64 => builder.emit(Inst::ConstFloat { dest: one.clone(), value: 1.0 }),
        _           => builder.emit(Inst::ConstInt   { dest: one.clone(), value: 1   }),
    }
    let new_val = builder.new_value();
    let inst = if op.is_increment() {
        Inst::Add { dest: new_val.clone(), lhs: old_val, rhs: one, ty: ty.clone() }
    } else {
        Inst::Sub { dest: new_val.clone(), lhs: old_val, rhs: one, ty: ty.clone() }
    };
    builder.emit(inst);
    new_val
}

/// Tests unitaires — boxing de `val` dans `lower_assign` (`Expr::Index`)
/// quand l'ÉLÉMENT du conteneur cible est `mixed` (`m["clé"] = 3.14`, où
/// `m:map<string,mixed>`). Voir
/// docs/roadmap.d/langage-mixed-container-indexed-assignment-boxing.md :
/// avant ce correctif, AUCUN boxing n'était appliqué sur ce chemin — un
/// float/bool/int assigné par indexation dans un conteneur `mixed` stockait
/// sa représentation IR brute (bits F64 pour un float, notamment) dans le
/// slot `mixed` (i64) du conteneur, plus tard déréférencée comme un pointeur
/// par tout consommateur `mixed` générique — SIGSEGV pour un float, valeur
/// fausse silencieuse pour un `int`/`bool` de petite magnitude.
///
/// Ces tests vérifient la DÉCISION DE LOWERING (quel(s) appel(s) de boxing
/// sont émis avant `__map_set`/`__array_set`), pas l'exécution réelle — même
/// esprit que `src/lower/expr.d/tests.rs` (inspection de l'état du
/// `LowerBuilder`/des instructions émises, sans exécuter de programme).
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower::builder::LowerBuilder;
    use crate::ir::module::IrModule;
    use crate::parsing::ast::{Literal, Type};
    use crate::parsing::token::Span;

    fn span() -> Span { Span::new(0, 0) }
    fn ident(name: &str) -> Expr { Expr::Ident(name.to_string(), span()) }
    fn index(object: Expr, idx: Expr) -> Expr {
        Expr::Index { object: Box::new(object), index: Box::new(idx), span: span() }
    }
    fn str_lit(s: &str) -> Expr { Expr::Literal(Literal::String(s.to_string()), span()) }
    fn float_lit(f: f64) -> Expr { Expr::Literal(Literal::Float(f), span()) }
    fn bool_lit(b: bool) -> Expr { Expr::Literal(Literal::Bool(b), span()) }
    fn int_lit(n: i64) -> Expr { Expr::Literal(Literal::Int(n), span()) }

    /// Nombre d'instructions `Call` vers `func_name`, tous blocs confondus
    /// (ces tests ne créent jamais qu'un seul bloc, mais itérer sur tous
    /// reste correct/robuste si ça changeait).
    fn call_count(builder: &LowerBuilder, func_name: &str) -> usize {
        builder.func.blocks.iter()
            .flat_map(|b| b.insts.iter())
            .filter(|inst| matches!(inst, Inst::Call { func, .. } if func == func_name))
            .count()
    }

    #[test]
    fn map_mixed_float_assignment_boxes_before_map_set() {
        let mut module = IrModule::new("test");
        let mut builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);
        builder.map_vars.insert("m".to_string());
        builder.elem_ast_types.insert("m".to_string(), Type::Mixed);

        lower_assign(&mut builder, &index(ident("m"), str_lit("c")), &float_lit(3.14));

        assert_eq!(call_count(&builder, "__box_float"), 1, "un float assigné dans map<string,mixed> doit être boxé");
        assert_eq!(call_count(&builder, "__map_set"), 1);
    }

    #[test]
    fn map_mixed_bool_assignment_boxes_before_map_set() {
        let mut module = IrModule::new("test");
        let mut builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);
        builder.map_vars.insert("m".to_string());
        builder.elem_ast_types.insert("m".to_string(), Type::Mixed);

        lower_assign(&mut builder, &index(ident("m"), str_lit("d")), &bool_lit(true));

        assert_eq!(call_count(&builder, "__box_bool"), 1, "un bool assigné dans map<string,mixed> doit être boxé");
        assert_eq!(call_count(&builder, "__map_set"), 1);
    }

    /// Un `int` littéral assigné dans un conteneur `mixed` doit TOUJOURS
    /// passer par `__box_int_for_mixed` — la décision de magnitude
    /// (`box_int_if_needed`) est prise au RUNTIME, pas ici (voir la doc de
    /// `box_for_any`) : même un petit entier émet cet appel, qui décidera
    /// lui-même de rester brut ou non.
    #[test]
    fn map_mixed_int_assignment_boxes_before_map_set() {
        let mut module = IrModule::new("test");
        let mut builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);
        builder.map_vars.insert("m".to_string());
        builder.elem_ast_types.insert("m".to_string(), Type::Mixed);

        lower_assign(&mut builder, &index(ident("m"), str_lit("stamp")), &int_lit(1_790_255_242));

        assert_eq!(call_count(&builder, "__box_int_for_mixed"), 1);
        assert_eq!(call_count(&builder, "__map_set"), 1);
    }

    /// Même correctif, conteneur `array<mixed>` plutôt que `map<string,mixed>`
    /// — `is_map_target` doit rester `false` (route vers `__array_set`), le
    /// boxing doit s'appliquer identiquement aux deux formes de conteneur.
    #[test]
    fn array_mixed_float_assignment_boxes_before_array_set() {
        let mut module = IrModule::new("test");
        let mut builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);
        builder.elem_ast_types.insert("arr".to_string(), Type::Mixed);

        lower_assign(&mut builder, &index(ident("arr"), int_lit(0)), &float_lit(2.5));

        assert_eq!(call_count(&builder, "__box_float"), 1);
        assert_eq!(call_count(&builder, "__array_set"), 1);
        assert_eq!(call_count(&builder, "__map_set"), 0, "un array ne doit jamais dispatcher vers __map_set");
    }

    /// Non-régression : un conteneur CONCRET (`array<float>`, jamais
    /// `mixed`) ne doit JAMAIS boxer — `val` a déjà le bon type IR (F64) pour
    /// `__array_set`, un boxing ici corromprait le tableau concret.
    #[test]
    fn array_concrete_float_assignment_never_boxes() {
        let mut module = IrModule::new("test");
        let mut builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);
        builder.elem_ast_types.insert("arr".to_string(), Type::Float);

        lower_assign(&mut builder, &index(ident("arr"), int_lit(0)), &float_lit(2.5));

        assert_eq!(call_count(&builder, "__box_float"), 0, "array<float> concret ne doit jamais boxer");
        assert_eq!(call_count(&builder, "__array_set"), 1);
    }

    /// Non-régression : type élément inconnu (`elem_type_after_index`
    /// retourne `None`, ex. conteneur jamais enregistré dans
    /// `elem_ast_types`) — comportement identique à avant ce correctif,
    /// aucun boxing, `val` transmis inchangé.
    #[test]
    fn unresolvable_container_element_type_never_boxes() {
        let mut module = IrModule::new("test");
        let mut builder = LowerBuilder::new(&mut module, "test_fn".into(), vec![], IrType::Void);
        // Volontairement AUCUNE entrée dans elem_ast_types pour "unknown".

        lower_assign(&mut builder, &index(ident("unknown"), int_lit(0)), &float_lit(2.5));

        assert_eq!(call_count(&builder, "__box_float"), 0);
        assert_eq!(call_count(&builder, "__array_set"), 1);
    }
}

/// Type conteneur (`array<T>`/`map<K,V>`) déclaré de la cible d'une
/// affectation, quand il est connu : variable locale (`elem_ast_types`/
/// `map_vars`), champ (`class_field_types`), élément indexé.
fn declared_container_type(builder: &LowerBuilder, target: &Expr) -> Option<Type> {
    match target {
        Expr::Ident(name, _) => {
            let elem = builder.elem_ast_types.get(name.as_str())?.clone();
            Some(if builder.map_vars.contains(name.as_str()) {
                Type::Map(Box::new(Type::String), Box::new(elem))
            } else {
                Type::Array(Box::new(elem))
            })
        }
        Expr::Field { object, field, .. } => {
            let class_name = crate::lower::expr::helpers::resolve_receiver_class(builder, object)?;
            builder.module.class_field_types.get(&class_name)?
                .iter().find(|(f, _)| f == field)
                .map(|(_, ty)| ty.clone())
        }
        Expr::Index { object, .. } => elem_type_after_index(builder, object),
        _ => None,
    }
}

/// Type déclaré du champ `field` de `class` (champs hérités compris).
fn declared_field_type(builder: &LowerBuilder, class: &str, field: &str) -> Option<Type> {
    builder.module.class_field_types.get(class)?
        .iter().find(|(f, _)| f == field)
        .map(|(_, ty)| ty.clone())
}
