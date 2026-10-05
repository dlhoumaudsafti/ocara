/// Conteneurs propriétaires de leurs objets : un `array<Classe>`/`map<K,
/// Classe>` libéré (`scoped`/`consumed`/`var` automatique) ne libère ses
/// instances (`__array_free_objects`) que si CHAQUE objet qui y entre est
/// NEUF — personne d'autre ne peut le référencer, la libération ne peut donc
/// pas laisser de référence pendante. Sinon seule la structure est libérée.
///
/// Neuf : `use Classe(...)`, appel d'une fonction/méthode qui ne retourne que
/// des objets neufs (`compute_fresh_returns`, point fixe), ou variable locale
/// initialisée ainsi et conservée nulle part ailleurs.
///
/// Disqualifie un conteneur : initialiseur non littéral (`= repo.all()`),
/// élément non neuf (`items.push(self.item)`), réaffectation, passage en
/// argument (l'appelé pourrait y insérer), toute méthode autre que `push`/
/// `len`, ou capture par une closure. L'extraction d'un élément conservé est
/// déjà traitée par `element_escape` (libération de surface).
use std::collections::{HashMap, HashSet};

use crate::parsing::ast::{Block, Expr, Param, Stmt, TemplatePartExpr, Type};

/// Contexte : appelés qui ne retournent que des objets neufs, classe courante.
pub struct FreshCtx<'a> {
    pub fresh_returns: &'a HashSet<String>,
    pub current_class: Option<&'a str>,
}

/// Point fixe : fonctions/méthodes (clé `"fonction"`/`"Classe_methode"`)
/// dont toutes les valeurs retournées sont des objets neufs.
pub fn compute_fresh_returns(callables: &[(String, Option<&str>, &[Param], &Block, Option<Type>)]) -> HashSet<String> {
    let mut fresh: HashSet<String> = callables.iter()
        .filter(|(_, _, _, _, ret)| matches!(ret, Some(Type::Named(_))))
        .map(|(key, ..)| key.clone())
        .collect();
    loop {
        let next: HashSet<String> = callables.iter()
            .filter(|(key, class, _, body, _)| {
                fresh.contains(key) && {
                    let ctx = FreshCtx { fresh_returns: &fresh, current_class: *class };
                    returns_only_fresh(body, &ctx)
                }
            })
            .map(|(key, ..)| key.clone())
            .collect();
        if next == fresh { return fresh; }
        fresh = next;
    }
}

fn returns_only_fresh(body: &Block, ctx: &FreshCtx) -> bool {
    let mut scan = Scan::with_refs(ctx, body);
    scan.block(body);
    let mut returns = Vec::new();
    collect_returns(body, &mut returns);
    !returns.is_empty() && returns.iter().all(|e| scan.is_fresh(e, true))
}

fn collect_returns<'b>(block: &'b Block, out: &mut Vec<&'b Expr>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Return { value: Some(v), .. } => out.push(v),
            Stmt::Return { value: None, .. } => out.push(&NULL_EXPR),
            _ => for_each_block(stmt, &mut |b| collect_returns(b, out)),
        }
    }
}

static NULL_EXPR: Expr = Expr::Literal(crate::parsing::ast::Literal::Null, crate::parsing::token::Span { line: 0, col: 0, file: None, runtime_ctx: None });

fn for_each_block<'b>(stmt: &'b Stmt, f: &mut dyn FnMut(&'b Block)) {
    match stmt {
        Stmt::If { then_block, elseif, else_block, .. } => {
            f(then_block);
            elseif.iter().for_each(|(_, b)| f(b));
            if let Some(b) = else_block { f(b); }
        }
        Stmt::Switch { cases, default, .. } => {
            cases.iter().for_each(|c| f(&c.body));
            if let Some(b) = default { f(b); }
        }
        Stmt::While { body, .. } | Stmt::ForIn { body, .. } | Stmt::ForMap { body, .. } => f(body),
        Stmt::Try { body, handlers, .. } => {
            f(body);
            handlers.iter().for_each(|h| f(&h.body));
        }
        _ => {}
    }
}

