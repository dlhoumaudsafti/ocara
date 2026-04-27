use std::collections::HashMap;

use cranelift_codegen::ir::{types as clt, AbiParam, Block as CrBlock, InstBuilder, MemFlags,
    Signature, StackSlotData, StackSlotKind, UserFuncName, Value as CrValue};
use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::{Context, settings};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{DataDescription, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};

use crate::ir::func::IrFunction;
use crate::ir::inst::{BlockId, Inst, Value};
use crate::ir::module::IrModule;
use crate::ir::types::IrType;
use crate::codegen::runtime::{builtin_sig, BUILTINS};

// ─────────────────────────────────────────────────────────────────────────────
// Erreur de codegen
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct CodegenError(pub String);

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "codegen error: {}", self.0)
    }
}
impl std::error::Error for CodegenError {}

pub type CgResult<T> = Result<T, CodegenError>;

// ─────────────────────────────────────────────────────────────────────────────
// CraneliftEmitter
// ─────────────────────────────────────────────────────────────────────────────

pub struct CraneliftEmitter {
    module: ObjectModule,
    /// Table des FuncId déclarés (pour les appels forward)
    func_ids: HashMap<String, FuncId>,
    /// Table des chaînes internées
    strings: Vec<String>,
    /// Types des paramètres de chaque fonction (pour les bitcasts F64↔I64)
    param_types: HashMap<String, Vec<cranelift_codegen::ir::Type>>,
    /// Type de retour de chaque fonction (pour les bitcasts F64↔I64)
    ret_types: HashMap<String, cranelift_codegen::ir::Type>,
    /// Layout des classes : class_name → liste ordonnée (field_name, field_type)
    class_layouts: HashMap<String, Vec<(String, cranelift_codegen::ir::Type)>>,
}

impl CraneliftEmitter {
    pub fn new(module_name: &str) -> CgResult<Self> {
        let settings_builder = settings::builder();
        let flags = settings::Flags::new(settings_builder);

        let isa = cranelift_native::builder()
            .map_err(|e| CodegenError(e.to_string()))?
            .finish(flags)
            .map_err(|e| CodegenError(e.to_string()))?;

        let obj_builder = ObjectBuilder::new(
            isa,
            module_name,
            cranelift_module::default_libcall_names(),
        )
        .map_err(|e| CodegenError(e.to_string()))?;

        let module = ObjectModule::new(obj_builder);

        Ok(Self {
            module,
            func_ids: HashMap::new(),
            strings: Vec::new(),
            param_types: HashMap::new(),
            ret_types: HashMap::new(),
            class_layouts: HashMap::new(),
        })
    }

    // ── Pré-déclaration de toutes les fonctions ───────────────────────────────

    fn predeclare_functions(&mut self, ir: &IrModule) -> CgResult<()> {
        let call_conv = self.module.isa().default_call_conv();

        // Builtins runtime — uniquement ceux dont le module est importé (ou internes)
        for desc in BUILTINS {
            let allowed = match desc.module {
                None => true, // interne, toujours disponible
                Some(m) => ir.imports.iter().any(|imp| imp == m),
            };
            if !allowed { continue; }
            let sig = builtin_sig(desc, call_conv);
            let fid = self.module
                .declare_function(desc.name, Linkage::Import, &sig)
                .map_err(|e| CodegenError(e.to_string()))?;
            self.func_ids.insert(desc.name.to_string(), fid);
            // Enregistre les types des paramètres pour les bitcasts
            self.param_types.insert(
                desc.name.to_string(),
                desc.params.to_vec(),
            );
            if let Some(ret) = desc.returns {
                self.ret_types.insert(desc.name.to_string(), ret);
            }
        }

        // Fonctions du module
        for func in &ir.functions {
            let sig = self.ir_func_signature(func);
            let fid = self.module
                .declare_function(&func.name, Linkage::Export, &sig)
                .map_err(|e| CodegenError(e.to_string()))?;
            self.func_ids.insert(func.name.clone(), fid);
            // Enregistre les types des params et du retour pour les bitcasts F64↔I64
            self.param_types.insert(
                func.name.clone(),
                func.params.iter().map(|p| ir_type_to_cl(&p.ty)).collect(),
            );
            if func.ret_ty != IrType::Void {
                self.ret_types.insert(func.name.clone(), ir_type_to_cl(&func.ret_ty));
            }
        }

        Ok(())
    }

