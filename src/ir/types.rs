// ─────────────────────────────────────────────────────────────────────────────
// Types HIR — miroir des types Ocara mais aplatis pour le codegen
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum IrType {
    I64,
    F64,
    Bool,
    Ptr,   // pointeur opaque (string, objets)
    Void,
}

impl IrType {
    pub fn from_ast(ty: &crate::parsing::ast::Type) -> Self {
        use crate::parsing::ast::Type;
        match ty {
            Type::Int              => IrType::I64,
            Type::Float            => IrType::F64,
            Type::Bool             => IrType::Bool,
            Type::Void             => IrType::Void,
            Type::String           => IrType::Ptr,
            Type::Mixed            => IrType::Ptr,
            Type::Null             => IrType::Ptr,
            Type::Named(_)         => IrType::Ptr,
            Type::Qualified(_)     => IrType::Ptr,
            Type::Array(_)         => IrType::Ptr,
            Type::Map(_, _)        => IrType::Ptr,
            // Pointeur vers le frame (état + locales) du générateur — voir
            // docs/roadmap.d/langage-emit-iterable.md. Valable uniquement
            // comme type de retour d'une fonction contenant `emit` (vérifié
            // par la sema) : ce cas est donc le seul jamais rencontré ici.
            Type::Message(_)       => IrType::Ptr,
            // `Resolvable<T>` (voir
            // docs/roadmap.d/langage-async-non-int-return-type-check.md) est
            // un type PUREMENT sémantique : `T` n'existe qu'au niveau du
            // typechecker, jamais dans la représentation mémoire runtime — le
            // handle reste exactement ce qu'il est aujourd'hui (l'entier
            // opaque transportant le pointeur `OcaraTask`), donc `I64` comme
            // `Type::Int`, jamais `Ptr` (pas de vtable, pas de classe
            // backing, contrairement à `Type::Generic` ci-dessous).
            Type::Resolvable(_)    => IrType::I64,
            Type::Generic { .. }   => IrType::Ptr,  // Générique monomorphisé = objet
            Type::Union(_)         => IrType::Ptr,
            Type::Function { .. }  => IrType::Ptr,
        }
    }
}