/// Conteneurs propriétaires de leurs objets dans `body`.
pub fn object_owners(body: &Block, ctx: &FreshCtx) -> HashSet<String> {
    let mut scan = Scan::with_refs(ctx, body);
    scan.block(body);
    scan.candidates.difference(&scan.disqualified).cloned().collect()
}

struct Scan<'a> {
    ctx: &'a FreshCtx<'a>,
    /// Variable locale → initialiseur neuf (objet) ?
    fresh_vars: HashMap<String, bool>,
    /// Nombre de positions où une variable est conservée (push, argument…).
    kept_uses: HashMap<String, usize>,
    /// Nombre total de références à chaque identifiant dans le corps.
    refs: HashMap<String, usize>,
    candidates: HashSet<String>,
    disqualified: HashSet<String>,
}

impl<'a> Scan<'a> {
    fn new(ctx: &'a FreshCtx<'a>) -> Self {
        Scan { ctx, fresh_vars: HashMap::new(), kept_uses: HashMap::new(), refs: HashMap::new(), candidates: HashSet::new(), disqualified: HashSet::new() }
    }

    fn with_refs(ctx: &'a FreshCtx<'a>, body: &Block) -> Self {
        let mut scan = Scan::new(ctx);
        count_refs_block(body, &mut scan.refs);
        scan
    }

    fn callee_fresh(&self, expr: &Expr) -> bool {
        let key = match expr {
            Expr::Call { callee, .. } => match callee.as_ref() {
                Expr::Ident(name, _) => name.clone(),
                Expr::Field { object, field, .. } if matches!(object.as_ref(), Expr::SelfExpr(_)) => match self.ctx.current_class {
                    Some(c) => format!("{}_{}", c, field),
                    None => return false,
                },
                _ => return false,
            },
            Expr::StaticCall { class, method, .. } => match (class.as_str(), self.ctx.current_class) {
                ("<self>", Some(c)) => format!("{}_{}", c, method),
                ("<self>" | "<parent>", _) => return false,
                (c, _) => format!("{}_{}", c, method),
            },
            _ => return false,
        };
        self.ctx.fresh_returns.contains(&key)
    }

    /// Objet neuf. `single_use` : une variable neuve ne doit être conservée
    /// qu'à cet endroit.
    fn is_fresh(&self, expr: &Expr, single_use: bool) -> bool {
        match expr {
            Expr::New { .. } | Expr::Literal(crate::parsing::ast::Literal::Null, _) => true,
            Expr::Call { .. } | Expr::StaticCall { .. } => self.callee_fresh(expr),
            // Une variable neuve n'est « neuve » que si cette insertion est sa
            // SEULE référence : lue ailleurs, elle survivrait au conteneur.
            Expr::Ident(name, _) => self.fresh_vars.get(name).copied().unwrap_or(false)
                && self.refs.get(name).copied().unwrap_or(0) <= 1
                && (!single_use || self.kept_uses.get(name).copied().unwrap_or(0) <= 1),
            _ => false,
        }
    }

    fn note_kept(&mut self, expr: &Expr) {
        if let Expr::Ident(name, _) = expr {
            *self.kept_uses.entry(name.clone()).or_default() += 1;
        }
    }

    fn insertion(&mut self, container: &str, value: &Expr) {
        if !self.is_fresh(value, false) {
            self.disqualified.insert(container.to_string());
        }
        self.note_kept(value);
    }

