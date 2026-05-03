/// CraneliftEmitter : compilateur IR Ocara → code machine

use std::collections::HashMap;
use cranelift_codegen::ir::{types as clt, AbiParam, Block as CrBlock, InstBuilder, MemFlags,
    Signature, UserFuncName};
use cranelift_codegen::{Context, settings};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{DataDescription, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};

use crate::ir::func::IrFunction;
use crate::ir::inst::BlockId;
use crate::ir::module::IrModule;
use crate::ir::types::IrType;
use crate::codegen::runtime::{builtin_sig, BUILTINS};
use super::error::{CodegenError, CgResult};
use super::helpers::{ir_type_to_cl, max_value_id};
use super::instructions::emit_inst;

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
