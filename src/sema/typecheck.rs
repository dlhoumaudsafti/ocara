use crate::parsing::ast::*;
use crate::sema::error::{SemaError, SemaWarning};
use crate::sema::scope::{LocalBinding, ScopeStack, OwnershipClass, ownership_class_of};
use crate::sema::symbols::SymbolTable;
use crate::parsing::token::Span;

// ─────────────────────────────────────────────────────────────────────────────
// TypeChecker
// ─────────────────────────────────────────────────────────────────────────────

pub struct TypeChecker<'a> {
    pub symbols:   &'a SymbolTable,
    pub errors:    Vec<SemaError>,
    pub warnings:  Vec<SemaWarning>,
    scopes:        ScopeStack,
    /// Type de retour de la fonction en cours d'analyse
    current_ret:   Option<Type>,
    /// Nom de la classe en cours (pour `self`)
    current_class: Option<String>,
    /// Contexte runtime actuel (init, main, error, success, exit)
    current_runtime_ctx: Option<String>,
    /// Classes déjà typecheckées (pour éviter de les typecheck plusieurs fois)
    checked_classes: std::collections::HashSet<String>,
    /// Référence au Program pour accéder aux ClassDecl lors du typecheck lazy
    program: Option<&'a Program>,
    /// Mapping var_name → func_name pour les variables qui contiennent un task handle async.
    /// Utilisé par Expr::Resolve pour retrouver le type de retour original.
    async_var_funcs: std::collections::HashMap<String, String>,
    /// Paramètres échappants par fonction/méthode/constructeur utilisateur
    /// (voir `crate::sema::escape`) — calculé une fois dans `check_program`,
    /// consulté par `check_argument_escape` (diagnostic E26/ArgumentEscape).
    escaping_params: std::collections::HashMap<crate::sema::escape::CalleeKey, Vec<bool>>,
    /// `class_name → membres appelables` — calculé une fois dans
    /// `check_program`, utilisé pour résoudre un appel vers une classe
    /// utilisateur (voir `crate::sema::escape::resolve_user_callable`).
    class_members: crate::sema::escape::ClassMembers,
    /// Classes utilisateur contenant (directement ou transitivement) une
    /// ressource native — calculé une fois dans `check_program`, voir
    /// `crate::sema::scope::compute_resource_classes`/`ownership_class_of`.
    resource_classes: std::collections::HashSet<String>,
}

impl<'a> TypeChecker<'a> {
    pub fn new(symbols: &'a SymbolTable) -> Self {
        Self {
            symbols,
            errors:   Vec::new(),
            warnings: Vec::new(),
            scopes:   ScopeStack::default(),
            current_ret:   None,
            current_class: None,
            current_runtime_ctx: None,
            checked_classes: std::collections::HashSet::new(),
            program: None,
            async_var_funcs: std::collections::HashMap::new(),
            escaping_params: std::collections::HashMap::new(),
            class_members: std::collections::HashMap::new(),
            resource_classes: std::collections::HashSet::new(),
        }
    }
    
    // ── Helper pour ajouter le contexte runtime au span ──────────────────────
    
    fn with_runtime_ctx(&self, span: &Span) -> Span {
        let mut s = span.clone();
        if let Some(ctx) = &self.current_runtime_ctx {
            s.runtime_ctx = Some(ctx.clone());
        }
        s
    }

    // ── Point d'entrée ───────────────────────────────────────────────────────

    pub fn check_program(&mut self, program: &'a Program) {
        // Stocker la référence au program pour le typecheck lazy
        self.program = Some(program);

        // Analyse d'échappement interprocédurale (voir crate::sema::escape)
        // — nécessaire pour le diagnostic E26 (ArgumentEscape) ci-dessous.
        self.escaping_params = crate::sema::escape::compute_escaping_params(program);
        self.class_members = crate::sema::escape::collect_class_members(&program.classes);
        self.resource_classes = crate::sema::scope::compute_resource_classes(&program.classes);

        // W04 : ressource scoped/consumed encore ouverte au moment d'un
        // raise non rattrapé localement — voir crate::sema::resource_raise.
        self.warnings.extend(crate::sema::resource_raise::check_program(program, &self.resource_classes));

        // Enums — vérifier les doublons de variantes
        for en in &program.enums {
            self.check_enum(en);
        }
        
        // Blocs runtime AVANT les classes et fonctions
        // Comme ça, les classes utilisées dans les runtime blocks seront typecheckées avec le contexte
        self.check_runtime_blocks(program);
        
        // Fonctions libres — vérifier les types de retour mixed
        for func in &program.functions {
            if let Type::Mixed = func.ret_ty {
                self.errors.push(SemaError::MixedInReturnType {
                    name: func.name.clone(),
                    span: func.span.clone(),
                });
            }
            self.check_func(func);
        }
        // Classes (celles qui n'ont pas été typecheckées via les runtime blocks)
        for class in &program.classes {
            self.check_class(class);
        }
    }

    // ── Enum ─────────────────────────────────────────────────────────────────

    fn check_enum(&mut self, en: &crate::parsing::ast::EnumDecl) {
        let mut seen = std::collections::HashSet::new();
        for v in &en.variants {
            if !seen.insert(v.name.clone()) {
                self.errors.push(SemaError::DuplicateSymbol {
                    name: format!("{}::{}", en.name, v.name),
                    span: v.span.clone(),
                });
            }
        }
    }

    // ── Fonction ─────────────────────────────────────────────────────────────

    fn check_func(&mut self, func: &FuncDecl) {
        self.scopes.push();
        // Sauvegarder/restaurer current_ret (pas juste le remettre à None) : cette
        // fonction peut être appelée EN IMBRICATION d'une autre vérification en
        // cours (le typecheck des classes est lazy — check_class() est déclenché
        // au premier usage d'une classe, potentiellement DEPUIS le corps d'une
        // fonction déjà en cours de vérification). Sans save/restore, vérifier une
        // méthode void appelée depuis `main(): int` écrasait le current_ret de
        // main à None, faisant croire que les `return <valeur>` suivants de main
        // visaient une fonction void (bug : "expected return type 'void', found
        // 'int'" sur un simple `function main(): int { ...; return 0 }`).
        let saved_ret = self.current_ret.take();
        self.current_ret = Some(func.ret_ty.clone());

        // `message<T>` (générateurs) est return-type-only : jamais un type
        // de paramètre (voir docs/roadmap.d/langage-emit-iterable.md).
        for param in &func.params {
            if let Type::Message(_) = &param.ty {
                self.errors.push(SemaError::MessageAsParamType {
                    name: param.name.clone(),
                    span: param.span.clone(),
                });
            }
        }

        // `message<T>` en retour ⇒ le corps doit contenir au moins un `emit`
        // atteignable — sinon `message<T>` n'a ici aucun sens (voir la même
        // fiche roadmap).
        if let Type::Message(_) = &func.ret_ty {
            let analysis = crate::sema::message_emit::analyze_emit(&func.body);
            if !analysis.has_emit {
                self.errors.push(SemaError::MessageReturnWithoutEmit {
                    name: func.name.clone(),
                    span: func.span.clone(),
                });
            }
            // Cas A (emit dans un try) : désormais pris en charge par le
            // lowering (voir `crate::lower::builder::message_gen::lower_try_in_generator`
            // et docs/roadmap.d/langage-emit-iterable.md, §4) — plus de
            // restriction ici.
        }

        for param in &func.params {
            // Warning si variadic<mixed>
            if param.is_variadic {
                if let Type::Mixed = param.ty {
                    self.warnings.push(SemaWarning::VariadicMixed {
                        name: param.name.clone(),
                        span: param.span.clone(),
                    });
                }
            }

            // Désucrage : variadic<T> → T[] dans le corps de la fonction
            let param_ty = if param.is_variadic {
                Type::Array(Box::new(param.ty.clone()))
            } else {
                param.ty.clone()
            };
            
            self.scopes.declare(
                param.name.clone(),
                LocalBinding { ty: param_ty, mutable: false, span: param.span.clone(), used: false, is_param: true, kind: VarKind::Var, consumed_used_at: None, resource_finalized: false, resource_contained: false },
            );
        }

        self.check_block(&func.body);
        { let _u = self.scopes.pop_scope(&self.resource_classes); self.flush_warnings(_u); }
        self.current_ret = saved_ret;
    }

    // ── Classe ───────────────────────────────────────────────────────────────

    fn check_class(&mut self, class: &ClassDecl) {
        // Ne pas typecheck deux fois la même classe
        if self.checked_classes.contains(&class.name) {
            return;
        }
        self.checked_classes.insert(class.name.clone());

        // Save/restore (même raison que current_ret dans check_func) : le
        // typecheck lazy peut déclencher check_class() DEPUIS le corps d'une
        // méthode d'une AUTRE classe déjà en cours de vérification — sans
        // save/restore, ceci écraserait durablement le current_class du
        // contexte englobant (self:: y résoudrait alors la mauvaise classe).
        let saved_class = self.current_class.take();
        self.current_class = Some(class.name.clone());

        for member in &class.members {
            match member {
                ClassMember::Method { decl, .. } => {
                    // Vérifier le type de retour mixed
                    if let Type::Mixed = decl.ret_ty {
                        self.errors.push(SemaError::MixedInReturnType {
                            name: format!("{}::{}", class.name, decl.name),
                            span: decl.span.clone(),
                        });
                    }
                    self.check_func(decl)
                },
                ClassMember::Constructor { params, body, .. } => {
                    self.scopes.push();
                    // Save/restore : voir le commentaire équivalent dans check_func —
                    // même risque d'écrasement du current_ret d'une fonction englobante
                    // via le typecheck lazy des classes.
                    let saved_ret = self.current_ret.take();
                    self.current_ret = Some(Type::Void);
                    for p in params {
                        // `message<T>` return-type-only (voir check_func) :
                        // s'applique aussi aux paramètres de constructeur.
                        if let Type::Message(_) = &p.ty {
                            self.errors.push(SemaError::MessageAsParamType {
                                name: p.name.clone(),
                                span: p.span.clone(),
                            });
                        }
                        // Warning si variadic<mixed>
                        if p.is_variadic {
                            if let Type::Mixed = p.ty {
                                self.warnings.push(SemaWarning::VariadicMixed {
                                    name: p.name.clone(),
                                    span: p.span.clone(),
                                });
                            }
                        }
                        
                        // Désucrage : variadic<T> → T[]
                        let param_ty = if p.is_variadic {
                            Type::Array(Box::new(p.ty.clone()))
                        } else {
                            p.ty.clone()
                        };
                        
                        self.scopes.declare(
                            p.name.clone(),
                            LocalBinding { ty: param_ty, mutable: false, span: p.span.clone(), used: false, is_param: true, kind: VarKind::Var, consumed_used_at: None, resource_finalized: false, resource_contained: false },
                        );
                    }
                    self.check_block(body);
                    { let _u = self.scopes.pop_scope(&self.resource_classes); self.flush_warnings(_u); }
                    self.current_ret = saved_ret;
                }
                ClassMember::Const { ty, value, span, .. } => {
                    let val_ty = self.infer_expr(value);
                    if !types_compat(&val_ty, ty, &self.symbols) {
                        self.errors.push(SemaError::TypeMismatch {
                            expected: type_name(ty),
                            found:    type_name(&val_ty),
                            span:     span.clone(),
                        });
                    }
                }
                ClassMember::Field { name, ty, span, .. } => {
                    // Vérifier que les property ne sont pas de type mixed
                    if let Type::Mixed = ty {
                        self.errors.push(SemaError::MixedInProperty {
                            class: class.name.clone(),
                            field: name.clone(),
                            span: span.clone(),
                        });
                    }
                    // Un champ de type ressource (Mutex/SQLite/MySQL/MariaDB/
                    // HTTPRequest/HTTPResponse) est autorisé : `__free_<Classe>`
                    // le ferme désormais via son symbole runtime dédié (voir
                    // `class_ownership::classify_field`/`FieldOwnership::Resource`).
                    // La classe porteuse est de ce fait traitée comme
                    // `OwnershipClass::Resource` PARTOUT où l'échappement est
                    // vérifié (voir `compute_resource_classes`/
                    // `ownership_class_of`, calculé une fois dans
                    // `check_program`) — exactement les mêmes règles qu'une
                    // ressource nue : ne peut pas s'échapper de son bloc
                    // `scoped`/`consumed` (E18), et un `var`/`const` doit
                    // être fermé manuellement ou prouvé non-échappant (E28).
                    // Sans ça, deux instances pourraient se retrouver à
                    // partager le même handle natif (`__clone_<Classe>` ne
                    // clone jamais un champ ressource, voir
                    // `FieldOwnership::Resource`) et l'une fermerait la
                    // ressource sous le nez de l'autre.
                }
            }
        }

        self.current_class = saved_class;
    }

