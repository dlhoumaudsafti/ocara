/// Helpers pour le lowering des statements

use crate::ir::inst::{Inst, Value};
use crate::ir::types::IrType;
use crate::lower::builder::LowerBuilder;

/// Si la variable cible est de type `mixed` (Ptr) et la valeur est F64 ou Bool,
/// on la boxe pour éviter que les bits soient interprétés comme un pointeur.
///
/// Sens inverse : la cible est CONCRÈTE (int/float/bool) mais `val_ty` est
/// `Ptr` — ne peut arriver que si `val` provient en réalité d'un `mixed`
/// (jamais d'un vrai `string`/`array`/`map`/objet, dont l'affectation à une
/// cible concrète est refusée par la sema) : un `float`/`bool` boxé, ou un
/// `int` déjà brut. Sans déballage, le bit pattern du pointeur boxé était
/// stocké tel quel dans la cible concrète — résultat faux, confirmé par
/// reproduction (`var m:mixed = 3.5; var f:float = m` ; voir
/// docs/roadmap.d/langage-mixed-arithmetic.md, découvert en corrigeant
/// l'arithmétique `mixed` mais pas spécifique à un opérateur).
pub fn box_for_any(builder: &mut LowerBuilder, target_ty: &IrType, val_ty: IrType, val: Value) -> Value {
    if *target_ty != IrType::Ptr {
        if val_ty != IrType::Ptr {
            return val;
        }
        let (func, ret_ty) = match target_ty {
            IrType::F64 => ("__mixed_to_float", IrType::F64),
            _           => ("__mixed_to_int",   IrType::I64),
        };
        let d = builder.new_value();
        builder.emit(Inst::Call { dest: Some(d.clone()), func: func.into(), args: vec![val], ret_ty });
        return d;
    }
    match val_ty {
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
        // Un `int` connu STATIQUEMENT logé dans un `mixed` : boxer si assez
        // grand pour être ambigu avec un pointeur heap (voir
        // `box_int_if_needed`/`__box_int_for_mixed`, runtime/src/lib.rs — la
        // décision magnitude est prise au runtime, pas ici, pour ne pas
        // pénaliser le cas courant d'un petit entier). Corrige le SEGFAULT
        // documenté dans docs/roadmap.d/memoire-fiabilite-runtime-bas-niveau.md
        // (`var n:mixed = 1000000; if n is string {...}`).
        //
        // ⚠ Ce bras dépend ENTIÈREMENT de la fiabilité de `val_ty` : si
        // `expr_ir_type` rapporte I64 par erreur pour une expression qui est
        // en réalité un pointeur objet/tableau (ex. `SQLite::open(...)`,
        // absent de la table `fn_ret_types`, voir program.rs), ce pointeur
        // serait boxé comme si c'était un entier — corruption confirmée par
        // reproduction (`SQLite::open` mal classé → `db.execute()` bloqué
        // dans une boucle infinie sur le self-pointer corrompu). Voir le
        // filet de sécurité dans `expr_ir_type` (`Expr::StaticCall`, dernier
        // bras `else`) : par prudence, il retombe désormais sur `Ptr` (jamais
        // boxé, comportement identique à avant ce correctif) plutôt que I64
        // dès qu'il ne peut pas prouver le vrai type de retour.
        IrType::I64 => {
            let d = builder.new_value();
            builder.emit(Inst::Call { dest: Some(d.clone()), func: "__box_int_for_mixed".into(), args: vec![val], ret_ty: IrType::Ptr });
            d
        }
        _ => val,
    }
}