    fn ir_func_signature(&self, func: &IrFunction) -> Signature {
        let call_conv = self.module.isa().default_call_conv();
        let mut sig = Signature::new(call_conv);
        for param in &func.params {
            sig.params.push(AbiParam::new(ir_type_to_cl(&param.ty)));
        }
        if func.ret_ty != IrType::Void {
            sig.returns.push(AbiParam::new(ir_type_to_cl(&func.ret_ty)));
        }
        sig
    }

    // ── Données globales (chaînes) ────────────────────────────────────────────

    fn emit_strings(&mut self, strings: &[String]) -> CgResult<Vec<cranelift_module::DataId>> {
        let mut ids = Vec::new();
        for (i, s) in strings.iter().enumerate() {
            let name = format!("__str_{}", i);
            let mut desc = DataDescription::new();
            desc.set_align(8); // align 8 pour garantir bits bas = 000 (invariant boxing)
            // Header de 8 octets : TAG_STRING = 1 (little-endian i64)
            // Suivi des données de la chaîne null-terminated.
            // Le pointeur retourné à Ocara pointe APRÈS ce header.
            let mut bytes: Vec<u8> = vec![1, 0, 0, 0, 0, 0, 0, 0]; // TAG_STRING
            bytes.extend_from_slice(s.as_bytes());
            bytes.push(0); // null-terminated
            desc.define(bytes.into_boxed_slice());
            let data_id = self.module
                .declare_data(&name, Linkage::Local, false, false)
                .map_err(|e| CodegenError(e.to_string()))?;
            self.module
                .define_data(data_id, &desc)
                .map_err(|e| CodegenError(e.to_string()))?;
            ids.push(data_id);
        }
        Ok(ids)
    }

    // ── Compilation d'une fonction ────────────────────────────────────────────

    fn emit_function(&mut self, ir_func: &IrFunction) -> CgResult<()> {
        let fid = *self.func_ids.get(&ir_func.name)
            .ok_or_else(|| CodegenError(format!("fonction non déclarée: {}", ir_func.name)))?;

        let sig = self.ir_func_signature(ir_func);

        let mut ctx = Context::new();
        ctx.func.signature = sig;
        ctx.func.name = UserFuncName::user(0, fid.as_u32());

        let mut fb_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fb_ctx);

        // ── Variables Cranelift (une par valeur HIR) ──────────────────────────
        let max_val = max_value_id(ir_func);
        let vars: Vec<Variable> = (0..=max_val)
            .map(Variable::from_u32)
            .collect();
        for var in &vars {
            builder.declare_var(*var, clt::I64);
        }

        // ── Blocs Cranelift ───────────────────────────────────────────────────
        let mut cl_blocks: HashMap<BlockId, CrBlock> = HashMap::new();
        for bb in &ir_func.blocks {
            let cl_bb = builder.create_block();
            cl_blocks.insert(bb.id.clone(), cl_bb);
        }

        // Paramètres d'entrée → variables
        let entry_bb = *cl_blocks.get(&BlockId(0))
            .ok_or_else(|| CodegenError("entry block manquant".into()))?;
        builder.append_block_params_for_function_params(entry_bb);
        builder.switch_to_block(entry_bb);
        // Ne pas sceller ici — on scelle tout à la fin avec seal_all_blocks()

        let params = builder.block_params(entry_bb).to_vec();
        for (i, param) in params.iter().enumerate() {
            if i < ir_func.params.len() {
                let v = ir_func.params[i].slot.0 as usize;
                if v < vars.len() {
                    // Si le paramètre est f64 (float), le bitcaster en i64 pour
                    // respecter la représentation uniforme des variables
                    let stored_val = if ir_func.params[i].ty == IrType::F64 {
                        builder.ins().bitcast(clt::I64, MemFlags::new(), *param)
                    } else {
                        *param
                    };
                    builder.def_var(vars[v], stored_val);
                }
            }
        }