    // ── Blocs runtime ────────────────────────────────────────────────────────

    fn check_runtime_blocks(&mut self, program: &Program) {
        use std::collections::HashMap;
        
        // Vérifier qu'il n'y a pas de doublons de blocs runtime
        let mut seen_kinds: HashMap<crate::parsing::ast::RuntimeBlockKind, crate::parsing::token::Span> = HashMap::new();
        for block in &program.runtime_blocks {
            if let Some(_prev_span) = seen_kinds.get(&block.kind) {
                self.errors.push(SemaError::DuplicateSymbol {
                    name: format!("{} runtime block", block.kind.as_str()),
                    span: block.span.clone(),
                });
                // Note: on pourrait aussi référencer _prev_span dans l'erreur
            } else {
                seen_kinds.insert(block.kind, block.span.clone());
            }
        }
        
        // Le bloc main est obligatoire (au moins l'un des blocs doit exister)
        // Note: Pour l'instant, nous permettons des programmes sans blocs runtime
        // (pour compatibilité avec le code existant)
        
        // Vérifier tous les blocs runtime comme s'ils étaient dans une seule fonction
        // pour éviter les warnings "unused" sur les variables qui flow entre blocs
        self.check_runtime_blocks_merged(program);
    }
    
    fn check_runtime_blocks_merged(&mut self, program: &Program) {
        // Créer un scope global pour tous les blocs runtime
        self.scopes.push();
        
        // Injecter les variables magiques ERROR et SUCCESS
        // Elles sont toujours disponibles dans les blocs runtime pour permettre
        // leur utilisation dans tous les blocs (par exemple: return ERROR dans main)
        self.scopes.declare(
            "ERROR".to_string(),
            LocalBinding {
                ty: Type::Int,
                mutable: true,
                span: crate::parsing::token::Span::new(0, 0),
                used: true,
                is_param: false,
                kind: VarKind::Var,
                consumed_used_at: None,
                resource_finalized: false, resource_contained: false,
            },
        );
        
        self.scopes.declare(
            "SUCCESS".to_string(),
            LocalBinding {
                ty: Type::Bool,
                mutable: true,
                span: crate::parsing::token::Span::new(0, 0),
                used: true,
                is_param: false,
                kind: VarKind::Var,
                consumed_used_at: None,
                resource_finalized: false, resource_contained: false,
            },
        );
        
        // Vérifier tous les statements de tous les blocs dans l'ordre
        let order = [
            crate::parsing::ast::RuntimeBlockKind::Init,
            crate::parsing::ast::RuntimeBlockKind::Main,
            crate::parsing::ast::RuntimeBlockKind::Error,
            crate::parsing::ast::RuntimeBlockKind::Success,
            crate::parsing::ast::RuntimeBlockKind::Exit,
        ];
        
        for kind in &order {
            if let Some(block) = program.runtime_blocks.iter().find(|b| b.kind == *kind) {
                // Définir le contexte runtime actuel
                self.current_runtime_ctx = Some(kind.as_str().to_string());
                
                for stmt in &block.statements {
                    self.check_stmt(stmt);
                }
                
                // Réinitialiser le contexte
                self.current_runtime_ctx = None;
            }
        }
        
        // Pop scope et flush warnings
        let _u = self.scopes.pop_scope(&self.resource_classes);
        self.flush_warnings(_u);
    }

    // ── Block ─────────────────────────────────────────────────────────────────

    fn check_block(&mut self, block: &Block) {
        self.scopes.push();
        for (i, stmt) in block.stmts.iter().enumerate() {
            self.check_stmt(stmt);
            self.check_resource_var_containment(stmt, block, i);
        }
        { let _u = self.scopes.pop_scope(&self.resource_classes); self.flush_warnings(_u); }
    }

    /// Juste après avoir déclaré un `var`/`const` d'un type ressource
    /// (`Mutex`/`SQLite`/`MySQL`/`MariaDB`), détermine s'il ne s'échappe
    /// jamais du reste de `block` (même analyse que pour la libération
    /// automatique d'un `var`, voir `crate::sema::escape::var_never_escapes`)
    /// et, si c'est prouvé, le marque `resource_contained` — condition
    /// nécessaire (mais pas suffisante : voir `pop_scope`) pour le diagnostic
    /// `UnclosedResourceVar`. Ne fait rien pour `scoped`/`consumed` (déjà
    /// finalisées automatiquement en fin de bloc, aucun risque de fuite).
    fn check_resource_var_containment(&mut self, stmt: &Stmt, block: &Block, i: usize) {
        let (name, ty) = match stmt {
            Stmt::Var { name, ty, kind: VarKind::Var, .. } => (name, ty),
            Stmt::Const { name, ty, .. } => (name, ty),
            _ => return,
        };
        if ownership_class_of(ty, &self.resource_classes) != OwnershipClass::Resource {
            return;
        }
        if crate::sema::escape::var_never_escapes(
            &self.class_members, name, block, i, self.current_class.as_deref(), &self.escaping_params,
        ) {
            self.scopes.mark_resource_contained(name);
        }
    }

    /// Convertit ce que `pop_scope` a trouvé en dépilant le scope courant :
    /// variables inutilisées (warning), `Thread` `scoped`/`consumed` jamais
    /// `.join()`/`.detach()`, et `var`/`const` ressource qui fuient leur
    /// handle natif (les deux dernières : erreurs — voir
    /// `OwnershipClass::Thread`/`Resource`).
    fn flush_warnings(&mut self, popped: crate::sema::scope::PoppedScope) {
        for u in popped.unused {
            self.warnings.push(SemaWarning::UnusedVariable { name: u.name, span: u.span });
        }
        for t in popped.unfinalized_threads {
            self.errors.push(SemaError::ThreadNotFinalized { name: t.name, span: t.span });
        }
        for r in popped.unclosed_resource_vars {
            self.errors.push(SemaError::UnclosedResourceVar { name: r.name, ty_name: r.ty_name, span: r.span });
        }
    }

    // ── Statement ────────────────────────────────────────────────────────────

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Var { name, ty, value, mutable, kind, span } => {
                let ty_is_message = matches!(ty, Type::Message(_));
                if ty_is_message {
                    self.errors.push(SemaError::MessageNotNameable {
                        name: name.clone(),
                        span: span.clone(),
                    });
                }
                let val_ty = self.infer_expr(value);
                // `value` peut lui-même être une `scoped`/`consumed` d'un
                // autre binding (`var y = x`) — c'est un point d'échappement.
                self.check_escape(value);
                self.check_message_scalar_consumption(value, &val_ty, span);
                // Si `ty` est déjà `message<T>` (rejeté juste au-dessus par
                // MessageNotNameable), un TypeMismatch ici ne ferait
                // qu'ajouter du bruit ("expected message<int>, found
                // message<int>" — `types_compat` déballe TOUJOURS le côté
                // "found", jamais le côté "expected").
                if !ty_is_message && !types_compat(&val_ty, ty, &self.symbols) {
                    self.errors.push(SemaError::TypeMismatch {
                        expected: type_name(ty),
                        found:    type_name(&val_ty),
                        span:     span.clone(),
                    });
                }
                // Warning si le type est mixed
                if let Type::Mixed = ty {
                    self.warnings.push(SemaWarning::MixedLocalVariable {
                        name: name.clone(),
                        span: span.clone(),
                    });
                }
                // NB : `scoped`/`consumed` sur un type non pris en charge par
                // ce chantier (primitif, Function, classe utilisateur,
                // SDL/Tauri...) n'est PAS une erreur — `scoped` est déjà
                // largement utilisée ainsi dans le code existant (déclaration
                // "locale au bloc" générique, sans intention de possession
                // d'une ressource tas). Ces cas se comportent exactement
                // comme `var` : aucune destruction, aucune restriction —
                // voir `OwnershipClass::Unsupported` et `check_escape`.
                // Tracker les variables qui stockent un task handle async
                if let Expr::Call { callee, .. } = value {
                    if let Expr::Ident(func_name, _) = callee.as_ref() {
                        if let Some(sig) = self.symbols.lookup_function(func_name) {
                            if sig.is_async {
                                self.async_var_funcs.insert(name.clone(), func_name.clone());
                            }
                        }
                    }
                }
                if !self.scopes.declare(
                    name.clone(),
                    LocalBinding { ty: ty.clone(), mutable: *mutable, span: span.clone(), used: false, is_param: false, kind: *kind, consumed_used_at: None, resource_finalized: false, resource_contained: false },
                ) {
                    self.errors.push(SemaError::DuplicateSymbol {
                        name: name.clone(),
                        span: span.clone(),
                    });
                }
            }

            Stmt::Const { name, ty, value, span } => {
                let ty_is_message = matches!(ty, Type::Message(_));
                if ty_is_message {
                    self.errors.push(SemaError::MessageNotNameable {
                        name: name.clone(),
                        span: span.clone(),
                    });
                }
                let val_ty = self.infer_expr(value);
                self.check_message_scalar_consumption(value, &val_ty, span);
                if !ty_is_message && !types_compat(&val_ty, ty, &self.symbols) {
                    self.errors.push(SemaError::TypeMismatch {
                        expected: type_name(ty),
                        found:    type_name(&val_ty),
                        span:     span.clone(),
                    });
                }
                if !self.scopes.declare(
                    name.clone(),
                    LocalBinding { ty: ty.clone(), mutable: false, span: span.clone(), used: false, is_param: false, kind: VarKind::Var, consumed_used_at: None, resource_finalized: false, resource_contained: false },
                ) {
                    self.errors.push(SemaError::DuplicateSymbol {
                        name: name.clone(),
                        span: span.clone(),
                    });
                }
            }