    fn block(&mut self, block: &Block) {
        block.stmts.iter().for_each(|s| self.stmt(s));
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Var { name, ty, value, .. } | Stmt::Const { name, ty, value, .. } => {
                if is_object_container(ty) {
                    match value {
                        Expr::Array { elements, .. } => {
                            self.candidates.insert(name.clone());
                            elements.iter().for_each(|e| self.insertion(name, e));
                        }
                        Expr::Map { entries, .. } => {
                            self.candidates.insert(name.clone());
                            entries.iter().for_each(|(_, v)| self.insertion(name, v));
                        }
                        _ => { self.disqualified.insert(name.clone()); }
                    }
                } else {
                    let fresh = matches!(ty, Type::Named(_)) && self.is_fresh(value, false);
                    self.fresh_vars.insert(name.clone(), fresh);
                    self.expr(value);
                }
            }
            Stmt::Assign { target, value, .. } => {
                match target {
                    Expr::Index { object, .. } => match object.as_ref() {
                        Expr::Ident(c, _) if self.candidates.contains(c) => self.insertion(c, value),
                        _ => self.expr(target),
                    },
                    Expr::Ident(name, _) => {
                        self.disqualified.insert(name.clone());
                        self.fresh_vars.insert(name.clone(), false);
                    }
                    _ => self.expr(target),
                }
                self.kept(value);
            }
            Stmt::Expr(e) => self.expr(e),
            Stmt::Return { value: Some(v), .. } | Stmt::Result { value: Some(v), .. } | Stmt::Raise { value: v, .. } | Stmt::Emit { value: v, .. } => self.kept(v),
            Stmt::If { condition, .. } | Stmt::While { condition, .. } => {
                self.expr(condition);
                for_each_block(stmt, &mut |b| self.block(b));
            }
            Stmt::Switch { subject, .. } => {
                self.expr(subject);
                for_each_block(stmt, &mut |b| self.block(b));
            }
            Stmt::ForIn { iter, .. } | Stmt::ForMap { iter, .. } => {
                self.read(iter);
                for_each_block(stmt, &mut |b| self.block(b));
            }
            Stmt::Try { .. } => for_each_block(stmt, &mut |b| self.block(b)),
            Stmt::Return { .. } | Stmt::Result { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => {}
        }
    }

    /// Valeur conservée (retournée, affectée…) : un conteneur candidat qui
    /// s'en va ainsi est cloné par `maybe_clone_escaping`, mais ses objets
    /// le seraient aussi — disqualifié par prudence.
    fn kept(&mut self, expr: &Expr) {
        self.note_kept(expr);
        if let Expr::Ident(name, _) = expr {
            self.disqualified.insert(name.clone());
        }
        self.expr(expr);
    }

    /// Lecture du conteneur (itération, index) : sans effet sur sa propriété.
    fn read(&mut self, expr: &Expr) {
        if !matches!(expr, Expr::Ident(..)) { self.expr(expr); }
    }

    fn call_args(&mut self, args: &[Expr]) {
        for a in args {
            self.note_kept(a);
            if let Expr::Ident(name, _) = a { self.disqualified.insert(name.clone()); }
            self.expr(a);
        }
    }

