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
    pub fn from_ast(ty: &crate::ast::Type) -> Self {
        use crate::ast::Type;
        match ty {
            Type::Int              => IrType::I64,
            Type::Float            => IrType::F64,
            Type::Bool             => IrType::Bool,
            Type::Void             => IrType::Void,
            Type::String           => IrType::Ptr,
            Type::Mixed              => IrType::Ptr,
            Type::Null             => IrType::Ptr,
            Type::Named(_)         => IrType::Ptr,
            Type::Qualified(_)     => IrType::Ptr,
            Type::Array(_)         => IrType::Ptr,
            Type::Map(_, _)        => IrType::Ptr,
            Type::Union(_)         => IrType::Ptr,
            Type::Function { .. }  => IrType::Ptr,
        }
    }
}