            Stmt::Expr(expr) => { self.infer_expr(expr); }

            Stmt::If { condition, then_block, elseif, else_block, span } => {
                let cond_ty = self.infer_expr(condition);
                if !types_compat(&cond_ty, &Type::Bool, &self.symbols) {
                    self.errors.push(SemaError::TypeMismatch {
                        expected: "bool".into(),
                        found:    type_name(&cond_ty),
                        span:     span.clone(),
                    });
                }
                self.check_block(then_block);
                for (cond, blk) in elseif {
                    self.infer_expr(cond);
                    self.check_block(blk);
                }
                if let Some(blk) = else_block {
                    self.check_block(blk);
                }
            }

            Stmt::Switch { subject, cases, default, .. } => {
                self.infer_expr(subject);
                for case in cases { self.check_block(&case.body); }
                if let Some(blk) = default { self.check_block(blk); }
            }

            Stmt::While { condition, body, span } => {
                let cond_ty = self.infer_expr(condition);
                if !types_compat(&cond_ty, &Type::Bool, &self.symbols) {
                    self.errors.push(SemaError::TypeMismatch {
                        expected: "bool".into(),
                        found:    type_name(&cond_ty),
                        span:     span.clone(),
                    });
                }
                self.check_block(body);
            }

            Stmt::ForIn { var, iter, body, span } => {
                let iter_ty = self.infer_expr(iter);
                // L'itérateur doit être un range (int), un tableau, ou un
                // générateur `message<T>` (voir
                // docs/roadmap.d/langage-emit-iterable.md — `for` reste
                // valable quel que soit le nombre d'`emit`, y compris dans
                // une boucle : pas de restriction ici, contrairement à la
                // consommation scalaire directe).
                let elem_ty = match &iter_ty {
                    Type::Array(inner)   => *inner.clone(),
                    Type::Message(inner) => *inner.clone(),
                    Type::Int            => Type::Int, // range produit des int
                    _ => {
                        self.errors.push(SemaError::TypeMismatch {
                            expected: "itérable".into(),
                            found:    type_name(&iter_ty),
                            span:     span.clone(),
                        });
                        Type::Mixed
                    }
                };
                self.scopes.push();
                self.scopes.declare(var.clone(), LocalBinding { ty: elem_ty, mutable: false, span: span.clone(), used: false, is_param: true, kind: VarKind::Var, consumed_used_at: None, resource_finalized: false, resource_contained: false });
                self.check_block(body);
                { let _u = self.scopes.pop_scope(&self.resource_classes); self.flush_warnings(_u); }
            }

            Stmt::ForMap { key, value, iter, body, span } => {
                self.infer_expr(iter);
                self.scopes.push();
                self.scopes.declare(key.clone(),   LocalBinding { ty: Type::Mixed, mutable: false, span: span.clone(), used: false, is_param: true, kind: VarKind::Var, consumed_used_at: None, resource_finalized: false, resource_contained: false });
                self.scopes.declare(value.clone(), LocalBinding { ty: Type::Mixed, mutable: false, span: span.clone(), used: false, is_param: true, kind: VarKind::Var, consumed_used_at: None, resource_finalized: false, resource_contained: false });
                self.check_block(body);
                { let _u = self.scopes.pop_scope(&self.resource_classes); self.flush_warnings(_u); }
            }

            Stmt::Return { value, span } => {
                // `return` est réservé aux fonctions/méthodes normales : dans un bloc
                // runtime, c'est `result` qui fixe ERROR sans quitter le bloc.
                if self.current_runtime_ctx.is_some() {
                    self.errors.push(SemaError::ReturnInsideRuntimeBlock { span: span.clone() });
                    return;
                }

                let ret_ty = self.current_ret.clone().unwrap_or(Type::Void);

                // `return` dans une fonction/méthode `message<T>` (générateur,
                // voir docs/roadmap.d/langage-emit-iterable.md) : un
                // générateur ne "retourne" jamais un `T`, il en `emit` — un
                // `return <valeur>` ici n'a pas de sens (le forwarding
                // implicite d'une valeur scalaire vers le consommateur du
                // message<T> n'est pas ce que fait `return`). Un `return`
                // SANS valeur reste valable : sortie anticipée du générateur
                // (plus aucune valeur produite après ce point), symétrique à
                // ce qu'un `return` sans valeur ferait dans n'importe quelle
                // fonction void.
                if let Type::Message(_) = &ret_ty {
                    if let Some(expr) = value {
                        let ty = self.infer_expr(expr);
                        self.errors.push(SemaError::ReturnTypeMismatch {
                            expected: "void (sortie anticipée d'un générateur — utilisez 'emit' pour produire une valeur)".into(),
                            found:    type_name(&ty),
                            span:     span.clone(),
                        });
                    }
                    return;
                }

                if let Some(expr) = value {
                    let ty = self.infer_expr(expr);
                    // `return x` est un point d'échappement au même titre
                    // qu'une affectation.
                    self.check_escape(expr);
                    self.check_message_scalar_consumption(expr, &ty, span);

                    // Exception pour les blocs runtime : main peut retourner ERROR (int) ou SUCCESS (bool)
                    // même si son type de retour est void
                    let is_runtime_return = if self.current_runtime_ctx.is_some() && ret_ty == Type::Void {
                        // Vérifier si c'est "return ERROR" ou "return SUCCESS"
                        if let Expr::Ident(name, _) = expr {
                            name == "ERROR" || name == "SUCCESS"
                        } else {
                            // Accepter aussi les expressions int dans main runtime
                            ty == Type::Int
                        }
                    } else {
                        false
                    };
                    
                    if !is_runtime_return && !types_compat(&ty, &ret_ty, &self.symbols) {
                        self.errors.push(SemaError::ReturnTypeMismatch {
                            expected: type_name(&ret_ty),
                            found:    type_name(&ty),
                            span:     span.clone(),
                        });
                    }
                } else if ret_ty != Type::Void {
                    self.errors.push(SemaError::ReturnTypeMismatch {
                        expected: type_name(&ret_ty),
                        found:    "void".into(),
                        span:     span.clone(),
                    });
                }
            }

            Stmt::Result { value, span } => {
                // `result` n'a de sens qu'à l'intérieur d'un bloc runtime.
                if self.current_runtime_ctx.is_none() {
                    self.errors.push(SemaError::ResultOutsideRuntimeBlock { span: span.clone() });
                    return;
                }

                let ret_ty = self.current_ret.clone().unwrap_or(Type::Void);
                if let Some(expr) = value {
                    let ty = self.infer_expr(expr);

                    // Un bloc runtime peut fixer ERROR (int) ou SUCCESS (bool) même si
                    // son type de retour "apparent" est void.
                    let is_runtime_result = if ret_ty == Type::Void {
                        if let Expr::Ident(name, _) = expr {
                            name == "ERROR" || name == "SUCCESS"
                        } else {
                            ty == Type::Int
                        }
                    } else {
                        false
                    };

                    if !is_runtime_result && !types_compat(&ty, &ret_ty, &self.symbols) {
                        self.errors.push(SemaError::ReturnTypeMismatch {
                            expected: type_name(&ret_ty),
                            found:    type_name(&ty),
                            span:     span.clone(),
                        });
                    }
                }
            }

            Stmt::Break { .. } | Stmt::Continue { .. } => {
                // break/continue sont valides dans les corps de boucle — pas de vérification sémantique supplémentaire
            }

            Stmt::Try { body, handlers, .. } => {
                self.check_block(body);
                for (idx, handler) in handlers.iter().enumerate() {
                    // Un handler catch-all (`on e { }`, sans `is`) filtre déjà
                    // tout : les handlers suivants, quels qu'ils soient, ne
                    // seraient jamais atteints (E24) — voir docs/EBNF.md §28.2,
                    // déjà documenté mais jamais imposé jusqu'ici.
                    if handler.class_filter.is_none() && idx + 1 < handlers.len() {
                        self.errors.push(SemaError::CatchAllNotLast { span: handler.span.clone() });
                    }
                    // `on e is X` : X doit être une classe connue (utilisateur
                    // ou exception builtin, toujours enregistrée — voir
                    // SymbolTable::new) — sinon ce handler est silencieusement
                    // mort, aucun `raise` ne peut jamais correspondre (E23).
                    if let Some(class_name) = &handler.class_filter {
                        if self.symbols.lookup_class(class_name).is_none() {
                            self.errors.push(SemaError::OnFilterClassNotFound {
                                name: class_name.clone(),
                                span: handler.span.clone(),
                            });
                        }
                    }
                    self.scopes.push();
                    // Le binding est de type mixed (type de l'erreur inconnu statiquement)
                    self.scopes.declare(
                        handler.binding.clone(),
                        LocalBinding { ty: Type::Mixed, mutable: false, span: handler.span.clone(), used: false, is_param: true, kind: VarKind::Var, consumed_used_at: None, resource_finalized: false, resource_contained: false },
                    );
                    self.check_block(&handler.body);
                    { let _u = self.scopes.pop_scope(&self.resource_classes); self.flush_warnings(_u); }
                }
            }

            Stmt::Raise { value, .. } => {
                let _ = self.infer_expr(value);
            }

            // `emit expr` — voir docs/roadmap.d/langage-emit-iterable.md.
            // N'a de sens que dans une fonction/méthode dont le type de
            // retour déclaré est `message<T>` : la valeur émise doit alors
            // être compatible avec `T` (même règle que `return`/`result`).
            Stmt::Emit { value, span } => {
                let ty = self.infer_expr(value);
                match self.current_ret.clone() {
                    Some(Type::Message(inner)) => {
                        if !types_compat(&ty, &inner, &self.symbols) {
                            self.errors.push(SemaError::ReturnTypeMismatch {
                                expected: type_name(&inner),
                                found:    type_name(&ty),
                                span:     span.clone(),
                            });
                        }
                    }
                    _ => {
                        self.errors.push(SemaError::EmitOutsideMessageFunction {
                            span: span.clone(),
                        });
                    }
                }
            }

