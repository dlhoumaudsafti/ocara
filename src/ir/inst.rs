use crate::ir::types::IrType;

// ─────────────────────────────────────────────────────────────────────────────
// Valeur SSA — référence à un résultat d'instruction
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Value(pub u32);

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "v{}", self.0)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Bloc de base
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockId(pub u32);

impl std::fmt::Display for BlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "bb{}", self.0)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Instructions HIR
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Inst {
    // ── Constantes ────────────────────────────────────────────────────────────
    ConstInt   { dest: Value, value: i64 },
    ConstFloat { dest: Value, value: f64 },
    ConstBool  { dest: Value, value: bool },
    /// Pointeur vers une chaîne littérale dans la table des constantes
    ConstStr   { dest: Value, idx: u32 },

    // ── Arithmétique / logique ─────────────────────────────────────────────────
    Add  { dest: Value, lhs: Value, rhs: Value, ty: IrType },
    Sub  { dest: Value, lhs: Value, rhs: Value, ty: IrType },
    Mul  { dest: Value, lhs: Value, rhs: Value, ty: IrType },
    Div  { dest: Value, lhs: Value, rhs: Value, ty: IrType },
    Mod  { dest: Value, lhs: Value, rhs: Value, ty: IrType },
    Neg  { dest: Value, src: Value, ty: IrType },

    // ── Comparaison (résultat toujours Bool) ───────────────────────────────────
    CmpEq  { dest: Value, lhs: Value, rhs: Value, ty: IrType },
    CmpNe  { dest: Value, lhs: Value, rhs: Value, ty: IrType },
    CmpLt  { dest: Value, lhs: Value, rhs: Value, ty: IrType },
    CmpLe  { dest: Value, lhs: Value, rhs: Value, ty: IrType },
    CmpGt  { dest: Value, lhs: Value, rhs: Value, ty: IrType },
    CmpGe  { dest: Value, lhs: Value, rhs: Value, ty: IrType },    
    // ── Logique booléenne ──────────────────────────────────────────────────────
    And  { dest: Value, lhs: Value, rhs: Value },
    Or   { dest: Value, lhs: Value, rhs: Value },
    Not  { dest: Value, src: Value },

    // ── Variables locales (pile) ───────────────────────────────────────────────
    Alloca  { dest: Value, ty: IrType },              // alloue un slot
    Store   { ptr: Value, src: Value },               // écrit dans un slot
    Load    { dest: Value, ptr: Value, ty: IrType },  // lit depuis un slot

    // ── Appel de fonction ──────────────────────────────────────────────────────
    Call {
        dest:  Option<Value>,
        func:  String,
        args:  Vec<Value>,
        ret_ty: IrType,
    },

    // ── Appel indirect (méthode virtuelle) ─────────────────────────────────────
    CallIndirect {
        dest:     Option<Value>,
        callee:   Value,
        args:     Vec<Value>,
        ret_ty:   IrType,
    },

    // ── Flux de contrôle ──────────────────────────────────────────────────────
    Jump    { target: BlockId },
    Branch  { cond: Value, then_bb: BlockId, else_bb: BlockId },
    Return  { value: Option<Value> },

    // ── Phi (SSA) — utilisé après lowering de branchements ────────────────────
    Phi     { dest: Value, ty: IrType, sources: Vec<(Value, BlockId)> },

    // ── Gestion de pile d'objet (GC-free, ownership Rust-style) ───────────────
    /// Allocation d'objet sur le tas (retourne un Ptr)
    Alloc   { dest: Value, class: String },
    /// Écriture d'un champ (offset en bytes depuis le pointeur d'objet)
    SetField { obj: Value, field: String, src: Value, offset: i32 },
    /// Lecture d'un champ (offset en bytes depuis le pointeur d'objet)
    GetField { dest: Value, obj: Value, field: String, ty: IrType, offset: i32 },

    /// Instruction vide / no-op (sert de marqueur)
    Nop,

    /// Adresse d'une fonction nommée (pour passer en pointeur de fonction)
    FuncAddr { dest: Value, func: String },
}
