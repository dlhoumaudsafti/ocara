use crate::ast::*;
use crate::sema::error::{SemaError, SemaWarning};
use crate::sema::scope::{LocalBinding, ScopeStack};
use crate::sema::symbols::SymbolTable;
use crate::token::Span;

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
        }
    }

    // ── Point d'entrée ───────────────────────────────────────────────────────

    pub fn check_program(&mut self, program: &Program) {
        // Fonctions libres
        for func in &program.functions {
            self.check_func(func);
        }
        // Classes
        for class in &program.classes {
            self.check_class(class);
        }
    }

    // ── Fonction ─────────────────────────────────────────────────────────────

    fn check_func(&mut self, func: &FuncDecl) {
        self.scopes.push();
        self.current_ret = Some(func.ret_ty.clone());

        for param in &func.params {
            self.scopes.declare(
                param.name.clone(),
                LocalBinding { ty: param.ty.clone(), mutable: false, span: param.span.clone(), used: false, is_param: true },
            );
        }

        self.check_block(&func.body);
        { let _u = self.scopes.pop_with_warnings(); self.flush_warnings(_u); }
        self.current_ret = None;
    }

    // ── Classe ───────────────────────────────────────────────────────────────

    fn check_class(&mut self, class: &ClassDecl) {
        self.current_class = Some(class.name.clone());

        for member in &class.members {
            match member {
                ClassMember::Method { decl, .. } => self.check_func(decl),
                ClassMember::Constructor { params, body, .. } => {
                    self.scopes.push();
                    self.current_ret = Some(Type::Void);
                    for p in params {
                        self.scopes.declare(
                            p.name.clone(),
                            LocalBinding { ty: p.ty.clone(), mutable: false, span: p.span.clone(), used: false, is_param: true },
                        );
                    }
                    self.check_block(body);
                    { let _u = self.scopes.pop_with_warnings(); self.flush_warnings(_u); }
                    self.current_ret = None;
                }
                ClassMember::Const { ty, value, span, .. } => {
                    let val_ty = self.infer_expr(value);
                    if !types_compat(&val_ty, ty) {
                        self.errors.push(SemaError::TypeMismatch {
                            expected: type_name(ty),
                            found:    type_name(&val_ty),
                            span:     span.clone(),
                        });
                    }
                }
                ClassMember::Field { .. } => {}
            }
        }

        self.current_class = None;
    }

    // ── Block ─────────────────────────────────────────────────────────────────

    fn check_block(&mut self, block: &Block) {
        self.scopes.push();
        for stmt in &block.stmts {
            self.check_stmt(stmt);
        }
        { let _u = self.scopes.pop_with_warnings(); self.flush_warnings(_u); }
    }

    /// Convertit les variables inutilisées retournées par pop_with_warnings en SemaWarning.
    fn flush_warnings(&mut self, unused: Vec<crate::sema::scope::UnusedVar>) {
        for u in unused {
            self.warnings.push(SemaWarning::UnusedVariable { name: u.name, span: u.span });
        }
    }

    // ── Statement ────────────────────────────────────────────────────────────

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Var { name, ty, value, mutable, span } => {
                let val_ty = self.infer_expr(value);
                if !types_compat(&val_ty, ty) {
                    self.errors.push(SemaError::TypeMismatch {
                        expected: type_name(ty),
                        found:    type_name(&val_ty),
                        span:     span.clone(),
                    });
                }
                if !self.scopes.declare(
                    name.clone(),
                    LocalBinding { ty: ty.clone(), mutable: *mutable, span: span.clone(), used: false, is_param: false },
                ) {
                    self.errors.push(SemaError::DuplicateSymbol {
                        name: name.clone(),
                        span: span.clone(),
                    });
                }
            }

            Stmt::Const { name, ty, value, span } => {
                let val_ty = self.infer_expr(value);
                if !types_compat(&val_ty, ty) {
                    self.errors.push(SemaError::TypeMismatch {
                        expected: type_name(ty),
                        found:    type_name(&val_ty),
                        span:     span.clone(),
                    });
                }
                if !self.scopes.declare(
                    name.clone(),
                    LocalBinding { ty: ty.clone(), mutable: false, span: span.clone(), used: false, is_param: false },
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
                if !types_compat(&cond_ty, &Type::Bool) {
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
                if !types_compat(&cond_ty, &Type::Bool) {
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
                // L'itérateur doit être un range (int) ou un tableau
                let elem_ty = match &iter_ty {
                    Type::Array(inner) => *inner.clone(),
                    Type::Int          => Type::Int, // range produit des int
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
                self.scopes.declare(var.clone(), LocalBinding { ty: elem_ty, mutable: false, span: span.clone(), used: false, is_param: true });
                self.check_block(body);
                { let _u = self.scopes.pop_with_warnings(); self.flush_warnings(_u); }
            }

            Stmt::ForMap { key, value, iter, body, span } => {
                self.infer_expr(iter);
                self.scopes.push();
                self.scopes.declare(key.clone(),   LocalBinding { ty: Type::Mixed, mutable: false, span: span.clone(), used: false, is_param: true });
                self.scopes.declare(value.clone(), LocalBinding { ty: Type::Mixed, mutable: false, span: span.clone(), used: false, is_param: true });
                self.check_block(body);
                { let _u = self.scopes.pop_with_warnings(); self.flush_warnings(_u); }
            }

            Stmt::Return { value, span } => {
                let ret_ty = self.current_ret.clone().unwrap_or(Type::Void);
                if let Some(expr) = value {
                    let ty = self.infer_expr(expr);
                    if !types_compat(&ty, &ret_ty) {
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

            Stmt::Break { .. } | Stmt::Continue { .. } => {
                // break/continue sont valides dans les corps de boucle — pas de vérification sémantique supplémentaire
            }

            Stmt::Try { body, handlers, .. } => {
                self.check_block(body);
                for handler in handlers {
                    self.scopes.push();
                    // Le binding est de type mixed (type de l'erreur inconnu statiquement)
                    self.scopes.declare(
                        handler.binding.clone(),
                        LocalBinding { ty: Type::Mixed, mutable: false, span: handler.span.clone(), used: false, is_param: true },
                    );
                    self.check_block(&handler.body);
                    { let _u = self.scopes.pop_with_warnings(); self.flush_warnings(_u); }
                }
            }

            Stmt::Raise { value, .. } => {
                let _ = self.infer_expr(value);
            }

            Stmt::Assign { target, value, span } => {
                let val_ty = self.infer_expr(value);
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

            Expr::Ident(name, span) => {
                // 1. variable locale
                if let Some(b) = self.scopes.lookup(name) {
                    let ty = b.ty.clone();
                    self.scopes.mark_used(name);
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
                self.errors.push(SemaError::UndefinedSymbol {
                    name: name.clone(),
                    span: span.clone(),
                });
                Type::Mixed
            }

            Expr::Field { object, field, span } => {
                let obj_ty = self.infer_expr(object);
                let cls_name = match &obj_ty {
                    Type::Named(n)         => n.clone(),
                    Type::Qualified(parts) => parts.last().cloned().unwrap_or_default(),
                    _ => return Type::Mixed,
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
                        span:  span.clone(),
                    });
                }
                Type::Mixed
            }

            Expr::Call { callee, args, span } => {
                // Résolution : Ident direct → fonction libre
                if let Expr::Ident(name, _) = callee.as_ref() {
                    if let Some(sig) = self.symbols.lookup_function(name) {
                        let expected = sig.params.len();
                        if args.len() != expected {
                            self.errors.push(SemaError::WrongArgCount {
                                name:     name.clone(),
                                expected,
                                found:    args.len(),
                                span:     span.clone(),
                            });
                        }
                        let ret = sig.ret_ty.clone();
                        for arg in args { self.infer_expr(arg); }
                        return ret;
                    }

                }
                // Appel de méthode : Field { object, field } → méthode
                if let Expr::Field { object, field, span: fspan } = callee.as_ref() {
                    let obj_ty = self.infer_expr(object);
                    let cls_name = match &obj_ty {
                        Type::Named(n) => n.clone(),
                        Type::Qualified(parts) => parts.last().cloned().unwrap_or_default(),
                        _ => { for a in args { self.infer_expr(a); } return Type::Mixed; }
                    };
                    if let Some(info) = self.symbols.lookup_class(&cls_name) {
                        // Classe opaque (import non résolu) — accès permissif
                        if info.is_opaque {
                            for a in args { self.infer_expr(a); }
                            return Type::Mixed;
                        }
                        if let Some(sig) = self.symbols.lookup_method_in_chain(&cls_name, field) {
                            // Une méthode static ne peut pas être appelée sur une instance
                            if sig.is_static {
                                self.errors.push(SemaError::StaticOnInstance {
                                    class:  cls_name.clone(),
                                    method: field.clone(),
                                    span:   fspan.clone(),
                                });
                            }
                            let expected = sig.params.len();
                            if args.len() != expected {
                                self.errors.push(SemaError::WrongArgCount {
                                    name:     format!("{}::{}", cls_name, field),
                                    expected,
                                    found:    args.len(),
                                    span:     span.clone(),
                                });
                            }
                            let ret = sig.ret_ty.clone();
                            for arg in args { self.infer_expr(arg); }
                            return ret;
                        }
                        let _ = info;
                        self.errors.push(SemaError::FieldNotFound {
                            class: cls_name,
                            field: field.clone(),
                            span:  fspan.clone(),
                        });
                    }
                }
                for arg in args { self.infer_expr(arg); }
                Type::Mixed
            }

            Expr::StaticCall { class, method, args, span } => {
                if let Some(info) = self.symbols.lookup_class(class) {
                    if let Some(sig) = info.methods.get(method) {
                        // Une méthode non-static ne peut pas être appelée via ::
                        if !sig.is_static {
                            self.errors.push(SemaError::NotStaticMethod {
                                class:  class.clone(),
                                method: method.clone(),
                                span:   span.clone(),
                            });
                        }
                        let ret = sig.ret_ty.clone();
                        if args.len() != sig.params.len() {
                            self.errors.push(SemaError::WrongArgCount {
                                name:     format!("{}::{}", class, method),
                                expected: sig.params.len(),
                                found:    args.len(),
                                span:     span.clone(),
                            });
                        }
                        for arg in args { self.infer_expr(arg); }
                        return ret;
                    }
                }
                for arg in args { self.infer_expr(arg); }
                Type::Mixed
            }

            Expr::StaticConst { class, name, span } => {
                // Classe opaque (import non résolu) — accès permissif
                if let Some(info) = self.symbols.lookup_class(class) {
                    if info.is_opaque { return Type::Mixed; }
                }
                if let Some((ty, _)) = self.symbols.lookup_class_const(class, name) {
                    return ty.clone();
                }
                self.errors.push(SemaError::UndefinedSymbol {
                    name: format!("{}::{}", class, name),
                    span: span.clone(),
                });
                Type::Mixed
            }

            Expr::New { class, args, span } => {
                if self.symbols.lookup_class(class).is_none() {
                    self.errors.push(SemaError::NotAClass {
                        name: class.clone(),
                        span: span.clone(),
                    });
                }
                for arg in args { self.infer_expr(arg); }
                Type::Named(class.clone())
            }

            Expr::Binary { op, left, right, span } => {
                let lt = self.infer_expr(left);
                let rt = self.infer_expr(right);
                binary_result_type(op, &lt, &rt, span, &mut self.errors)
            }

            Expr::Unary { op, operand, span } => {
                let ty = self.infer_expr(operand);
                match op {
                    UnaryOp::Not => {
                        if !types_compat(&ty, &Type::Bool) {
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
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

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
        Type::Mixed              => "mixed".into(),
        Type::Void             => "void".into(),
        Type::Null             => "null".into(),
        Type::Named(n)         => n.clone(),
        Type::Qualified(parts) => parts.join("."),
        Type::Array(inner)     => format!("{}[]", type_name(inner)),
        Type::Map(k, v)        => format!("map<{},{}>", type_name(k), type_name(v)),
    }
}

/// Compatibilité laxiste : `mixed` accepte tout, `null` compatible avec tout type référence
pub fn types_compat(found: &Type, expected: &Type) -> bool {
    if matches!(found, Type::Mixed) || matches!(expected, Type::Mixed) {
        return true;
    }
    // null est compatible avec tout type référence (string, objet, tableau, map)
    if matches!(found, Type::Null) {
        return matches!(expected,
            Type::String | Type::Named(_) | Type::Array(_) | Type::Map(..) | Type::Null
        );
    }
    match (found, expected) {
        (Type::Array(f), Type::Array(e)) => types_compat(f, e),
        (Type::Map(fk, fv), Type::Map(ek, ev)) =>
            types_compat(fk, ek) && types_compat(fv, ev),
        _ => found == expected,
    }
}

fn binary_result_type(
    op:     &BinOp,
    lt:     &Type,
    rt:     &Type,
    span:   &Span,
    errors: &mut Vec<SemaError>,
) -> Type {
    match op {
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
            // Concaténation implicite : string + T ou T + string → string
            if op == &BinOp::Add
                && (matches!(lt, Type::String) || matches!(rt, Type::String))
            {
                return Type::String;
            }
            if types_compat(lt, rt) { lt.clone() } else {
                errors.push(SemaError::TypeMismatch {
                    expected: type_name(lt),
                    found:    type_name(rt),
                    span:     span.clone(),
                });
                lt.clone()
            }
        }
        BinOp::EqEq | BinOp::NotEq |
        BinOp::Lt   | BinOp::LtEq  |
        BinOp::Gt   | BinOp::GtEq  |
        BinOp::And  | BinOp::Or    => Type::Bool,
    }
}