    fn expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Call { callee, args, .. } => match callee.as_ref() {
                Expr::Field { object, field, .. } if matches!(object.as_ref(), Expr::Ident(c, _) if self.candidates.contains(c)) => {
                    let Expr::Ident(c, _) = object.as_ref() else { unreachable!() };
                    match (field.as_str(), args.as_slice()) {
                        ("push", [value]) => { let c = c.clone(); self.insertion(&c, value); self.expr(value); }
                        ("len", []) => {}
                        _ => { self.disqualified.insert(c.clone()); self.call_args(args); }
                    }
                }
                _ => {
                    if let Expr::Field { object, .. } = callee.as_ref() { self.read(object); } else { self.expr(callee); }
                    self.call_args(args);
                }
            },
            Expr::StaticCall { class, method, args, .. } => {
                if let (true, [Expr::Ident(c, _), value]) = (class == "Array" && method == "push", args.as_slice()) {
                    if self.candidates.contains(c) {
                        let c = c.clone();
                        self.insertion(&c, value);
                        self.expr(value);
                        return;
                    }
                }
                if crate::sema::escape::is_pure_builtin(class, method) {
                    args.iter().for_each(|a| self.expr(a));
                } else {
                    self.call_args(args);
                }
            }
            Expr::New { args, .. } => self.call_args(args),
            Expr::Array { elements, .. } => elements.iter().for_each(|e| self.kept(e)),
            Expr::Map { entries, .. } => entries.iter().for_each(|(k, v)| { self.expr(k); self.kept(v); }),
            Expr::NamedArg { value, .. } => self.kept(value),
            Expr::Index { object, index, .. } => { self.read(object); self.expr(index); }
            Expr::Field { object, .. } => self.read(object),
            Expr::Unary { operand: e, .. } | Expr::Resolve { expr: e, .. } | Expr::IsCheck { expr: e, .. } | Expr::IncDec { target: e, .. } => self.expr(e),
            Expr::Binary { left: a, right: b, .. } | Expr::Range { start: a, end: b, .. } => { self.expr(a); self.expr(b); }
            Expr::Template { parts, .. } => parts.iter().for_each(|p| {
                if let TemplatePartExpr::Expr(e) = p { self.expr(e); }
            }),
            Expr::Match { subject, arms, .. } => {
                self.expr(subject);
                arms.iter().for_each(|arm| self.kept(&arm.body));
            }
            Expr::Nameless { body, .. } => {
                let mut refs = HashSet::new();
                crate::sema::escape::collect_ident_refs(body, &mut refs);
                for name in refs {
                    self.disqualified.insert(name.clone());
                    *self.kept_uses.entry(name).or_default() += 2;
                }
            }
            Expr::Literal(..) | Expr::Ident(..) | Expr::SelfExpr(_) | Expr::ParentExpr(_) | Expr::StaticConst { .. } => {}
        }
    }
}

fn count_refs_block(block: &Block, refs: &mut HashMap<String, usize>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Var { value, .. } | Stmt::Const { value, .. } | Stmt::Expr(value) | Stmt::Raise { value, .. } | Stmt::Emit { value, .. } => count_refs(value, refs),
            Stmt::Assign { target, value, .. } => { count_refs(target, refs); count_refs(value, refs); }
            Stmt::Return { value: Some(v), .. } | Stmt::Result { value: Some(v), .. } => count_refs(v, refs),
            Stmt::If { condition: c, .. } | Stmt::While { condition: c, .. } | Stmt::Switch { subject: c, .. } => count_refs(c, refs),
            Stmt::ForIn { iter, .. } | Stmt::ForMap { iter, .. } => count_refs(iter, refs),
            _ => {}
        }
        for_each_block(stmt, &mut |b| count_refs_block(b, refs));
    }
}

fn count_refs(expr: &Expr, refs: &mut HashMap<String, usize>) {
    match expr {
        Expr::Ident(name, _) => *refs.entry(name.clone()).or_default() += 1,
        Expr::Call { callee, args, .. } => { count_refs(callee, refs); args.iter().for_each(|a| count_refs(a, refs)); }
        Expr::StaticCall { args, .. } | Expr::New { args, .. } | Expr::Array { elements: args, .. } => args.iter().for_each(|a| count_refs(a, refs)),
        Expr::Map { entries, .. } => entries.iter().for_each(|(k, v)| { count_refs(k, refs); count_refs(v, refs); }),
        Expr::Field { object: e, .. } | Expr::Unary { operand: e, .. } | Expr::Resolve { expr: e, .. }
        | Expr::IsCheck { expr: e, .. } | Expr::IncDec { target: e, .. } | Expr::NamedArg { value: e, .. } => count_refs(e, refs),
        Expr::Binary { left: a, right: b, .. } | Expr::Index { object: a, index: b, .. } | Expr::Range { start: a, end: b, .. } => {
            count_refs(a, refs);
            count_refs(b, refs);
        }
        Expr::Template { parts, .. } => parts.iter().for_each(|p| if let TemplatePartExpr::Expr(e) = p { count_refs(e, refs) }),
        Expr::Match { subject, arms, .. } => { count_refs(subject, refs); arms.iter().for_each(|a| count_refs(&a.body, refs)); }
        Expr::Nameless { body, .. } => {
            let mut inner = HashSet::new();
            crate::sema::escape::collect_ident_refs(body, &mut inner);
            inner.into_iter().for_each(|n| *refs.entry(n).or_default() += 2);
        }
        Expr::Literal(..) | Expr::SelfExpr(_) | Expr::ParentExpr(_) | Expr::StaticConst { .. } => {}
    }
}

