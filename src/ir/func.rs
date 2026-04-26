use crate::ir::inst::{BlockId, Inst, Value};
use crate::ir::types::IrType;

// ─────────────────────────────────────────────────────────────────────────────
// BasicBlock
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub id:    BlockId,
    pub insts: Vec<Inst>,
}

impl BasicBlock {
    pub fn new(id: BlockId) -> Self {
        Self { id, insts: Vec::new() }
    }

    pub fn push(&mut self, inst: Inst) {
        self.insts.push(inst);
    }

    /// Vérifie si le bloc est terminé (dernière instruction = terminateur)
    pub fn is_terminated(&self) -> bool {
        matches!(
            self.insts.last(),
            Some(Inst::Jump { .. } | Inst::Branch { .. } | Inst::Return { .. })
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// IrParam
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct IrParam {
    pub name: String,
    pub ty:   IrType,
    pub slot: Value, // valeur SSA associée au paramètre
}

// ─────────────────────────────────────────────────────────────────────────────
// IrFunction
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct IrFunction {
    pub name:    String,
    pub params:  Vec<IrParam>,
    pub ret_ty:  IrType,
    pub blocks:  Vec<BasicBlock>,
    /// Compteur pour générer des valeurs SSA uniques
    next_value:  u32,
    /// Compteur pour générer des blocs de base uniques
    next_block:  u32,
    /// Index du bloc courant
    current_bb:  usize,
}

impl IrFunction {
    pub fn new(name: String, params: Vec<IrParam>, ret_ty: IrType) -> Self {
        let entry = BasicBlock::new(BlockId(0));
        Self {
            name,
            params,
            ret_ty,
            blocks: vec![entry],
            next_value: 0,
            next_block: 1,
            current_bb: 0,
        }
    }

    // ── Génération de valeurs / blocs ─────────────────────────────────────────

    pub fn new_value(&mut self) -> Value {
        let v = Value(self.next_value);
        self.next_value += 1;
        v
    }

    pub fn new_block(&mut self) -> BlockId {
        let id = BlockId(self.next_block);
        self.next_block += 1;
        self.blocks.push(BasicBlock::new(id.clone()));
        id
    }

    /// Passe au bloc dont l'id est fourni
    pub fn switch_to(&mut self, id: &BlockId) {
        self.current_bb = self.blocks.iter().position(|b| &b.id == id)
            .expect("BasicBlock introuvable");
    }

    // ── Émission d'instructions ───────────────────────────────────────────────

    pub fn emit(&mut self, inst: Inst) {
        self.blocks[self.current_bb].push(inst);
    }

    pub fn is_current_terminated(&self) -> bool {
        self.blocks[self.current_bb].is_terminated()
    }
}
