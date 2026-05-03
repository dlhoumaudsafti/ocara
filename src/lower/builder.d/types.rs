/// Types et structures du builder

use std::collections::{HashMap, HashSet};
use crate::parsing::ast::*;
use crate::ir::func::{IrFunction, IrParam};
use crate::ir::inst::{BlockId, Inst, Value};
use crate::ir::module::IrModule;
use crate::ir::types::IrType;

// ─────────────────────────────────────────────────────────────────────────────
// LowerBuilder — contexte de lowering pour une fonction
// ─────────────────────────────────────────────────────────────────────────────

pub struct LowerBuilder<'m> {
    pub module: &'m mut IrModule,
    pub func:   IrFunction,
    /// Mapping nom local → slot Alloca (Value)
    pub locals: HashMap<String, (Value, IrType, bool)>, // (slot, type, mutable)
    /// Types de retour des fonctions utilisateur (pour le dispatch IO::writeln)
    pub fn_ret_types: HashMap<String, IrType>,
    /// Types des paramètres des fonctions (pour la génération de wrappers Function)
    pub fn_param_types: HashMap<String, Vec<IrType>>,
    /// Infos variadic : (fixed_params_count, array_elem_type)
    pub fn_variadic_info: HashMap<String, (usize, IrType)>,
    /// Type des éléments pour les variables tableau (ex: jours:string[] → Ptr)
    pub elem_types: HashMap<String, IrType>,
    /// Noms des variables déclarées comme map<K,V> (pour Expr::Index → __map_get)
    pub map_vars: HashSet<String>,
    /// Mapping nom_variable → nom_classe (pour résoudre les appels de méthode)
    pub var_class: HashMap<String, String>,
    /// Classe courante (Some(name) si on est dans une méthode/constructeur)
    pub current_class: Option<String>,
    /// Pile de boucles : (continue_bb, break_bb) — pour break/continue
    pub loop_stack: Vec<(BlockId, BlockId)>,
    /// Variables de type Function (pointeurs de fonction) — pour CallIndirect
    pub func_vars: HashSet<String>,
    /// Type de retour des variables Function<ReturnType> — pour CallIndirect
    pub func_ret_types: HashMap<String, IrType>,
    /// Variables capturées par une closure — accès via GetField/SetField sur __env
    /// (env_ptr: Value, index: usize, type: IrType)
    pub captured_vars: HashMap<String, (Value, usize, IrType)>,
    /// Variables locales déjà promues sur le tas pour le partage avec des closures.
    /// Après promotion, `locals[name]` est un heap pointer (pas une Alloca stack).
    pub heap_promoted: HashSet<String>,
    /// Noms des fonctions marquées `async` (au sens Ocara : spawn thread)
    pub async_funcs: HashSet<String>,
    /// Mapping var_name → IrType de retour original de la fonction async
    /// Utilisé par Expr::Resolve pour savoir quel unboxing appliquer.
    pub async_var_ret: HashMap<String, IrType>,
    /// Noms des paramètres variadic (pour unboxing dans les boucles for)
    pub variadic_params: HashSet<String>,
    /// Valeurs par défaut des paramètres : func_name → Vec<Option<Expr>>
    pub func_default_args: HashMap<String, Vec<Option<Expr>>>,
    /// Nombre total de paramètres pour les variables Function : var_name → count
    pub func_var_param_count: HashMap<String, usize>,
}

impl<'m> LowerBuilder<'m> {
    pub fn new(
        module: &'m mut IrModule,
        name: String,
        params: Vec<IrParam>,
        ret_ty: IrType,
    ) -> Self {
        let func = IrFunction::new(name, params, ret_ty);
        Self {
            module,
            func,
            locals: HashMap::new(),
            fn_ret_types: HashMap::new(),
            fn_param_types: HashMap::new(),
            fn_variadic_info: HashMap::new(),
            elem_types: HashMap::new(),
            map_vars: HashSet::new(),
            var_class: HashMap::new(),
            current_class: None,
            loop_stack: Vec::new(),
            func_vars: HashSet::new(),
            func_ret_types: HashMap::new(),
            captured_vars: HashMap::new(),
            heap_promoted: HashSet::new(),
            async_funcs: HashSet::new(),
            async_var_ret: HashMap::new(),
            variadic_params: HashSet::new(),
            func_default_args: HashMap::new(),
            func_var_param_count: HashMap::new(),
        }
    }

    // ── Wrappers ─────────────────────────────────────────────────────────────

    pub fn new_value(&mut self) -> Value {
        self.func.new_value()
    }

    pub fn new_block(&mut self) -> BlockId {
        self.func.new_block()
    }

    pub fn switch_to(&mut self, id: &BlockId) {
        self.func.switch_to(id);
    }

    pub fn emit(&mut self, inst: Inst) {
        self.func.emit(inst);
    }

    pub fn is_terminated(&self) -> bool {
        self.func.is_current_terminated()
    }

    // ── Déclaration d'une variable locale (avec Alloca) ───────────────────────

    pub fn declare_local(&mut self, name: &str, ty: IrType, mutable: bool) -> Value {
        let slot = self.new_value();
        self.emit(Inst::Alloca { dest: slot.clone(), ty: ty.clone() });
        self.locals.insert(name.to_string(), (slot.clone(), ty, mutable));
        slot
    }

    /// Retourne le slot (stack ou heap pointer) d'un local sans émettre de Load.
    pub fn slot_of_local(&self, name: &str) -> Option<Value> {
        self.locals.get(name).map(|(slot, _, _)| slot.clone())
    }

    /// Stocke `src` dans le slot du local `name`.
    /// Pour les variables capturées, écrit via double-indirection :
    /// GetField(env, idx) → heap_ptr, puis Store(heap_ptr, src).
    pub fn store_local(&mut self, name: &str, src: Value) {
        // Variable capturée → double-indirection via l'env struct (heap pointer)
        if let Some((env_val, idx, _)) = self.captured_vars.get(name).cloned() {
            let ptr = self.new_value();
            self.emit(Inst::GetField {
                dest:   ptr.clone(),
                obj:    env_val,
                field:  format!("__cap_{}", idx),
                ty:     IrType::Ptr,
                offset: (idx * 8) as i32,
            });
            self.emit(Inst::Store { ptr, src });
            return;
        }
        if let Some((slot, _, _)) = self.locals.get(name) {
            let slot = slot.clone();
            self.emit(Inst::Store { ptr: slot, src });
        }
    }

    /// Charge le local `name` → retourne (Value résultat, IrType).
    /// Pour les variables capturées, lit via double-indirection :
    /// GetField(env, idx) → heap_ptr, puis Load(heap_ptr) → valeur.
    pub fn load_local(&mut self, name: &str) -> Option<(Value, IrType)> {
        // Variable capturée → double-indirection via l'env struct
        if let Some((env_val, idx, ty)) = self.captured_vars.get(name).cloned() {
            let ptr = self.new_value();
            self.emit(Inst::GetField {
                dest:   ptr.clone(),
                obj:    env_val,
                field:  format!("__cap_{}", idx),
                ty:     IrType::Ptr,
                offset: (idx * 8) as i32,
            });
            let dest = self.new_value();
            self.emit(Inst::Load { dest: dest.clone(), ptr, ty: ty.clone() });
            return Some((dest, ty));
        }
        if let Some((slot, ty, _)) = self.locals.get(name).cloned() {
            let dest = self.new_value();
            self.emit(Inst::Load { dest: dest.clone(), ptr: slot, ty: ty.clone() });
            Some((dest, ty))
        } else {
            None
        }
    }
}
