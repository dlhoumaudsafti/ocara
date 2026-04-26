use std::collections::{HashMap, HashSet};
use crate::ast::*;
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
            elem_types: HashMap::new(),
            map_vars: HashSet::new(),
            var_class: HashMap::new(),
            current_class: None,
            loop_stack: Vec::new(),
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

    /// Stocke `src` dans le slot du local `name`
    pub fn store_local(&mut self, name: &str, src: Value) {
        if let Some((slot, _, _)) = self.locals.get(name) {
            let slot = slot.clone();
            self.emit(Inst::Store { ptr: slot, src });
        }
    }

    /// Charge le local `name` → retourne (Value résultat, IrType)
    pub fn load_local(&mut self, name: &str) -> Option<(Value, IrType)> {
        if let Some((slot, ty, _)) = self.locals.get(name).cloned() {
            let dest = self.new_value();
            self.emit(Inst::Load { dest: dest.clone(), ptr: slot, ty: ty.clone() });
            Some((dest, ty))
        } else {
            None
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Point d'entrée du lowering
// ─────────────────────────────────────────────────────────────────────────────

pub fn lower_program(program: &Program) -> IrModule {
    let module_name = "ocara_module".to_string();
    let mut module = IrModule::new(module_name);

    // Enregistre les modules importés (dernier segment du path : "ocara.IO" → "IO")
    for imp in &program.imports {
        if let Some(last) = imp.path.last() {
            module.imports.push(last.clone());
        }
    }

    // Constantes globales → globals du module
    for c in &program.consts {
        lower_const_global(&mut module, c);
    }

    // Pré-collecte des types de retour de toutes les fonctions utilisateur
    let mut fn_ret_types: HashMap<String, IrType> = HashMap::new();
    for func in &program.functions {
        fn_ret_types.insert(func.name.clone(), IrType::from_ast(&func.ret_ty));
    }

    // Enregistre les parents et construit les layouts (champs parents en premier)
    for class in &program.classes {
        if let Some(parent_name) = &class.extends {
            module.class_parents.insert(class.name.clone(), parent_name.clone());
        }
    }
    // Construction des layouts dans l'ordre (parents avant enfants) — on refait si besoin
    fn collect_fields(
        classes: &[ClassDecl],
        class_name: &str,
    ) -> Vec<(String, IrType)> {
        let class = match classes.iter().find(|c| c.name == class_name) {
            Some(c) => c,
            None    => return vec![],
        };
        let mut fields = if let Some(parent) = &class.extends {
            collect_fields(classes, parent)
        } else {
            vec![]
        };
        for member in &class.members {
            if let ClassMember::Field { name, ty, .. } = member {
                fields.push((name.clone(), IrType::from_ast(ty)));
            }
        }
        fields
    }
    for class in &program.classes {
        let fields = collect_fields(&program.classes, &class.name);
        module.class_layouts.insert(class.name.clone(), fields);
    }

    // Collecte les types de paramètres des constructeurs (pour le boxing mixed)
    for class in &program.classes {
        if let Some(ctor_params) = class.members.iter().find_map(|m| {
            if let ClassMember::Constructor { params, .. } = m { Some(params) } else { None }
        }) {
            let param_types: Vec<IrType> = ctor_params.iter()
                .map(|p| IrType::from_ast(&p.ty))
                .collect();
            module.ctor_param_types.insert(class.name.clone(), param_types);
        }
    }

    // Collecte les constantes de classes pour inlining (Class::NAME)
    for class in &program.classes {
        for member in &class.members {
            if let ClassMember::Const { name, ty, value, .. } = member {
                if let Expr::Literal(lit, _) = value {
                    let key = format!("{}__{}", class.name, name);
                    module.class_consts.insert(key, (IrType::from_ast(ty), lit.clone()));
                }
            }
        }
    }

    // Collecte les types de retour des méthodes propres
    for class in &program.classes {
        for member in &class.members {
            if let ClassMember::Method { decl, .. } = member {
                fn_ret_types.insert(
                    format!("{}_{}", class.name, decl.name),
                    IrType::from_ast(&decl.ret_ty),
                );
            }
        }
    }
    // Propage les types de retour des méthodes héritées (non surchargées) dans fn_ret_types
    for class in &program.classes {
        if let Some(parent_name) = &class.extends {
            let own_methods: HashSet<String> = class.members.iter()
                .filter_map(|m| if let ClassMember::Method { decl, .. } = m { Some(decl.name.clone()) } else { None })
                .collect();
            if let Some(parent) = program.classes.iter().find(|c| &c.name == parent_name) {
                for member in &parent.members {
                    if let ClassMember::Method { decl, .. } = member {
                        if !own_methods.contains(&decl.name) {
                            let child_key  = format!("{}_{}", class.name, decl.name);
                            let parent_key = format!("{}_{}", parent_name, decl.name);
                            if let Some(ty) = fn_ret_types.get(&parent_key).cloned() {
                                fn_ret_types.insert(child_key, ty);
                            }
                        }
                    }
                }
            }
        }
    }

    // Fonctions libres (les constantes sont inlinées dans chaque fonction)
    for func in &program.functions {
        lower_func(&mut module, func, &program.consts, &fn_ret_types, None);
    }

    // Méthodes de classes (passe toutes les classes pour l'héritage)
    for class in &program.classes {
        lower_class(&mut module, class, &program.classes, &program.consts, &fn_ret_types);
    }

    module
}

// ─────────────────────────────────────────────────────────────────────────────
// Constante globale
// ─────────────────────────────────────────────────────────────────────────────

fn lower_const_global(module: &mut IrModule, c: &ConstDecl) {
    use crate::ir::module::IrGlobal;

    let bytes = match &c.value {
        Expr::Literal(Literal::Int(n), _)   => n.to_le_bytes().to_vec(),
        Expr::Literal(Literal::Float(f), _) => f.to_le_bytes().to_vec(),
        Expr::Literal(Literal::Bool(b), _)  => vec![*b as u8],
        Expr::Literal(Literal::String(s), _) => s.as_bytes().to_vec(),
        _ => vec![],
    };
    module.add_global(IrGlobal { name: c.name.clone(), bytes });
}

// ─────────────────────────────────────────────────────────────────────────────
// Fonction libre
// ─────────────────────────────────────────────────────────────────────────────

pub fn lower_func(module: &mut IrModule, func: &FuncDecl, consts: &[crate::ast::ConstDecl], fn_ret_types: &HashMap<String, IrType>, class_name: Option<&str>) {
    let ir_params: Vec<IrParam> = func.params.iter().enumerate().map(|(i, p)| {
        IrParam {
            name: p.name.clone(),
            ty:   IrType::from_ast(&p.ty),
            slot: Value(i as u32),
        }
    }).collect();
    let ret_ty = IrType::from_ast(&func.ret_ty);

    let mut builder = LowerBuilder::new(module, func.name.clone(), ir_params.clone(), ret_ty);
    // Si on est dans une méthode/constructeur de classe, enregistrer la classe courante
    if let Some(cls) = class_name {
        builder.current_class = Some(cls.to_string());
        builder.var_class.insert("self".to_string(), cls.to_string());
    }
    builder.fn_ret_types = fn_ret_types.iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for c in consts {
        let ir_ty = IrType::from_ast(&c.ty);
        let _slot = builder.declare_local(&c.name, ir_ty, false);
        let val = crate::lower::expr::lower_expr(&mut builder, &c.value);
        builder.store_local(&c.name, val);
    }

    // Enregistre les paramètres comme locaux immuables et met à jour IrParam::slot
    // pour pointer vers l'alloca réel (les consts ont avancé next_value).
    let updated_params: Vec<IrParam> = func.params.iter().map(|param| {
        let ir_ty = IrType::from_ast(&param.ty);
        // Marquer les paramètres de type map<> pour Expr::Index → __map_get
        if let crate::ast::Type::Map(_, _) = &param.ty {
            builder.map_vars.insert(param.name.clone());
        }
        // Slot alloca qui recevra la valeur du paramètre
        let alloca_slot = builder.declare_local(&param.name, ir_ty.clone(), false);
        // Variable « receiver » distincte : mappée aux block_params Cranelift
        let receiver = builder.new_value();
        // Store initial : param_receiver → alloca_slot (remplit le slot stack)
        builder.emit(Inst::Store { ptr: alloca_slot, src: receiver.clone() });
        // IrParam::slot pointe vers le receiver (utilisé dans le block-param mapping)
        IrParam { name: param.name.clone(), ty: ir_ty, slot: receiver }
    }).collect();
    builder.func.params = updated_params;

    // Body
    crate::lower::stmt::lower_block(&mut builder, &func.body);

    // Return implicite si le bloc courant n'est pas terminé
    if !builder.is_terminated() {
        let ret_ty_copy = builder.func.ret_ty.clone();
        let ret_val = if ret_ty_copy != IrType::Void {
            // Bloc mort (toutes les branches ont retourné) : valeur dummy
            let zero = builder.new_value();
            builder.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
            Some(zero)
        } else {
            None
        };
        builder.emit(Inst::Return { value: ret_val });
    }

    let ir_func = builder.func;
    module.add_function(ir_func);
}

// ─────────────────────────────────────────────────────────────────────────────
// Classe
// ─────────────────────────────────────────────────────────────────────────────

fn lower_class(module: &mut IrModule, class: &ClassDecl, all_classes: &[ClassDecl], consts: &[crate::ast::ConstDecl], fn_ret_types: &HashMap<String, IrType>) {
    // Collecte des noms de méthodes propres (pour détecter les surcharges)
    let own_methods: HashSet<String> = class.members.iter()
        .filter_map(|m| if let ClassMember::Method { decl, .. } = m { Some(decl.name.clone()) } else { None })
        .collect();

    for member in &class.members {
        match member {
            ClassMember::Method { decl, is_static, .. } => {
                if *is_static {
                    // Méthode statique : pas de self, appelée via Class::method()
                    let mangled = FuncDecl {
                        name: format!("{}_{}", class.name, decl.name),
                        ..decl.clone()
                    };
                    lower_func(module, &mangled, consts, fn_ret_types, None);
                } else {
                let self_param = crate::ast::Param {
                    name: "self".into(),
                    ty:   Type::Mixed,
                    span: decl.span.clone(),
                };
                let mut full_params = vec![self_param];
                full_params.extend(decl.params.clone());
                let mangled = FuncDecl {
                    name:   format!("{}_{}", class.name, decl.name),
                    params: full_params,
                    ..decl.clone()
                };
                lower_func(module, &mangled, consts, fn_ret_types, Some(&class.name));
                }
            }
            ClassMember::Constructor { params, body, span } => {
                let self_param = crate::ast::Param {
                    name: "self".into(),
                    ty:   Type::Mixed,
                    span: span.clone(),
                };
                let mut full_params = vec![self_param];
                full_params.extend(params.clone());
                let init_func = FuncDecl {
                    name:   format!("{}_init", class.name),
                    params: full_params,
                    ret_ty: Type::Void,
                    body:   body.clone(),
                    span:   span.clone(),
                };
                lower_func(module, &init_func, consts, fn_ret_types, Some(&class.name));
            }
            ClassMember::Const { name, value, .. } => {
                use crate::ir::module::IrGlobal;
                let bytes = match value {
                    Expr::Literal(Literal::Int(n), _)    => n.to_le_bytes().to_vec(),
                    Expr::Literal(Literal::Float(f), _)  => f.to_le_bytes().to_vec(),
                    Expr::Literal(Literal::Bool(b), _)   => vec![*b as u8],
                    Expr::Literal(Literal::String(s), _) => s.as_bytes().to_vec(),
                    _ => vec![],
                };
                module.add_global(IrGlobal {
                    name: format!("{}__{}" , class.name, name),
                    bytes,
                });
            }
            ClassMember::Field { .. } => {}
        }
    }

    // Émettre les méthodes héritées non surchargées comme Child_method
    if let Some(parent_name) = &class.extends {
        if let Some(parent) = all_classes.iter().find(|c| c.name == *parent_name) {
            for member in &parent.members {
                if let ClassMember::Method { decl, .. } = member {
                    if !own_methods.contains(&decl.name) {
                        let self_param = crate::ast::Param {
                            name: "self".into(),
                            ty:   Type::Mixed,
                            span: decl.span.clone(),
                        };
                        let mut full_params = vec![self_param];
                        full_params.extend(decl.params.clone());
                        let mangled = FuncDecl {
                            name:   format!("{}_{}", class.name, decl.name),
                            params: full_params,
                            ..decl.clone()
                        };
                        // Émettre avec le contexte de la classe enfant (layouts corrects)
                        lower_func(module, &mangled, consts, fn_ret_types, Some(&class.name));
                    }
                }
            }
        }
    }
}