        // ── Émission des instructions ─────────────────────────────────────────
        let func_ids    = self.func_ids.clone();
        let param_types = self.param_types.clone();
        let ret_types   = self.ret_types.clone();
        let class_layouts = self.class_layouts.clone();
        let func_ret_ty = ir_func.ret_ty.clone();
        let module = &mut self.module;

        for (bb_idx, bb) in ir_func.blocks.iter().enumerate() {
            if bb_idx > 0 {
                let cl_bb = cl_blocks[&bb.id];
                builder.switch_to_block(cl_bb);
                // Pas de seal_block ici — on scelle tout à la fin
            }

            for inst in &bb.insts {
                emit_inst(
                    &mut builder,
                    inst,
                    &vars,
                    &cl_blocks,
                    module,
                    &func_ids,
                    &param_types,
                    &ret_types,
                    &class_layouts,
                    &func_ret_ty,
                )?;
            }
        }

        // Scelle tous les blocs après émission complète (SSA correcte pour les back-edges)
        builder.seal_all_blocks();
        builder.finalize();

        // Vérification + compilation
        let fid = *self.func_ids.get(&ir_func.name).unwrap();
        self.module
            .define_function(fid, &mut ctx)
            .map_err(|e| CodegenError(format!("define_function: {:?}", e)))?;