/// `array<Classe>` ou `map<K, Classe>` (classe utilisateur).
pub fn object_elem_class(ty: &Type) -> Option<&str> {
    match ty {
        Type::Array(inner) | Type::Map(_, inner) => match inner.as_ref() {
            Type::Named(n) => Some(n.as_str()),
            _ => None,
        },
        _ => None,
    }
}

fn is_object_container(ty: &Type) -> bool {
    object_elem_class(ty).is_some()
}

#[cfg(test)]
mod tests {
    use super::{compute_fresh_returns, object_owners, FreshCtx};
    use crate::parsing::{lexer::Lexer, parser::Parser};
    use std::collections::HashSet;

    fn owners(src: &str) -> Vec<String> {
        let program = Parser::new(Lexer::new(src).tokenize().unwrap()).parse_program().unwrap();
        let callables: Vec<_> = program.functions.iter()
            .map(|f| (f.name.clone(), None, f.params.as_slice(), &f.body, Some(f.ret_ty.clone())))
            .collect();
        let fresh = compute_fresh_returns(&callables);
        let main = program.functions.iter().find(|f| f.name == "main").unwrap();
        let mut out: Vec<String> = object_owners(&main.body, &FreshCtx { fresh_returns: &fresh, current_class: None }).into_iter().collect();
        out.sort();
        out
    }

    const ITEM: &str = "class Item {\n    init() { }\n}\nfunction build(): Item {\n    return use Item()\n}\nfunction pick(xs:array<Item>): Item {\n    return xs[0]\n}\n";

    #[test]
    fn fresh_insertions_make_an_owner() {
        let src = format!("{}function main(): int {{\n    scoped a:array<Item> = [use Item()]\n    a.push(build())\n    var it:Item = use Item()\n    a.push(it)\n    for x in a {{ IO::writeln(\"x\") }}\n    return a.len()\n}}\n", ITEM);
        assert_eq!(owners(&src), vec!["a"]);
    }

    #[test]
    fn shared_or_unknown_insertions_disqualify() {
        let shared = format!("{}function main(): int {{\n    var it:Item = use Item()\n    scoped a:array<Item> = []\n    a.push(it)\n    IO::writeln(it.name)\n    return 0\n}}\n", ITEM);
        assert!(owners(&shared).is_empty());
        let unknown = format!("{}function main(): int {{\n    var src:array<Item> = []\n    scoped a:array<Item> = []\n    a.push(pick(src))\n    return 0\n}}\n", ITEM);
        assert!(owners(&unknown).is_empty());
        let passed = format!("{}function main(): int {{\n    scoped a:array<Item> = []\n    fill(a)\n    return 0\n}}\n", ITEM);
        assert!(owners(&passed).is_empty());
    }

    #[test]
    fn fresh_returns_fixpoint() {
        let src = format!("{}function main(): int {{ return 0 }}\n", ITEM);
        let program = Parser::new(Lexer::new(&src).tokenize().unwrap()).parse_program().unwrap();
        let callables: Vec<_> = program.functions.iter()
            .map(|f| (f.name.clone(), None, f.params.as_slice(), &f.body, Some(f.ret_ty.clone())))
            .collect();
        let fresh = compute_fresh_returns(&callables);
        assert_eq!(fresh, HashSet::from(["build".to_string()]));
    }
}