            Stmt::Assign { target, value, span } => {
                let val_ty = self.infer_expr(value);
                // `target = value` : `value` peut être une `scoped`/
                // `consumed` qui s'échappe vers `target`.
                self.check_escape(value);
                match target {
                    Expr::Ident(name, _) => {
                        if let Some(binding) = self.scopes.lookup(name) {
                            if !binding.mutable {
                                self.errors.push(SemaError::InvalidAssign {
                                    name: name.clone(),
                                    span: span.clone(),
                                });
                            }
                            let _ = val_ty;
                        } else {
                            self.errors.push(SemaError::UndefinedSymbol {
                                name: name.clone(),
                                span: span.clone(),
                            });
                        }
                    }
                    Expr::Field { object, .. } => {
                        self.infer_expr(object);
                    }
                    Expr::Index { object, index, .. } => {
                        self.infer_expr(object);
                        self.infer_expr(index);
                    }
                    _ => {
                        self.errors.push(SemaError::InvalidAssign {
                            name: "cible invalide".into(),
                            span: span.clone(),
                        });
                    }
                }
            }
        }
    }

    // ── `message<T>` : consommation scalaire directe ─────────────────────────

    /// Vérifie la règle "au plus un `emit` atteignable hors boucle" d'un
    /// `message<T>` consommé directement en scalaire (`var x:T = truc()`,
    /// `return truc()`...) — voir `crate::sema::message_emit` et
    /// docs/roadmap.d/langage-emit-iterable.md. `for`/`Array::fromMessage`
    /// ne passent PAS par ici : ils restent valables dans tous les cas.
    ///
    /// `message<T>` n'étant jamais nommable, une valeur de ce type ne peut
    /// provenir que de l'appel lui-même (`expr`) : pas besoin de suivre un
    /// alias.
    fn check_message_scalar_consumption(&mut self, expr: &Expr, val_ty: &Type, span: &Span) {
        let Type::Message(_) = val_ty else { return };

        let unsafe_call = match expr {
            Expr::Call { callee, .. } => match callee.as_ref() {
                Expr::Ident(name, _) => self.symbols.lookup_function(name)
                    .map(|sig| (name.clone(), sig.message_emit_in_loop)),
                Expr::Field { object, field, .. } => {
                    let obj_ty = match object.as_ref() {
                        Expr::Ident(name, _) => self.scopes.lookup(name).map(|b| b.ty.clone()),
                        _ => None,
                    };
                    obj_ty.and_then(|ty| match ty {
                        Type::Named(class_name) => self.symbols.lookup_method_in_chain(&class_name, field)
                            .map(|sig| (field.clone(), sig.message_emit_in_loop)),
                        _ => None,
                    })
                }
                _ => None,
            },
            Expr::StaticCall { class, method, .. } => {
                let resolved_class = if class == "<self>" {
                    self.current_class.clone().unwrap_or_default()
                } else {
                    class.clone()
                };
                self.symbols.lookup_method_in_chain(&resolved_class, method)
                    .map(|sig| (format!("{}::{}", resolved_class, method), sig.message_emit_in_loop))
            }
            _ => None,
        };

        if let Some((name, true)) = unsafe_call {
            self.errors.push(SemaError::MessageUnsafeScalarConsumption {
                name,
                span: span.clone(),
            });
        }
    }

    // ── Propriété (`scoped`/`consumed`) : points d'échappement ───────────────

    /// Vérifie la règle d'échappement d'une `scoped`/`consumed` : appelé à
    /// chaque point où une expression fait sortir une valeur vers un binding
    /// qui survit à la portée de sa `scoped`/`consumed` source — affectation
    /// (`var y = x`, `y = x`, `champ = x`) ou `return x`.
    ///
    /// PAS un argument d'appel : contrairement à une affectation, le
    /// paramètre du côté du callee ne survit pas à l'appel — il meurt avec
    /// le retour de la fonction, exactement comme n'importe quel `var`
    /// aliasant un tableau aujourd'hui. Cloner systématiquement sur chaque
    /// argument casserait par ailleurs le sucre `Array::push(arr, x)`/
    /// `Map::set(m, k, v)` (méthodes statiques utilisées comme mutateurs :
    /// `arr` y est un vrai alias à muter en place, pas une valeur qui
    /// s'échappe) — confirmé par un cas concret : `Array::push(data, 99)`
    /// sur une `scoped data` ne mutait plus `data` du tout, la mutation
    /// atterrissant sur un clone jetable.
    ///
    /// Ne s'applique qu'aux identifiants simples référant à un binding local
    /// `scoped`/`consumed` — tout le reste (littéraux, appels, accès de
    /// champ...) produit de toute façon une valeur fraîche, rien à échapper.
    fn check_escape(&mut self, expr: &Expr) {
        let Expr::Ident(name, use_span) = expr else { return };
        let Some(b) = self.scopes.lookup(name) else { return };
        if b.kind == VarKind::Var {
            return;
        }
        match ownership_class_of(&b.ty, &self.resource_classes) {
            OwnershipClass::Value => {
                // OK : clonée automatiquement à l'échappement (chantier
                // clonage réel) — la source reste possédée et détruite
                // normalement à son propre point de destruction.
            }
            OwnershipClass::Resource | OwnershipClass::Thread => {
                self.errors.push(SemaError::ResourceEscape {
                    name: name.clone(),
                    class_name: type_name(&b.ty),
                    span: use_span.clone(),
                });
            }
            OwnershipClass::Unsupported => {
                // Comportement voulu, pas une omission : `scoped`/`consumed`
                // sur un type non pris en charge par ce chantier (primitif,
                // Function, union, mixed...) se comporte exactement comme
                // `var` — aucune vérification d'échappement, voir le
                // commentaire de `Stmt::Var` plus haut.
            }
        }
    }

    /// Vérifie les arguments d'un appel dont le callee résolu est
    /// `resolved_key` (`None` si le callee n'a pas pu être résolu vers une
    /// fonction/méthode/constructeur utilisateur connue — builtin, classe
    /// inconnue... : comportement inchangé, comme aujourd'hui, aucun de ces
    /// cas n'est vérifié) — voir `crate::sema::escape` pour la justification
    /// du traitement différent Resource/Thread vs Value ci-dessous.
    ///
    /// - `Resource`/`Thread` : TOUJOURS une erreur (`ResourceEscape`, déjà
    ///   utilisée pour affectation/`return` — sa formulation mentionnait
    ///   déjà "argument" sans que ce soit jamais vérifié). Aucun usage
    ///   légitime de "prêt" via argument n'existe pour ces types (ressources
    ///   utilisées uniquement via leurs propres méthodes) — contrairement à
    ///   `Value` ci-dessous, pas besoin de savoir si le callee retient
    ///   vraiment le paramètre.
    /// - `Value` (string/array/map/classe utilisateur) : seulement une
    ///   erreur (`ArgumentEscape`, E26) si `resolved_key` est un callable
    ///   CONNU dont ce paramètre précis est PROUVÉ échappant (voir
    ///   `escape::compute_escaping_params`) — préserve le sucre
    ///   `Array::push(arr, x)`/`Map::set(m, k, v)` (builtins, jamais résolus
    ///   ici, donc jamais vérifiés) et tout appel dont le callee ne retient
    ///   pas son paramètre.
    /// `allow_resource_use` : `true` UNIQUEMENT pour un appel dont TOUS les
    /// paramètres ressource sont, par construction, seulement "utilisés en
    /// place" (jamais retenus au-delà de l'appel) — cas de `HTTPRequest::*`,
    /// dont toutes les méthodes sont STATIQUES avec le handle passé en
    /// argument (`HTTPRequest::send(req)`), contrairement à `Mutex`/`SQLite`
    /// (méthodes D'INSTANCE, `m.lock()` — jamais un "argument" au sens de
    /// cette fonction, donc jamais concernées par ce carve-out). Sans cette
    /// exception, l'usage normal de `HTTPRequest` serait rejeté à tort comme
    /// un échappement — voir docs/roadmap.d/memoire-double-free-et-fuites-scoped.md.
    fn check_argument_escape(&mut self, args: &[Expr], resolved_key: Option<&str>, allow_resource_use: bool) {
        for (i, arg) in args.iter().enumerate() {
            let Expr::Ident(name, use_span) = arg else { continue };
            let Some(b) = self.scopes.lookup(name) else { continue };
            if b.kind == VarKind::Var {
                continue;
            }
            match ownership_class_of(&b.ty, &self.resource_classes) {
                OwnershipClass::Resource | OwnershipClass::Thread if !allow_resource_use => {
                    self.errors.push(SemaError::ResourceEscape {
                        name: name.clone(),
                        class_name: type_name(&b.ty),
                        span: use_span.clone(),
                    });
                }
                OwnershipClass::Resource | OwnershipClass::Thread => {}
                OwnershipClass::Value => {
                    if let Some(key) = resolved_key {
                        let escapes = self.escaping_params.get(key)
                            .and_then(|v| v.get(i))
                            .copied()
                            .unwrap_or(false);
                        if escapes {
                            self.errors.push(SemaError::ArgumentEscape {
                                name: name.clone(),
                                class_name: type_name(&b.ty),
                                callee: key.to_string(),
                                span: use_span.clone(),
                            });
                        }
                    }
                }
                OwnershipClass::Unsupported => {}
            }
        }
    }

    // ── Inférence de type des expressions ────────────────────────────────────

    pub fn infer_expr(&mut self, expr: &Expr) -> Type {
        match expr {
            Expr::Literal(lit, _) => literal_type(lit),

            Expr::SelfExpr(_) => {
                if let Some(cls) = &self.current_class {
                    Type::Named(cls.clone())
                } else {
                    Type::Mixed
                }
            }

            Expr::ParentExpr(_) => {
                if let Some(cls) = &self.current_class {
                    // Récupérer la classe parent
                    if let Some(parent_name) = self.symbols.lookup_parent_class(cls) {
                        Type::Named(parent_name)
                    } else {
                        Type::Mixed
                    }
                } else {
                    Type::Mixed
                }
            }

            Expr::Ident(name, span) => {
                // 1. variable locale
                if let Some(b) = self.scopes.lookup(name) {
                    let ty = b.ty.clone();
                    if let Err(first_use) = self.scopes.use_binding(name, span) {
                        self.errors.push(SemaError::ConsumedUsedTwice {
                            name: name.clone(),
                            first_use,
                            span: span.clone(),
                        });
                    }
                    return ty;
                }
                // 2. constante globale
                if let Some(ty) = self.symbols.lookup_const(name) {
                    return ty.clone();
                }
                // 3. nom de classe (utilisé comme type)
                if self.symbols.lookup_class(name).is_some() {
                    return Type::Named(name.clone());
                }
                // 4. référence à une fonction libre (sans appel)
                if let Some(sig) = self.symbols.lookup_function(name) {
                    // Construire le type Function avec les paramètres
                    let param_tys = sig.params.iter().map(|(_, ty)| ty.clone()).collect();
                    return Type::Function {
                        ret_ty: Box::new(sig.ret_ty.clone()),
                        param_tys,
                    };
                }
                self.errors.push(SemaError::UndefinedSymbol {
                    name: name.clone(),
                    span: span.clone(),
                });
                Type::Mixed
            }

            Expr::Field { object, field, span } => {
                let obj_ty = self.infer_expr(object);
                let cls_name = match type_class_name(&obj_ty) {
                    Some(n) => n,
                    None    => return Type::Mixed,
                };
                if let Some(info) = self.symbols.lookup_class(&cls_name) {
                    // Classe opaque (import non résolu) — accès permissif
                    if info.is_opaque { return Type::Mixed; }
                    // Cherche le champ en remontant la chaîne d'héritage
                    if let Some(f) = self.symbols.lookup_field_in_chain(&cls_name, field) {
                        return f.ty.clone();
                    }
                    // peut être une méthode sans appel
                    if self.symbols.lookup_method_in_chain(&cls_name, field).is_some() {
                        return Type::Mixed;
                    }
                    let _ = info;
                    self.errors.push(SemaError::FieldNotFound {
                        class: cls_name,
                        field: field.clone(),
                        span:  self.with_runtime_ctx(span),
                    });
                }
                Type::Mixed
            }

            Expr::Call { callee, args, span } => {
                // Appel indirect : variable locale de type Function
                if let Expr::Ident(name, _) = callee.as_ref() {
                    if let Some(b) = self.scopes.lookup(name) {
                        if let Type::Function { ret_ty, param_tys } = &b.ty {
                            let ret = ret_ty.as_ref().clone();
                            let param_tys_clone = param_tys.clone(); // Cloner pour éviter les problèmes de borrowing
                            if let Err(first_use) = self.scopes.use_binding(name, span) {
                                self.errors.push(SemaError::ConsumedUsedTwice {
                                    name: name.clone(),
                                    first_use,
                                    span: span.clone(),
                                });
                            }
                            
                            // Vérifier le nombre et les types des arguments si param_tys est défini
                            if !param_tys_clone.is_empty() {
                                if args.len() != param_tys_clone.len() {
                                    self.errors.push(SemaError::WrongArgCount {
                                        name: name.clone(),
                                        expected: param_tys_clone.len(),
                                        found: args.len(),
                                        span: span.clone(),
                                    });
                                } else {
                                    for (arg, expected_ty) in args.iter().zip(param_tys_clone.iter()) {
                                        let arg_ty = self.infer_expr(arg);
                                        if !types_compat(&arg_ty, expected_ty, &self.symbols) {
                                            self.errors.push(SemaError::TypeMismatch {
                                                expected: type_name(expected_ty),
                                                found: type_name(&arg_ty),
                                                span: span.clone(),
                                            });
                                        }
                                    }
                                }
                            } else {
                                // Ancienne syntaxe : inférer les arguments sans vérification
                                for arg in args { self.infer_expr(arg); }
                            }
                            return ret;
                        }
                    }
                }
                // Résolution : Ident direct → fonction libre
                if let Expr::Ident(name, _) = callee.as_ref() {
                    if let Some(sig) = self.symbols.lookup_function(name) {
                        // Vérification du nombre d'arguments avec support variadic et paramètres optionnels
                        let args_ok = if sig.has_variadic {
                            // Si variadic : accepte required_params_count ou plus
                            args.len() >= sig.required_params_count
                        } else {
                            // Si non-variadic : entre required_params_count et params.len()
                            args.len() >= sig.required_params_count && args.len() <= sig.params.len()
                        };
                        
                        if !args_ok {
                            let expected = if sig.has_variadic {
                                sig.required_params_count  // Minimum avec variadic
                            } else {
                                sig.required_params_count  // Minimum sans variadic
                            };
                            self.errors.push(SemaError::WrongArgCount {
                                name:     name.clone(),
                                expected,
                                found:    args.len(),
                                span:     span.clone(),
                            });
                        }
                        // Appel async : retourne Type::Int (le task handle opaque)
                        let ret = if sig.is_async { Type::Int } else { sig.ret_ty.clone() };
                        let resolved_key = if self.escaping_params.contains_key(name) {
                            Some(name.as_str())
                        } else {
                            None
                        };
                        self.check_argument_escape(args, resolved_key, false);
                        for arg in args {
                            let arg_ty = self.infer_expr(arg);
                            self.check_message_scalar_consumption(arg, &arg_ty, span);
                        }
                        return ret;
                    }

                }
                // Appel de méthode : Field { object, field } → méthode
                if let Expr::Field { object, field, span: fspan } = callee.as_ref() {
                    let obj_ty = self.infer_expr(object);

                    // Valeur d'un générique instancié (`List<int>`, ...) : résoudre
                    // la méthode dans la déclaration `generic`, avec substitution
                    // des paramètres de type par les arguments concrets de CETTE
                    // instance (`T` → `int` pour `List<int>`) — jusqu'ici, ce cas
                    // retombait silencieusement sur `Type::Mixed` sans la moindre
                    // vérification (voir docs/roadmap.d/langage-generiques.md).
                    if let Type::Generic { name: generic_name, args: type_args } = &obj_ty {
                        if let Some(ginfo) = self.symbols.lookup_generic(generic_name) {
                            if let Some(sig) = ginfo.methods.get(field) {
                                let expected_min = sig.required_params_count;
                                let expected_max = sig.params.len();
                                let args_ok = if sig.has_variadic {
                                    args.len() >= expected_min
                                } else {
                                    args.len() >= expected_min && args.len() <= expected_max
                                };
                                if !args_ok {
                                    self.errors.push(SemaError::WrongArgCount {
                                        name:     format!("{}::{}", generic_name, field),
                                        expected: expected_min,
                                        found:    args.len(),
                                        span:     span.clone(),
                                    });
                                }
                                for (i, arg) in args.iter().enumerate() {
                                    let arg_ty = self.infer_expr(arg);
                                    if let Some((_, param_ty)) = sig.params.get(i) {
                                        let expected_ty = substitute_type_params(param_ty, &ginfo.type_params, type_args);
                                        if expected_ty != Type::Mixed && arg_ty != Type::Mixed && !types_compat(&arg_ty, &expected_ty, &self.symbols) {
                                            self.errors.push(SemaError::TypeMismatch {
                                                expected: type_name(&expected_ty),
                                                found:    type_name(&arg_ty),
                                                span:     span.clone(),
                                            });
                                        }
                                    }
                                }
                                return substitute_type_params(&sig.ret_ty, &ginfo.type_params, type_args);
                            }
                            self.errors.push(SemaError::FieldNotFound {
                                class: generic_name.clone(),
                                field: field.clone(),
                                span:  self.with_runtime_ctx(fspan),
                            });
                        }
                        for a in args { self.infer_expr(a); }
                        return Type::Mixed;
                    }

                    // `expr.méthode(...)` où `expr` (typiquement le résultat
                    // d'un appel précédent chaîné, ex. `self.port(8080)`) est
                    // de type `void` — rejeté explicitement, pas juste
                    // silencieusement permissif comme les autres types sans
                    // classe associée (`Mixed`, `int`...) : `void` signifie
                    // ICI "aucune valeur produite", chaîner dessus n'a jamais
                    // de sens, quel que soit le champ appelé. Voir
                    // docs/roadmap.d/langage-appel-methode-sur-void-accepte.md
                    // — sans ce rejet, la suite de la chaîne manglait vers un
                    // symbole inexistant, ignoré silencieusement par le
                    // codegen (confirmé par reproduction :
                    // `self.port(8080).workers(4).rootPath(...)` compilait
                    // sans erreur, mais `workers`/`rootPath` n'étaient JAMAIS
                    // appelés).
                    if matches!(obj_ty, Type::Void) {
                        self.errors.push(SemaError::MethodCallOnVoid {
                            method: field.clone(),
                            span: self.with_runtime_ctx(fspan),
                        });
                        for a in args { self.infer_expr(a); }
                        return Type::Mixed;
                    }

                    let cls_name = match type_class_name(&obj_ty) {
                        Some(n) => n,
                        _ => { for a in args { self.infer_expr(a); } return Type::Mixed; }
                    };
                    // `t.join()`/`t.detach()` finalise une `scoped`/`consumed
                    // Thread` — voir OwnershipClass::Thread et pop_scope().
                    if cls_name == "Thread" && (field == "join" || field == "detach") {
                        if let Expr::Ident(recv_name, _) = object.as_ref() {
                            if self.scopes.mark_resource_finalized(recv_name) {
                                self.errors.push(SemaError::ThreadAlreadyFinalized {
                                    name: recv_name.clone(),
                                    span: fspan.clone(),
                                });
                            }
                        }
                    }
                    // `m.destroy()` (Mutex), `db.close()` (SQLite/MySQL/
                    // MariaDB) : même mécanisme que Thread ci-dessus,
                    // généralisé (E25) — un second appel referait une
                    // libération déjà faite côté runtime, SEGFAULT confirmé
                    // par reproduction pour un double `Mutex::destroy` — voir
                    // docs/roadmap.d/memoire-documentation-diagnostics.md.
                    let manual_finalizer = matches!(
                        (cls_name.as_str(), field.as_str()),
                        ("Mutex", "destroy") | ("SQLite", "close") | ("MySQL", "close") | ("MariaDB", "close")
                            | ("HTTPRequest", "close") | ("HTTPResponse", "closeResponse")
                    );
                    if manual_finalizer {
                        if let Expr::Ident(recv_name, _) = object.as_ref() {
                            if self.scopes.mark_resource_finalized(recv_name) {
                                self.errors.push(SemaError::ResourceAlreadyFinalized {
                                    name: recv_name.clone(),
                                    class_name: cls_name.clone(),
                                    method: field.clone(),
                                    span: fspan.clone(),
                                });
                            }
                        }
                        // `self.<champ>.close()`/`.destroy()`/`.closeResponse()` :
                        // un champ ressource est déjà fermé automatiquement
                        // par `__free_<Classe>` quand l'instance porteuse est
                        // détruite (voir `class_ownership::classify_field`) —
                        // un appel manuel ici referait TOUJOURS cette
                        // fermeture une seconde fois (à la différence de
                        // `ResourceAlreadyFinalized` ci-dessus, qui ne détecte
                        // qu'un second appel EXPLICITE : ici, le premier est
                        // déjà en trop, aucun suivi inter-méthodes n'étant
                        // possible pour distinguer un usage sûr).
                        if let Expr::Field { object: inner, field: field_name, .. } = object.as_ref() {
                            if matches!(inner.as_ref(), Expr::SelfExpr(_)) {
                                if let Some(class_name) = self.current_class.clone() {
                                    self.errors.push(SemaError::ManualCloseOnResourceField {
                                        class: class_name,
                                        field: field_name.clone(),
                                        ty_name: cls_name.clone(),
                                        method: field.clone(),
                                        span: fspan.clone(),
                                    });
                                }
                            }
                        }
                    }
                    if let Some(info) = self.symbols.lookup_class(&cls_name) {
                        // Classe opaque (import non résolu) — accès permissif
                        if info.is_opaque {
                            for a in args { self.infer_expr(a); }
                            return Type::Mixed;
                        }
                        // `HTTPResponse` n'a aucune méthode À ELLE : toutes
                        // les opérations sur une réponse (`status`/`body`/...)
                        // restent déclarées sur `HTTPRequest` (voir
                        // `src/builtins/httprequest.rs`) — chercher la
                        // méthode là plutôt que sur `HTTPResponse` lui-même,
                        // pour que `res.status()` (sucre d'instance)
                        // fonctionne malgré cette asymétrie. Sans garde-fou
                        // supplémentaire, ça permettrait aussi `res.send()`/
                        // `req.status()` (les deux existent bien sur
                        // `HTTPRequest`, mais avec un PREMIER PARAMÈTRE de
                        // l'autre type — confondre les deux passerait le
                        // mauvais pointeur à la fonction runtime, UB) : la
                        // liste ci-dessous distingue les méthodes "côté req"
                        // des méthodes "côté res", même check que
                        // `is_compatible` un peu plus bas pour JSON.
                        const HTTP_REQUEST_METHODS: &[&str] = &["setMethod", "setHeader", "setBody", "setTimeout", "send", "close"];
                        const HTTP_RESPONSE_METHODS: &[&str] = &["status", "body", "header", "headers", "ok", "isError", "error", "closeResponse"];
                        let http_receiver_ok = match cls_name.as_str() {
                            "HTTPRequest"  => HTTP_REQUEST_METHODS.contains(&field.as_str()),
                            "HTTPResponse" => HTTP_RESPONSE_METHODS.contains(&field.as_str()),
                            _ => true,
                        };
                        let method_owner: String = if cls_name == "HTTPResponse" {
                            self.symbols.local_name_for_builtin("HTTPRequest")
                        } else {
                            cls_name.clone()
                        };
                        if let Some(sig) = self.symbols.lookup_method_in_chain(&method_owner, field).filter(|_| http_receiver_ok) {
                            // Une méthode static ne peut pas être appelée sur une instance
                            // SAUF pour ces classes : les méthodes sont statiques mais utilisables
                            // comme méthodes d'instance sur les variables (ex: a.trim(), arr.len(), m.size(), data.encode(), req.close(), res.status()).
                            let allows_instance_sugar = matches!(cls_name.as_str(), "String" | "Array" | "Map" | "JSON" | "HTTPRequest" | "HTTPResponse");
                            if sig.is_static && !allows_instance_sugar {
                                self.errors.push(SemaError::StaticOnInstance {
                                    class:  cls_name.clone(),
                                    method: field.clone(),
                                    span:   fspan.clone(),
                                });
                            }

                            // Pour ces classes, ajuster le comptage des arguments :
                            // String::trim(s) a 1 paramètre, mais a.trim() n'en fournit 0
                            // Array::len(arr) a 1 paramètre, mais arr.len() n'en fournit 0
                            // Map::size(m) a 1 paramètre, mais m.size() n'en fournit 0
                            // JSON::encode(data) a 1 paramètre, mais data.encode() n'en fournit 0
                            // HTTPRequest::close(req) a 1 paramètre, mais req.close() n'en fournit 0
                            // car l'objet sera automatiquement passé comme premier argument
                            let (expected_min, expected_max) = if allows_instance_sugar && sig.is_static {
                                // Accepter N-1 arguments (le self est ajouté automatiquement)
                                let min = if sig.required_params_count > 0 {
                                    sig.required_params_count - 1
                                } else {
                                    0
                                };
                                let max = if sig.params.len() > 0 {
                                    sig.params.len() - 1
                                } else {
                                    0
                                };
                                (min, max)
                            } else {
                                (sig.required_params_count, sig.params.len())
                            };
                            
                            // Vérification du nombre d'arguments avec support variadic et paramètres optionnels
                            let args_ok = if sig.has_variadic {
                                args.len() >= expected_min
                            } else {
                                args.len() >= expected_min && args.len() <= expected_max
                            };
                            
                            if !args_ok {
                                let expected = if sig.has_variadic {
                                    expected_min
                                } else {
                                    expected_min
                                };
                                self.errors.push(SemaError::WrongArgCount {
                                    name:     format!("{}::{}", cls_name, field),
                                    expected,
                                    found:    args.len(),
                                    span:     span.clone(),
                                });
                            }
                            let ret = sig.ret_ty.clone();
                            let resolved_key = crate::sema::escape::resolve_user_callable(&self.class_members, &cls_name, field);
                            self.check_argument_escape(args, resolved_key.as_deref(), false);
                            for arg in args { self.infer_expr(arg); }
                            return ret;
                        }

                        // ── Méthodes JSON sur types primitifs ─────────────────────────────
                        // array/map.encode() → JSON::encode(obj)
                        // string.decode() / string.pretty() / string.minimize() → JSON::<method>(obj)
                        let is_json_method = matches!(field.as_str(), "encode" | "decode" | "pretty" | "minimize");
                        if is_json_method {
                            // JSON est maintenant toujours disponible (enregistré dans SymbolTable::new())
                            // pas besoin de vérifier l'import
                            
                            // Vérifier que le type est compatible
                            let is_compatible = match (field.as_str(), cls_name.as_str()) {
                                ("encode", "Array") => true,
                                ("encode", "Map") => true,
                                ("decode", "String") => true,
                                ("pretty", "String") => true,
                                ("minimize", "String") => true,
                                _ => false,
                            };
                            
                            if !is_compatible {
                                self.errors.push(SemaError::FieldNotFound {
                                    class: cls_name.clone(),
                                    field: field.clone(),
                                    span:  self.with_runtime_ctx(fspan),
                                });
                                for a in args { self.infer_expr(a); }
                                return Type::Mixed;
                            }
                            
                            // Résoudre depuis JSON
                            if let Some(json_info) = self.symbols.lookup_class("JSON") {
                                if let Some(sig) = json_info.methods.get(field) {
                                    // Ajuster le comptage des arguments (N-1 car l'objet est passé automatiquement)
                                    let expected_min = if sig.required_params_count > 0 {
                                        sig.required_params_count - 1
                                    } else {
                                        0
                                    };
                                    let expected_max = if sig.params.len() > 0 {
                                        sig.params.len() - 1
                                    } else {
                                        0
                                    };
                                    
                                    let args_ok = args.len() >= expected_min && args.len() <= expected_max;
                                    if !args_ok {
                                        self.errors.push(SemaError::WrongArgCount {
                                            name:     format!("JSON::{}", field),
                                            expected: expected_min,
                                            found:    args.len(),
                                            span:     span.clone(),
                                        });
                                    }
                                    
                                    let ret = sig.ret_ty.clone();
                                    for arg in args { self.infer_expr(arg); }
                                    return ret;
                                }
                            }
                        }
                        
                        let _ = info;
                        self.errors.push(SemaError::FieldNotFound {
                            class: cls_name,
                            field: field.clone(),
                            span:  self.with_runtime_ctx(fspan),
                        });
                    }
                }
                for arg in args { self.infer_expr(arg); }
                Type::Mixed
            }

            Expr::StaticCall { class, method, args, span } => {
                // Résoudre "<self>" vers la classe courante
                let resolved_class = if class == "<self>" {
                    match &self.current_class {
                        Some(c) => c.clone(),
                        None => {
                            self.errors.push(SemaError::SelfOutsideClass {
                                span: span.clone(),
                            });
                            for a in args { self.infer_expr(a); }
                            return Type::Mixed;
                        }
                    }
                } else {
                    class.clone()
                };

                // `Array::fromMessage(message<T>) -> array<T>` (voir §2 de
                // docs/roadmap.d/langage-emit-iterable.md) : draine TOUS les
                // `emit`, SANS la restriction "au plus un emit hors boucle"
                // (contrairement à toute autre consommation scalaire) — son
                // type de retour dépend dynamiquement de l'argument, jamais
                // un `FuncSig` fixe comme les autres méthodes `Array::*` —
                // traité entièrement à part, jamais enregistré dans les
                // builtins normaux.
                if resolved_class == "Array" && method == "fromMessage" {
                    if args.len() != 1 {
                        self.errors.push(SemaError::WrongArgCount {
                            name:     "Array::fromMessage".to_string(),
                            expected: 1,
                            found:    args.len(),
                            span:     span.clone(),
                        });
                        for a in args { self.infer_expr(a); }
                        return Type::Array(Box::new(Type::Mixed));
                    }
                    let arg_ty = self.infer_expr(&args[0]);
                    if let Type::Message(inner) = arg_ty {
                        return Type::Array(inner);
                    }
                    self.errors.push(SemaError::TypeMismatch {
                        expected: "message<T>".into(),
                        found:    type_name(&arg_ty),
                        span:     span.clone(),
                    });
                    return Type::Array(Box::new(Type::Mixed));
                }

                // Chercher la méthode dans la chaîne d'héritage
                if let Some(sig) = self.symbols.lookup_method_in_chain(&resolved_class, method) {
                    // Une méthode non-static ne peut pas être appelée via ::
                    if !sig.is_static {
                        self.errors.push(SemaError::NotStaticMethod {
                            class:  resolved_class.clone(),
                            method: method.clone(),
                            span:   span.clone(),
                        });
                    }
                    let ret = sig.ret_ty.clone();
                    // Vérification du nombre d'arguments avec support variadic et paramètres optionnels
                    let args_ok = if sig.has_variadic {
                        args.len() >= sig.required_params_count
                    } else {
                        args.len() >= sig.required_params_count && args.len() <= sig.params.len()
                    };
                    
                    if !args_ok {
                        let expected = if sig.has_variadic {
                            sig.required_params_count
                        } else {
                            sig.required_params_count
                        };
                        self.errors.push(SemaError::WrongArgCount {
                            name:     format!("{}::{}", resolved_class, method),
                            expected,
                            found:    args.len(),
                            span:     span.clone(),
                        });
                    }
                    let resolved_key = crate::sema::escape::resolve_user_callable(&self.class_members, &resolved_class, method);
                    let is_http_request_call = resolved_class == self.symbols.local_name_for_builtin("HTTPRequest");
                    self.check_argument_escape(args, resolved_key.as_deref(), is_http_request_call);
                    for arg in args {
                        let arg_ty = self.infer_expr(arg);
                        self.check_message_scalar_consumption(arg, &arg_ty, span);
                    }
                    // `HTTPRequest::close(req)`/`::closeResponse(res)` — même
                    // mécanisme que `m.destroy()`/`db.close()` ci-dessus
                    // (E25), mais l'argument est ici passé en ARGUMENT
                    // (appel statique), pas en receveur d'un appel d'instance
                    // — voir docs/roadmap.d/memoire-double-free-et-fuites-scoped.md.
                    let manual_finalizer_arg = if is_http_request_call && matches!(method.as_str(), "close" | "closeResponse") {
                        args.first()
                    } else {
                        None
                    };
                    if let Some(Expr::Ident(recv_name, _)) = manual_finalizer_arg {
                        if self.scopes.mark_resource_finalized(recv_name) {
                            self.errors.push(SemaError::ResourceAlreadyFinalized {
                                name: recv_name.clone(),
                                class_name: if method == "close" { "HTTPRequest".to_string() } else { "HTTPResponse".to_string() },
                                method: method.clone(),
                                span: span.clone(),
                            });
                        }
                    }
                    return ret;
                }
                for arg in args { self.infer_expr(arg); }
                Type::Mixed
            }

            Expr::StaticConst { class, name, span } => {
                // Résoudre "<self>"/"<parent>" vers la classe réelle AVANT
                // toute recherche — contrairement à Expr::StaticCall
                // (résolu dès l'entrée, voir plus haut), ce nœud cherchait
                // jusqu'ici sous le nom littéral "<self>"/"<parent>" (jamais
                // une classe enregistrée sous ce nom), donc `self::CONST`
                // échouait TOUJOURS avec "undefined symbol '<self>::CONST'",
                // y compris depuis l'intérieur du constructeur.
                let resolved_class = if class == "<self>" {
                    self.current_class.clone().unwrap_or_default()
                } else if class == "<parent>" {
                    self.current_class.as_deref()
                        .and_then(|c| self.symbols.lookup_parent_class(c))
                        .unwrap_or_default()
                } else {
                    class.clone()
                };
                // Classe opaque (import non résolu) — accès permissif
                if let Some(info) = self.symbols.lookup_class(&resolved_class) {
                    if info.is_opaque { return Type::Mixed; }
                }
                if let Some((ty, _)) = self.symbols.lookup_class_const(&resolved_class, name) {
                    return ty.clone();
                }
                // Référence à une méthode statique sans appel : ClassName::myStatic
                if let Some(sig) = self.symbols.lookup_method_in_chain(&resolved_class, name) {
                    if sig.is_static {
                        // Construire le type Function avec les paramètres
                        let param_tys = sig.params.iter().map(|(_, ty)| ty.clone()).collect();
                        return Type::Function {
                            ret_ty: Box::new(sig.ret_ty.clone()),
                            param_tys,
                        };
                    }
                }
                self.errors.push(SemaError::UndefinedSymbol {
                    name: format!("{}::{}", resolved_class, name),
                    span: span.clone(),
                });
                Type::Mixed
            }

            Expr::New { class, type_args, args, span } => {
                // Vérifier si c'est une classe ou un générique
                let is_class = self.symbols.lookup_class(class).is_some();
                let is_generic = self.symbols.lookup_generic(class).is_some();
                
                if !is_class && !is_generic {
                    self.errors.push(SemaError::NotAClass {
                        name: class.clone(),
                        span: self.with_runtime_ctx(span),
                    });
                }
                
                // Typecheck lazy de la classe si elle n'a pas déjà été typecheckée
                if is_class && !self.checked_classes.contains(class) {
                    if let Some(prog) = self.program {
                        if let Some(class_decl) = prog.classes.iter().find(|c| &c.name == class) {
                            // Le typecheck d'une classe ne doit jamais hériter du contexte
                            // "bloc runtime" du site d'appel (ex: `use Foo()` écrit dans un
                            // `init { }`) : les méthodes de la classe (y compris une méthode
                            // nommée `init`, le constructeur) ne sont pas elles-mêmes dans
                            // ce bloc runtime.
                            let saved_ctx = self.current_runtime_ctx.take();
                            self.check_class(class_decl);
                            self.current_runtime_ctx = saved_ctx;
                        }
                    }
                }
                
                // Arité des arguments de type : entre le nombre de paramètres
                // sans valeur par défaut et le nombre total de paramètres
                // déclarés par `generic Foo<T, U=default>` (les paramètres
                // avec défaut sont optionnels à l'instanciation).
                if is_generic {
                    if let Some(generic_info) = self.symbols.lookup_generic(class) {
                        let expected_max = generic_info.type_params.len();
                        let expected_min = generic_info.type_params.iter()
                            .filter(|p| p.default.is_none())
                            .count();
                        let found = type_args.len();
                        if found < expected_min || found > expected_max {
                            self.errors.push(SemaError::GenericArityMismatch {
                                name: class.clone(),
                                expected_min,
                                expected_max,
                                found,
                                span: self.with_runtime_ctx(span),
                            });
                        }
                    }
                }

                let resolved_key = crate::sema::escape::resolve_user_callable(&self.class_members, class, "init");
                self.check_argument_escape(args, resolved_key.as_deref(), false);
                for arg in args { self.infer_expr(arg); }

                // Si c'est un générique avec type_args, retourner Type::Generic
                if is_generic && !type_args.is_empty() {
                    Type::Generic {
                        name: class.clone(),
                        args: type_args.clone()
                    }
                } else if is_generic && type_args.is_empty() {
                    // Arité déjà signalée ci-dessus si nécessaire (found=0) —
                    // on retombe sur Mixed pour ne pas propager une cascade
                    // d'erreurs de type incohérentes en aval.
                    Type::Mixed
                } else {
                    Type::Named(class.clone())
                }
            }

            Expr::Binary { op, left, right, span } => {
                let lt = self.infer_expr(left);
                let rt = self.infer_expr(right);
                binary_result_type(op, &lt, &rt, span, &mut self.errors, &self.symbols)
            }

            Expr::Unary { op, operand, span } => {
                let ty = self.infer_expr(operand);
                match op {
                    UnaryOp::Not => {
                        if !types_compat(&ty, &Type::Bool, &self.symbols) {
                            self.errors.push(SemaError::TypeMismatch {
                                expected: "bool".into(),
                                found:    type_name(&ty),
                                span:     span.clone(),
                            });
                        }
                        Type::Bool
                    }
                    UnaryOp::Neg => ty,
                }
            }

            // `i++`/`++i`/`i--`/`--i` — voir docs/roadmap.d/langage-increment-decrement.md.
            // Même règle de FORME de cible que `Stmt::Assign` (ligne ~786 :
            // Ident/Field/Index uniquement), plus une validation de TYPE
            // (int/float uniquement) absente de `Stmt::Assign`. Un seul appel
            // à `infer_expr(target)` (jamais deux) : pour un `Ident`, il fait
            // déjà le lookup ET `use_binding` (suivi `consumed` — un second
            // appel compterait une seconde lecture qui n'existe pas) ; pour
            // `Field`/`Index`, il infère déjà `object`/`index` en interne. La
            // vérification de mutabilité, elle, n'est PAS faite par une
            // lecture normale : `self.scopes.lookup` seul (sans passer par
            // `infer_expr`) reste donc nécessaire en plus, sans doublon.
            Expr::IncDec { target, span, .. } => {
                match target.as_ref() {
                    Expr::Ident(name, _) => {
                        if let Some(binding) = self.scopes.lookup(name) {
                            if !binding.mutable {
                                self.errors.push(SemaError::InvalidAssign {
                                    name: name.clone(),
                                    span: span.clone(),
                                });
                            }
                        }
                        // Absent : `infer_expr(target)` ci-dessous rapporte
                        // déjà `UndefinedSymbol` — pas la peine de dupliquer.
                    }
                    Expr::Field { .. } | Expr::Index { .. } => {}
                    _ => {
                        self.errors.push(SemaError::InvalidAssign {
                            name: "cible invalide".into(),
                            span: span.clone(),
                        });
                        return Type::Int;
                    }
                }
                let target_ty = self.infer_expr(target);
                if !matches!(target_ty, Type::Int | Type::Float) {
                    self.errors.push(SemaError::IncDecInvalidType {
                        found: type_name(&target_ty),
                        span:  span.clone(),
                    });
                }
                target_ty
            }

            Expr::Array { elements, .. } => {
                if elements.is_empty() {
                    return Type::Array(Box::new(Type::Mixed));
                }
                let elem_ty = self.infer_expr(&elements[0]);
                for e in &elements[1..] { self.infer_expr(e); }
                Type::Array(Box::new(elem_ty))
            }

            Expr::Range { start, end, .. } => {
                self.infer_expr(start);
                self.infer_expr(end);
                Type::Array(Box::new(Type::Int))
            }

            Expr::Match { subject, arms, .. } => {
                self.infer_expr(subject);
                let mut result = Type::Mixed;
                for arm in arms {
                    result = self.infer_expr(&arm.body);
                }
                result
            }

            Expr::Map { entries, .. } => {
                if entries.is_empty() {
                    return Type::Map(Box::new(Type::Mixed), Box::new(Type::Mixed));
                }
                let key_ty = self.infer_expr(&entries[0].0);
                let val_ty = self.infer_expr(&entries[0].1);
                for (k, v) in &entries[1..] {
                    self.infer_expr(k);
                    self.infer_expr(v);
                }
                Type::Map(Box::new(key_ty), Box::new(val_ty))
            }

            Expr::Index { object, index, .. } => {
                let obj_ty = self.infer_expr(object);
                self.infer_expr(index);
                match &obj_ty {
                    Type::Array(inner) => *inner.clone(),
                    Type::Map(_, val)  => *val.clone(),
                    _                  => Type::Mixed,
                }
            }

            Expr::Template { parts, .. } => {
                for part in parts {
                    if let TemplatePartExpr::Expr(e) = part {
                        self.infer_expr(e);
                    }
                }
                Type::String
            }

            Expr::Nameless { params, ret_ty, body, span: _ } => {
                // Ouvre un scope pour les paramètres de la closure
                self.scopes.push();
                for p in params {
                    self.scopes.declare(
                        p.name.clone(),
                        LocalBinding { ty: p.ty.clone(), mutable: false, span: p.span.clone(), used: false, is_param: true, kind: VarKind::Var, consumed_used_at: None, resource_finalized: false, resource_contained: false },
                    );
                }
                // Sauvegarder current_ret et le remplacer par le type de retour de la closure
                // pour que les `return` internes soient vérifiés contre le bon type
                let saved_ret = self.current_ret.take();
                let closure_ret = ret_ty.as_ref().cloned().unwrap_or(Type::Void);
                self.current_ret = Some(closure_ret.clone());
                self.check_block(body);
                self.current_ret = saved_ret;
                { let _u = self.scopes.pop_scope(&self.resource_classes); self.flush_warnings(_u); }
                
                // Construire le type Function avec les paramètres
                let param_tys = params.iter().map(|p| p.ty.clone()).collect();
                Type::Function {
                    ret_ty: Box::new(closure_ret),
                    param_tys,
                }
            }

            Expr::IsCheck { expr, ty: _, span: _ } => {
                // Test de type runtime : `val is int` retourne toujours bool
                self.infer_expr(expr);
                Type::Bool
            }

            Expr::Resolve { expr, .. } => {
                self.infer_expr(expr);
                // Retrouver le type de retour original de la fonction async
                let orig_ty: Option<Type> = match expr.as_ref() {
                    Expr::Ident(var_name, _) => {
                        self.async_var_funcs
                            .get(var_name)
                            .and_then(|fn_name| self.symbols.lookup_function(fn_name))
                            .map(|sig| sig.ret_ty.clone())
                    }
                    Expr::Call { callee, .. } => {
                        if let Expr::Ident(fn_name, _) = callee.as_ref() {
                            self.symbols
                                .lookup_function(fn_name)
                                .filter(|sig| sig.is_async)
                                .map(|sig| sig.ret_ty.clone())
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                orig_ty.unwrap_or(Type::Int)
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Extrait le nom de classe depuis un type Named, Qualified, ou Union (premier Named trouvé).
fn type_class_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Named(n)         => Some(n.clone()),
        Type::Qualified(parts) => parts.last().cloned(),
        Type::Union(variants)  => variants.iter().find_map(type_class_name),
        // Les variables string héritent automatiquement des méthodes de String
        Type::String           => Some("String".into()),
        // Les variables array héritent automatiquement des méthodes de Array
        Type::Array(_)         => Some("Array".into()),
        // Les variables map héritent automatiquement des méthodes de Map
        Type::Map(_, _)        => Some("Map".into()),
        _                      => None,
    }
}

/// Remplace chaque paramètre de type d'un `generic` (`T`, `K`, `V`, ...) par
/// son type concret pour une instance donnée — ex : `T` → `int` pour
/// `List<int>`. Un paramètre de type sans argument fourni au-delà de `args`
/// utilise sa valeur par défaut si elle existe (`generic Cache<K, V=string>`),
/// sinon `Type::Mixed` (arité déjà signalée par ailleurs si incorrecte — E21).
/// `Type::Named(n)` où `n` n'est PAS un nom de paramètre de type (une vraie
/// classe) traverse inchangé.
fn substitute_type_params(ty: &Type, params: &[TypeParam], args: &[Type]) -> Type {
    let substituted_named = |n: &str| -> Option<Type> {
        params.iter().position(|p| p.name == n).map(|i| {
            args.get(i).cloned()
                .or_else(|| params[i].default.clone())
                .unwrap_or(Type::Mixed)
        })
    };
    match ty {
        Type::Named(n) => substituted_named(n).unwrap_or_else(|| ty.clone()),
        Type::Array(inner) => Type::Array(Box::new(substitute_type_params(inner, params, args))),
        Type::Map(k, v) => Type::Map(
            Box::new(substitute_type_params(k, params, args)),
            Box::new(substitute_type_params(v, params, args)),
        ),
        Type::Union(variants) => Type::Union(
            variants.iter().map(|v| substitute_type_params(v, params, args)).collect()
        ),
        Type::Generic { name, args: inner_args } => Type::Generic {
            name: name.clone(),
            args: inner_args.iter().map(|a| substitute_type_params(a, params, args)).collect(),
        },
        Type::Function { ret_ty, param_tys } => Type::Function {
            ret_ty: Box::new(substitute_type_params(ret_ty, params, args)),
            param_tys: param_tys.iter().map(|p| substitute_type_params(p, params, args)).collect(),
        },
        _ => ty.clone(),
    }
}

fn literal_type(lit: &Literal) -> Type {
    match lit {
        Literal::Int(_)    => Type::Int,
        Literal::Float(_)  => Type::Float,
        Literal::String(_) => Type::String,
        Literal::Bool(_)   => Type::Bool,
        Literal::Null      => Type::Null,
    }
}

pub fn type_name(ty: &Type) -> String {
    match ty {
        Type::Int              => "int".into(),
        Type::Float            => "float".into(),
        Type::String           => "string".into(),
        Type::Bool             => "bool".into(),
        Type::Mixed            => "mixed".into(),
        Type::Void             => "void".into(),
        Type::Null             => "null".into(),
        Type::Named(n)         => n.clone(),
        Type::Qualified(parts) => parts.join("."),
        Type::Array(inner)     => format!("{}[]", type_name(inner)),
        Type::Map(k, v)        => format!("map<{},{}>", type_name(k), type_name(v)),
        Type::Message(inner)   => format!("message<{}>", type_name(inner)),
        Type::Generic { name, args } => {
            let type_args = args.iter().map(type_name).collect::<Vec<_>>().join(", ");
            format!("{}<{}>", name, type_args)
        }
        Type::Union(variants)  => variants.iter().map(type_name).collect::<Vec<_>>().join("|"),
        Type::Function { ret_ty, param_tys } => {
            let params = param_tys.iter().map(type_name).collect::<Vec<_>>().join(", ");
            format!("Function<{}({})>", type_name(ret_ty), params)
        }
    }
}

/// Compatibilité laxiste : `mixed` accepte tout, `null` compatible avec tout
/// type référence.
///
/// `symbols` permet de reconnaître une affectation POLYMORPHE réelle entre
/// deux types nommés (`Type::Named`) : une instance de classe compatible
/// avec le type déclaré d'une classe PARENTE (`var s:Shape = use Circle()`
/// où `Circle extends Shape`) ou d'une INTERFACE qu'elle implémente (`var
/// d:Drawable = use Circle()`) — voir `SymbolTable::class_matches`. Avant ce
/// mécanisme, `Type::Named(a) vs Type::Named(b)` ne passait que par l'égalité
/// stricte du catch-all final (`found == expected`), rejetant purement et
/// simplement TOUTE affectation polymorphe, même la plus basique (héritage
/// de classe) — voir docs/roadmap.d/langage-interfaces.md.
pub fn types_compat(found: &Type, expected: &Type, symbols: &SymbolTable) -> bool {
    if matches!(found, Type::Mixed) || matches!(expected, Type::Mixed) {
        return true;
    }
    // Les unions sont vérifiés en premier (avant le cas null)
    // union en position "found" : compatible si l'une des variantes est compatible avec expected
    if let Type::Union(variants) = found {
        return variants.iter().any(|v| types_compat(v, expected, symbols));
    }
    // union en position "expected" : compatible si found est compatible avec au moins une variante
    if let Type::Union(variants) = expected {
        return variants.iter().any(|v| types_compat(found, v, symbols));
    }
    // null est compatible avec tout type référence (string, objet, tableau, map)
    if matches!(found, Type::Null) {
        return matches!(expected,
            Type::String | Type::Named(_) | Type::Array(_) | Type::Map(..) | Type::Null
        );
    }
    // `message<T>` consommé en scalaire : compatible avec tout ce que `T`
    // accepte (voir docs/roadmap.d/langage-emit-iterable.md) — la règle "au
    // plus un `emit` hors boucle" est vérifiée séparément, PAS ici (voir
    // `check_message_scalar_consumption`, qui a besoin de l'expression
    // d'origine et pas seulement des types).
    if let Type::Message(inner) = found {
        return types_compat(inner, expected, symbols);
    }
    match (found, expected) {
        (Type::Named(f), Type::Named(e)) => {
            f == e || symbols.class_matches(f, e)
        }
        (Type::Array(f), Type::Array(e)) => types_compat(f, e, symbols),
        (Type::Map(fk, fv), Type::Map(ek, ev)) =>
            types_compat(fk, ek, symbols) && types_compat(fv, ev, symbols),
        (
            Type::Function { ret_ty: f_ret, param_tys: f_params },
            Type::Function { ret_ty: e_ret, param_tys: e_params }
        ) => {
            // Le type de retour doit être compatible
            if !types_compat(f_ret, e_ret, symbols) {
                return false;
            }
            // Les paramètres doivent correspondre exactement
            if f_params.len() != e_params.len() {
                return false;
            }
            for (fp, ep) in f_params.iter().zip(e_params.iter()) {
                if !types_compat(fp, ep, symbols) {
                    return false;
                }
            }
            true
        }
        _ => found == expected,
    }
}

fn binary_result_type(
    op:     &BinOp,
    lt:     &Type,
    rt:     &Type,
    span:   &Span,
    errors: &mut Vec<SemaError>,
    symbols: &SymbolTable,
) -> Type {
    match op {
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
            // Concaténation `+` : strictement string + string → string.
            // Mélanger un string avec un autre type est une erreur de
            // compilation (E20) — seul un template string ou une conversion
            // explicite (Convert::*ToStr) produit ce résultat. `mixed`
            // échappe à cette vérification statique (comme
            // comparable_types/orderable_types) faute d'information
            // disponible à la compilation.
            if op == &BinOp::Add
                && (matches!(lt, Type::String) || matches!(rt, Type::String))
            {
                if matches!(lt, Type::Mixed) || matches!(rt, Type::Mixed) {
                    return Type::String;
                }
                if matches!(lt, Type::String) && matches!(rt, Type::String) {
                    return Type::String;
                }
                errors.push(SemaError::StringConcatMismatch {
                    left:  type_name(lt),
                    right: type_name(rt),
                    span:  span.clone(),
                });
                return Type::String;
            }
            if types_compat(lt, rt, symbols) { lt.clone() } else {
                errors.push(SemaError::TypeMismatch {
                    expected: type_name(lt),
                    found:    type_name(rt),
                    span:     span.clone(),
                });
                lt.clone()
            }
        }
        BinOp::Equal | BinOp::NotEqual => {
            if !comparable_types(lt, rt, symbols) {
                errors.push(SemaError::IncomparableTypes {
                    op:    op_name(op),
                    left:  type_name(lt),
                    right: type_name(rt),
                    span:  span.clone(),
                });
            }
            Type::Bool
        }
        BinOp::Smaller | BinOp::Greater | BinOp::SmallerOrEqual | BinOp::GreaterOrEqual => {
            if !orderable_types(lt, rt) {
                errors.push(SemaError::IncomparableTypes {
                    op:    op_name(op),
                    left:  type_name(lt),
                    right: type_name(rt),
                    span:  span.clone(),
                });
            }
            Type::Bool
        }
        BinOp::And | BinOp::Or => Type::Bool,
    }
}

fn op_name(op: &BinOp) -> String {
    match op {
        BinOp::Equal          => "equal",
        BinOp::NotEqual        => "not equal",
        BinOp::Smaller         => "smaller",
        BinOp::Greater          => "greater",
        BinOp::SmallerOrEqual   => "smaller or equal",
        BinOp::GreaterOrEqual   => "greater or equal",
        _ => "?",
    }.to_string()
}

fn is_numeric(t: &Type) -> bool {
    matches!(t, Type::Int | Type::Float)
}

/// `equal` / `not equal` : vérifiable à la compilation dès que les deux types
/// sont statiquement connus. `int` et `float` sont l'unique paire compatible
/// malgré des types nominaux différents (widening numérique explicite lors du
/// lowering). `mixed` ne peut pas être vérifié statiquement — la comparaison
/// est alors déléguée à un contrôle de type au runtime (voir lower::expr::lower).
fn comparable_types(lt: &Type, rt: &Type, symbols: &SymbolTable) -> bool {
    if matches!(lt, Type::Mixed) || matches!(rt, Type::Mixed) {
        return true;
    }
    if is_numeric(lt) && is_numeric(rt) {
        return true;
    }
    types_compat(lt, rt, symbols) || types_compat(rt, lt, symbols)
}

/// `smaller` / `greater` / `smaller or equal` / `greater or equal` : un ordre
/// n'a de sens que pour des valeurs numériques (int/float, y compris mélangés).
/// `mixed` reste autorisé (vérifié au runtime) faute d'information statique.
fn orderable_types(lt: &Type, rt: &Type) -> bool {
    if matches!(lt, Type::Mixed) || matches!(rt, Type::Mixed) {
        return true;
    }
    is_numeric(lt) && is_numeric(rt)
}