        Ok(())
    }

    // ── Point d'entrée public ─────────────────────────────────────────────────

    /// Compile `IrModule` → bytes d'un fichier objet ELF/Mach-O/COFF
    pub fn compile(mut self, ir: &IrModule) -> CgResult<Vec<u8>> {
        self.strings = ir.strings.clone();
        self.class_layouts = ir.class_layouts.iter()
            .map(|(k, v)| (k.clone(), v.iter().map(|(f, t)| (f.clone(), ir_type_to_cl(t))).collect()))
            .collect();
        self.predeclare_functions(ir)?;
        self.emit_strings(&ir.strings)?;

        for func in &ir.functions {
            self.emit_function(func)?;
        }

        let product = self.module.finish();
        product
            .emit()
            .map_err(|e| CodegenError(e.to_string()))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Émission d'une instruction Ocara IR → Cranelift IR
// ─────────────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn emit_inst(
    builder:   &mut FunctionBuilder,
    inst:      &Inst,
    vars:      &[Variable],
    cl_blocks:   &HashMap<BlockId, CrBlock>,
    module:      &mut ObjectModule,
    func_ids:    &HashMap<String, FuncId>,
    param_types: &HashMap<String, Vec<cranelift_codegen::ir::Type>>,
    ret_types:   &HashMap<String, cranelift_codegen::ir::Type>,
    class_layouts: &HashMap<String, Vec<(String, cranelift_codegen::ir::Type)>>,
    func_ret_ty: &IrType,
) -> CgResult<()> {
    macro_rules! def {
        ($v:expr, $val:expr) => {
            builder.def_var(vars[$v.0 as usize], $val)
        };
    }
    macro_rules! use_var {
        ($v:expr) => {
            builder.use_var(vars[$v.0 as usize])
        };
    }

    match inst {
        Inst::Nop => {}

        Inst::ConstInt { dest, value } => {
            let v = builder.ins().iconst(clt::I64, *value);
            def!(dest, v);
        }
        Inst::ConstFloat { dest, value } => {
            let v = builder.ins().f64const(*value);
            // store as bitcast i64 for uniform variable representation
            let v64 = builder.ins().bitcast(clt::I64, MemFlags::new(), v);
            def!(dest, v64);
        }
        Inst::ConstBool { dest, value } => {
            let v = builder.ins().iconst(clt::I64, *value as i64);
            def!(dest, v);
        }
        Inst::ConstStr { dest, idx } => {
            // Résolution de l'adresse réelle du symbole de données __str_N
            // Le premier mot (8 octets) est le header TAG_STRING ; les données
            // commencent à l'offset +8. On retourne l'adresse +8.
            let name = format!("__str_{}", idx);
            let data_id = module
                .declare_data(&name, Linkage::Local, false, false)
                .map_err(|e| CodegenError(format!("declare_data({}): {}", name, e)))?;
            let gv  = module.declare_data_in_func(data_id, builder.func);
            let raw = builder.ins().global_value(clt::I64, gv);
            // Sauter le header de 8 octets pour pointer vers les données
            let eight = builder.ins().iconst(clt::I64, 8);
            let ptr   = builder.ins().iadd(raw, eight);
            def!(dest, ptr);
        }

        Inst::Add { dest, lhs, rhs, ty } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = if *ty == IrType::F64 {
                let lf = builder.ins().bitcast(clt::F64, MemFlags::new(), l);
                let rf = builder.ins().bitcast(clt::F64, MemFlags::new(), r);
                let res = builder.ins().fadd(lf, rf);
                builder.ins().bitcast(clt::I64, MemFlags::new(), res)
            } else { builder.ins().iadd(l, r) };
            def!(dest, v);
        }
        Inst::Sub { dest, lhs, rhs, ty } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = if *ty == IrType::F64 {
                let lf = builder.ins().bitcast(clt::F64, MemFlags::new(), l);
                let rf = builder.ins().bitcast(clt::F64, MemFlags::new(), r);
                let res = builder.ins().fsub(lf, rf);
                builder.ins().bitcast(clt::I64, MemFlags::new(), res)
            } else { builder.ins().isub(l, r) };
            def!(dest, v);
        }
        Inst::Mul { dest, lhs, rhs, ty } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = if *ty == IrType::F64 {
                let lf = builder.ins().bitcast(clt::F64, MemFlags::new(), l);
                let rf = builder.ins().bitcast(clt::F64, MemFlags::new(), r);
                let res = builder.ins().fmul(lf, rf);
                builder.ins().bitcast(clt::I64, MemFlags::new(), res)
            } else { builder.ins().imul(l, r) };
            def!(dest, v);
        }
        Inst::Div { dest, lhs, rhs, ty } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = if *ty == IrType::F64 {
                let lf = builder.ins().bitcast(clt::F64, MemFlags::new(), l);
                let rf = builder.ins().bitcast(clt::F64, MemFlags::new(), r);
                let res = builder.ins().fdiv(lf, rf);
                builder.ins().bitcast(clt::I64, MemFlags::new(), res)
            } else { builder.ins().sdiv(l, r) };
            def!(dest, v);
        }
        Inst::Mod { dest, lhs, rhs, .. } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = builder.ins().srem(l, r);
            def!(dest, v);
        }
        Inst::Neg { dest, src, .. } => {
            let s = use_var!(src);
            let v = builder.ins().ineg(s);
            def!(dest, v);
        }

        Inst::CmpEq { dest, lhs, rhs, ty } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = if *ty == IrType::F64 {
                let lf = builder.ins().bitcast(clt::F64, MemFlags::new(), l);
                let rf = builder.ins().bitcast(clt::F64, MemFlags::new(), r);
                builder.ins().fcmp(cranelift_codegen::ir::condcodes::FloatCC::Equal, lf, rf)
            } else { builder.ins().icmp(IntCC::Equal, l, r) };
            let v64 = builder.ins().uextend(clt::I64, v);
            def!(dest, v64);
        }
        Inst::CmpNe { dest, lhs, rhs, ty } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = if *ty == IrType::F64 {
                let lf = builder.ins().bitcast(clt::F64, MemFlags::new(), l);
                let rf = builder.ins().bitcast(clt::F64, MemFlags::new(), r);
                builder.ins().fcmp(cranelift_codegen::ir::condcodes::FloatCC::NotEqual, lf, rf)
            } else { builder.ins().icmp(IntCC::NotEqual, l, r) };
            let v64 = builder.ins().uextend(clt::I64, v);
            def!(dest, v64);
        }
        Inst::CmpLt { dest, lhs, rhs, ty } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = if *ty == IrType::F64 {
                let lf = builder.ins().bitcast(clt::F64, MemFlags::new(), l);
                let rf = builder.ins().bitcast(clt::F64, MemFlags::new(), r);
                builder.ins().fcmp(cranelift_codegen::ir::condcodes::FloatCC::LessThan, lf, rf)
            } else { builder.ins().icmp(IntCC::SignedLessThan, l, r) };
            let v64 = builder.ins().uextend(clt::I64, v);
            def!(dest, v64);
        }
        Inst::CmpLe { dest, lhs, rhs, ty } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = if *ty == IrType::F64 {
                let lf = builder.ins().bitcast(clt::F64, MemFlags::new(), l);
                let rf = builder.ins().bitcast(clt::F64, MemFlags::new(), r);
                builder.ins().fcmp(cranelift_codegen::ir::condcodes::FloatCC::LessThanOrEqual, lf, rf)
            } else { builder.ins().icmp(IntCC::SignedLessThanOrEqual, l, r) };
            let v64 = builder.ins().uextend(clt::I64, v);
            def!(dest, v64);
        }
        Inst::CmpGt { dest, lhs, rhs, ty } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = if *ty == IrType::F64 {
                let lf = builder.ins().bitcast(clt::F64, MemFlags::new(), l);
                let rf = builder.ins().bitcast(clt::F64, MemFlags::new(), r);
                builder.ins().fcmp(cranelift_codegen::ir::condcodes::FloatCC::GreaterThan, lf, rf)
            } else { builder.ins().icmp(IntCC::SignedGreaterThan, l, r) };
            let v64 = builder.ins().uextend(clt::I64, v);
            def!(dest, v64);
        }
        Inst::CmpGe { dest, lhs, rhs, ty } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = if *ty == IrType::F64 {
                let lf = builder.ins().bitcast(clt::F64, MemFlags::new(), l);
                let rf = builder.ins().bitcast(clt::F64, MemFlags::new(), r);
                builder.ins().fcmp(cranelift_codegen::ir::condcodes::FloatCC::GreaterThanOrEqual, lf, rf)
            } else { builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, l, r) };
            let v64 = builder.ins().uextend(clt::I64, v);
            def!(dest, v64);
        }

        Inst::And { dest, lhs, rhs } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = builder.ins().band(l, r);
            def!(dest, v);
        }
        Inst::Or { dest, lhs, rhs } => {
            let (l, r) = (use_var!(lhs), use_var!(rhs));
            let v = builder.ins().bor(l, r);
            def!(dest, v);
        }
        Inst::Not { dest, src } => {
            let s = use_var!(src);
            let one = builder.ins().iconst(clt::I64, 1);
            let v = builder.ins().bxor(s, one);
            def!(dest, v);
        }

        Inst::Alloca { dest, .. } => {
            // Slot de pile
            let slot = builder.create_sized_stack_slot(StackSlotData::new(
                StackSlotKind::ExplicitSlot, 8,
            ));
            let addr = builder.ins().stack_addr(clt::I64, slot, 0);
            def!(dest, addr);
        }

        Inst::Store { ptr, src } => {
            let p = use_var!(ptr);
            let s = use_var!(src);
            builder.ins().store(MemFlags::new(), s, p, 0);
        }
        Inst::Load { dest, ptr, .. } => {
            let p = use_var!(ptr);
            let v = builder.ins().load(clt::I64, MemFlags::new(), p, 0);
            def!(dest, v);
        }

        Inst::Call { dest, func, args, .. } => {
            // Récupère les types attendus des paramètres (pour les bitcasts F64↔I64)
            let empty_params: Vec<cranelift_codegen::ir::Type> = vec![];
            let expected_params = param_types.get(func.as_str()).unwrap_or(&empty_params);

            let arg_vals: Vec<CrValue> = args.iter().enumerate().map(|(i, a)| {
                let v = use_var!(a);
                // Si le builtin attend F64 mais la variable est stockée en I64 (bitcast float),
                // on rebitcast I64 → F64 pour respecter la convention d'appel
                if expected_params.get(i).copied() == Some(clt::F64) {
                    builder.ins().bitcast(clt::F64, MemFlags::new(), v)
                } else {
                    v
                }
            }).collect();

            if let Some(&fid) = func_ids.get(func.as_str()) {
                let fref = module.declare_func_in_func(fid, builder.func);
                let call = builder.ins().call(fref, &arg_vals);
                if let Some(d) = dest {
                    let results = builder.inst_results(call);
                    if !results.is_empty() {
                        let result = results[0];
                        // Si le retour est F64, on le bitcast en I64 pour stockage uniforme
                        let final_val = if ret_types.get(func.as_str()).copied() == Some(clt::F64) {
                            builder.ins().bitcast(clt::I64, MemFlags::new(), result)
                        } else {
                            result
                        };
                        def!(d, final_val);
                    }
                }
            }
            // Si la fonction n'est pas connue, on ignore (runtime résolution)
        }

        Inst::CallIndirect { dest, callee, args, .. } => {
            let callee_val = use_var!(callee);
            let arg_vals: Vec<CrValue> = args.iter().map(|a| use_var!(a)).collect();
            // Signature générique I64* → I64
            let call_conv = builder.func.signature.call_conv;
            let mut sig = cranelift_codegen::ir::Signature::new(call_conv);
            for _ in &arg_vals {
                sig.params.push(AbiParam::new(clt::I64));
            }
            sig.returns.push(AbiParam::new(clt::I64));
            let sig_ref = builder.import_signature(sig);
            let call = builder.ins().call_indirect(sig_ref, callee_val, &arg_vals);
            if let Some(d) = dest {
                let results = builder.inst_results(call);
                if !results.is_empty() {
                    def!(d, results[0]);
                }
            }
        }

        Inst::Jump { target } => {
            let cl_bb = cl_blocks[target];
            builder.ins().jump(cl_bb, &[]);
        }
        Inst::Branch { cond, then_bb, else_bb } => {
            let c = use_var!(cond);
            let c1 = builder.ins().icmp_imm(IntCC::NotEqual, c, 0);
            let then_cl = cl_blocks[then_bb];
            let else_cl = cl_blocks[else_bb];
            builder.ins().brif(c1, then_cl, &[], else_cl, &[]);
        }
        Inst::Return { value } => {
            if let Some(v) = value {
                let rv = use_var!(v);
                // Si la fonction retourne F64, bitcaster i64 → f64 (les floats sont stockés bitcastés)
                let final_rv = if *func_ret_ty == IrType::F64 {
                    builder.ins().bitcast(clt::F64, MemFlags::new(), rv)
                } else {
                    rv
                };
                builder.ins().return_(&[final_rv]);
            } else {
                builder.ins().return_(&[]);
            }
        }

        Inst::Alloc { dest, class } => {
            // Dispatch selon la nature de l'allocation :
            //   "__fat_ptr"     → __alloc_fat_ptr()      (TAG_FUNCTION, sans arg)
            //   "__env_*" / "__*" → __alloc_obj(size)    (interne, sans tag)
            //   classe utilisateur → __alloc_class_obj(size) (TAG_OBJECT)
            if class == "__fat_ptr" {
                // Fat pointer : {func_ptr, env_ptr} — taille fixe 16 octets
                let alloc_fid = func_ids.get("__alloc_fat_ptr")
                    .copied()
                    .expect("__alloc_fat_ptr non déclaré");
                let fref = module.declare_func_in_func(alloc_fid, builder.func);
                let call = builder.ins().call(fref, &[]);
                let ptr  = builder.inst_results(call)[0];
                def!(dest, ptr);
            } else if class.starts_with("__") {
                // Allocations internes (closure envs, etc.) — sans tag
                let n_fields = class_layouts.get(class.as_str()).map(|f| f.len()).unwrap_or(1);
                let size     = (n_fields as i64) * 8;
                let size_val = builder.ins().iconst(clt::I64, size);
                let alloc_fid = func_ids.get("__alloc_obj")
                    .copied()
                    .expect("__alloc_obj non déclaré");
                let fref = module.declare_func_in_func(alloc_fid, builder.func);
                let call = builder.ins().call(fref, &[size_val]);
                let ptr  = builder.inst_results(call)[0];
                def!(dest, ptr);
            } else {
                // Instance de classe utilisateur — TAG_OBJECT
                let n_fields = class_layouts.get(class.as_str()).map(|f| f.len()).unwrap_or(1);
                let size     = (n_fields as i64) * 8;
                let size_val = builder.ins().iconst(clt::I64, size);
                let alloc_fid = func_ids.get("__alloc_class_obj")
                    .copied()
                    .expect("__alloc_class_obj non déclaré");
                let fref = module.declare_func_in_func(alloc_fid, builder.func);
                let call = builder.ins().call(fref, &[size_val]);
                let ptr  = builder.inst_results(call)[0];
                def!(dest, ptr);
            }
        }
        Inst::SetField { obj, field: _, src, offset } => {
            let o = use_var!(obj);
            let s = use_var!(src);
            builder.ins().store(MemFlags::new(), s, o, *offset);
        }
        Inst::GetField { dest, obj, field: _, ty: _, offset } => {
            let o = use_var!(obj);
            let v = builder.ins().load(clt::I64, MemFlags::new(), o, *offset);
            def!(dest, v);
        }

        Inst::Phi { dest, sources, .. } => {
            // Phi simplifié — utilise la première source disponible
            if let Some((val, _)) = sources.first() {
                let v = use_var!(val);
                def!(dest, v);
            }
        }

        Inst::FuncAddr { dest, func } => {
            if let Some(&fid) = func_ids.get(func.as_str()) {
                let fref = module.declare_func_in_func(fid, builder.func);
                let addr = builder.ins().func_addr(clt::I64, fref);
                def!(dest, addr);
            }
        }
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn ir_type_to_cl(ty: &IrType) -> cranelift_codegen::ir::Type {
    match ty {
        IrType::I64  => clt::I64,
        IrType::F64  => clt::F64,
        IrType::Bool => clt::I64,
        IrType::Ptr  => clt::I64,
        IrType::Void => clt::I64,
    }
}

/// Calcule le plus grand ID de valeur HIR utilisé dans une fonction
fn max_value_id(func: &IrFunction) -> u32 {
    let mut max = 0u32;
    for bb in &func.blocks {
        for inst in &bb.insts {
            visit_values(inst, |v: &Value| {
                if v.0 > max { max = v.0; }
            });
        }
    }
    max
}

fn visit_values<F: FnMut(&Value)>(inst: &Inst, mut f: F) {
    macro_rules! v { ($val:expr) => { f($val) } }
    match inst {
        Inst::ConstInt   { dest, .. }       => { v!(dest); }
        Inst::ConstFloat { dest, .. }       => { v!(dest); }
        Inst::ConstBool  { dest, .. }       => { v!(dest); }
        Inst::ConstStr   { dest, .. }       => { v!(dest); }
        Inst::Add        { dest, lhs, rhs, .. } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::Sub        { dest, lhs, rhs, .. } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::Mul        { dest, lhs, rhs, .. } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::Div        { dest, lhs, rhs, .. } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::Mod        { dest, lhs, rhs, .. } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::Neg        { dest, src, .. }  => { v!(dest); v!(src); }
        Inst::CmpEq      { dest, lhs, rhs, .. } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::CmpNe      { dest, lhs, rhs, .. } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::CmpLt      { dest, lhs, rhs, .. } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::CmpLe      { dest, lhs, rhs, .. } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::CmpGt      { dest, lhs, rhs, .. } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::CmpGe      { dest, lhs, rhs, .. } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::And        { dest, lhs, rhs } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::Or         { dest, lhs, rhs } => { v!(dest); v!(lhs); v!(rhs); }
        Inst::Not        { dest, src }      => { v!(dest); v!(src); }
        Inst::Alloca     { dest, .. }       => { v!(dest); }
        Inst::Store      { ptr, src }       => { v!(ptr); v!(src); }
        Inst::Load       { dest, ptr, .. }  => { v!(dest); v!(ptr); }
        Inst::Call       { dest, args, .. } => {
            if let Some(d) = dest { v!(d); }
            args.iter().for_each(|a| v!(a));
        }
        Inst::CallIndirect { dest, callee, args, .. } => {
            if let Some(d) = dest { v!(d); }
            v!(callee);
            args.iter().for_each(|a| v!(a));
        }
        Inst::Jump { .. }                   => {}
        Inst::Branch { cond, .. }           => { v!(cond); }
        Inst::Return { value }              => { if let Some(val) = value { v!(val); } }
        Inst::Phi    { dest, sources, .. }  => {
            v!(dest);
            sources.iter().for_each(|(val, _)| v!(val));
        }
        Inst::Alloc    { dest, .. }         => { v!(dest); }
        Inst::SetField { obj, src, .. }     => { v!(obj); v!(src); }
        Inst::GetField { dest, obj, .. }    => { v!(dest); v!(obj); }
        Inst::FuncAddr { dest, .. }         => { v!(dest); }
        Inst::Nop                           => {}
    }
}
