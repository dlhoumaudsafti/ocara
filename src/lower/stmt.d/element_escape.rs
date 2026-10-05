/// Éléments d'un conteneur conservés au-delà de lui (`var n:string =
/// rows[0]["name"]`, `for row in rows { items.push(build(row)) }`…). La valeur
/// extraite est un alias d'un élément : la libération profonde du conteneur
/// (`scoped`/`consumed`, `var` libéré automatiquement) la rendrait pendante.
///
/// Deux mécanismes, sans fuite ni use-after-free :
/// - **Copie** (`dup_kept_leaf`, lowering) : une valeur DÉRIVÉE (index, champ,
///   variable de boucle) conservée là où le type cible est `string` est
///   copiée — chaînes immuables, copie invisible. Points : déclaration
///   `string`, affectation à une cible `string` (variable, `self.champ`,
///   élément d'un conteneur de `string`), `return` d'une fonction `string`,
///   argument d'un paramètre utilisateur `string`, élément de littéral
///   `string`. Une cible `int`/`float`/`bool` copie par valeur.
/// - **Libération de surface** (`shallow`) : un élément conservé sans copie
///   possible (cible composite ou inconnue) — seule la structure du
///   conteneur est libérée, la valeur conservée reste valide.
///
/// Un argument passé à une fonction/méthode/constructeur utilisateur n'est
/// conservé que si ce paramètre l'est dans l'appelé (`param_keeps`, calculé
/// par point fixe sur tout le programme) ; un builtin pur
/// (`crate::sema::escape::is_pure_builtin`) ne conserve rien. Une lecture
/// transitoire (template, comparaison, arithmétique) ne compte pas.
use std::collections::{HashMap, HashSet};

use crate::parsing::ast::{Block, Expr, Param, Stmt, TemplatePartExpr, Type};

/// Informations du programme nécessaires à l'analyse d'un corps.
pub struct EscapeCtx<'a> {
    /// `"fonction"`, `"Classe_methode"`, `"Classe_init"` → types des paramètres.
    pub param_types:   &'a HashMap<String, Vec<Type>>,
    /// Même clé → le paramètre `i` est-il conservé (lui ou une partie) ?
    pub param_keeps:   &'a HashMap<String, Vec<bool>>,
    pub field_types:   &'a HashMap<String, Vec<(String, Type)>>,
    pub current_class: Option<&'a str>,
    pub ret_ty:        Option<&'a Type>,
    /// `var` libérés automatiquement (`compute_auto_freeable_vars`) : une
    /// affectation de chaîne dérivée y est copiée (l'ancienne valeur est
    /// libérée par `free_before_reassign`).
    pub auto_free:     &'a HashSet<String>,
}

#[derive(Default)]
pub struct ElementEscapes {
    /// Conteneurs (et paramètres) dont un élément est conservé sans copie.
    pub shallow:      HashSet<String>,
    /// Variables de boucle parcourant un conteneur nommé.
    pub loop_aliases: HashSet<String>,
    /// Conteneurs dont un élément est désigné par une `var` (`var m = rows[0]`) :
    /// une `consumed` n'y est libérée qu'en fin de bloc, pas après son usage.
    pub var_alias_roots: HashSet<String>,
}

pub fn analyze(body: &Block, params: &[Param], ctx: &EscapeCtx) -> ElementEscapes {
    let mut walker = Walker {
        ctx,
        aliases: HashMap::new(),
        var_types: params.iter().map(|p| (p.name.clone(), p.ty.clone())).collect(),
        params: params.iter().map(|p| p.name.clone()).collect(),
        owned_vars: ctx.auto_free.clone(),
        out: ElementEscapes::default(),
    };
    walker.block(body);
    walker.out
}

/// Analyse le corps qui va être abaissé dans `builder` (avant `lower_block`) :
/// variables de boucle, `var` libérés automatiquement (si `auto_free`), puis
/// conteneurs à libérer en surface.
pub fn prepare_body(builder: &mut crate::lower::builder::LowerBuilder, body: &Block, params: &[Param], auto_free: bool) {
    let ret = builder.ret_ast_ty.clone();
    let class = builder.current_class.clone();
    let none = HashSet::new();
    let run = |builder: &crate::lower::builder::LowerBuilder, auto: &HashSet<String>| {
        let module = &*builder.module;
        let ctx = EscapeCtx {
            param_types: &module.param_ast_types, param_keeps: &module.param_keeps,
            field_types: &module.class_field_types, current_class: class.as_deref(), ret_ty: ret.as_ref(),
            auto_free: auto,
        };
        analyze(body, params, &ctx)
    };
    let aliases = run(builder, &none).loop_aliases;
    builder.auto_freeable_vars = if auto_free {
        crate::lower::stmt::ownership::compute_auto_freeable_vars(builder.module, body, class.as_deref(), &aliases)
    } else {
        HashSet::new()
    };
    let res = run(builder, &builder.auto_freeable_vars.clone());
    builder.element_escapes = res.shallow;
    builder.loop_aliases = res.loop_aliases;
    builder.var_alias_roots = res.var_alias_roots;
    let owners = {
        let module = &*builder.module;
        let ctx = crate::lower::stmt::object_owners::FreshCtx {
            fresh_returns: &module.fresh_returns, fresh_containers: &module.fresh_containers,
            preserving: &module.preserving_params, param_types: &module.param_ast_types,
            field_types: &module.class_field_types, field_decl: &module.field_decl, current_class: class.as_deref(),
            parent_class: builder.parent_class.as_deref(),
        };
        crate::lower::stmt::object_owners::object_owners(body, &ctx)
    };
    builder.object_owners = owners;
}

/// Point fixe : pour chaque fonction/méthode/constructeur, les paramètres
/// conservés par son corps. Départ « tout conservé », décroissant.
pub fn compute_param_keeps(
    callables: &[(String, Option<&str>, &[Param], &Block, Option<Type>)],
    param_types: &HashMap<String, Vec<Type>>,
    field_types: &HashMap<String, Vec<(String, Type)>>,
) -> HashMap<String, Vec<bool>> {
    let mut keeps: HashMap<String, Vec<bool>> = callables.iter()
        .map(|(key, _, params, _, _)| (key.clone(), vec![true; params.len()]))
        .collect();
    for _ in 0..8 {
        let mut next = HashMap::new();
        for (key, class, params, body, ret) in callables {
            let none = HashSet::new();
            let ctx = EscapeCtx { param_types, param_keeps: &keeps, field_types, current_class: *class, ret_ty: ret.as_ref(), auto_free: &none };
            let res = analyze(body, params, &ctx);
            next.insert(key.clone(), params.iter().map(|p| res.shallow.contains(&p.name)).collect());
        }
        if next == keeps { break; }
        keeps = next;
    }
    keeps
}

/// Le lowering copie-t-il une valeur dérivée conservée vers ce type ?
pub fn copies_into(ty: &Type) -> bool {
    matches!(ty, Type::String | Type::Int | Type::Float | Type::Bool)
}

struct Walker<'a> {
    ctx:       &'a EscapeCtx<'a>,
    aliases:   HashMap<String, String>,
    var_types: HashMap<String, Type>,
    params:    HashSet<String>,
    /// Variables libérées (`scoped`/`consumed`/`var` auto) : cibles de copie.
    owned_vars: HashSet<String>,
    out:       ElementEscapes,
}

impl Walker<'_> {
    fn root(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Ident(name, _) => Some(self.aliases.get(name).cloned().unwrap_or_else(|| name.clone())),
            Expr::Index { object, .. } | Expr::Field { object, .. } => self.root(object),
            _ => None,
        }
    }

    fn is_derived(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Index { .. } | Expr::Field { .. } => true,
            Expr::Ident(name, _) => self.aliases.contains_key(name),
            _ => false,
        }
    }

    /// `expr` conservé vers une cible de type `target` (inconnu : `None`).
    fn kept(&mut self, expr: &Expr, target: Option<&Type>) {
        match expr {
            Expr::Array { elements, .. } => {
                let inner = match target { Some(Type::Array(t)) => Some(t.as_ref()), _ => None };
                elements.iter().for_each(|e| self.kept(e, inner));
                return;
            }
            Expr::Map { entries, .. } => {
                let inner = match target { Some(Type::Map(_, t)) => Some(t.as_ref()), _ => None };
                entries.iter().for_each(|(k, v)| { self.expr(k); self.kept(v, inner); });
                return;
            }
            _ => {}
        }
        let copied = self.is_derived(expr) && target.is_some_and(copies_into);
        if !copied {
            let direct_param = matches!(expr, Expr::Ident(n, _) if self.params.contains(n));
            if self.is_derived(expr) || direct_param {
                if let Some(root) = self.root(expr) { self.out.shallow.insert(root); }
            }
        }
        if let Expr::Match { subject, arms, .. } = expr {
            self.expr(subject);
            arms.iter().for_each(|arm| self.kept(&arm.body, None));
        } else {
            self.expr(expr);
        }
    }

    fn block(&mut self, block: &Block) {
        block.stmts.iter().for_each(|s| self.stmt(s));
    }

    fn assign_target_type(&self, target: &Expr) -> Option<Type> {
        match target {
            Expr::Ident(name, _) if self.owned_vars.contains(name) => self.var_types.get(name).cloned(),
            Expr::Ident(..) => None,
            Expr::Field { object, field, .. } if matches!(object.as_ref(), Expr::SelfExpr(_)) => {
                self.ctx.field_types.get(self.ctx.current_class?)?.iter().find(|(f, _)| f == field).map(|(_, t)| t.clone())
            }
            Expr::Index { object, .. } => match self.container_type(object)? {
                Type::Array(inner) | Type::Map(_, inner) => Some(*inner),
                _ => None,
            },
            _ => None,
        }
    }

    /// Type déclaré d'un conteneur cible (`out` dans `out[k] = v`).
    fn container_type(&self, expr: &Expr) -> Option<Type> {
        match expr {
            Expr::Ident(name, _) => self.var_types.get(name).cloned(),
            Expr::Field { object, field, .. } if matches!(object.as_ref(), Expr::SelfExpr(_)) => {
                self.ctx.field_types.get(self.ctx.current_class?)?.iter().find(|(f, _)| f == field).map(|(_, t)| t.clone())
            }
            _ => None,
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match stmt {
            // `var x:map<…> = rows[0]` : alias d'un élément composite, comme
            // une variable de boucle — seuls ses usages conservés comptent.
            Stmt::Var { name, ty, value, kind: crate::parsing::ast::VarKind::Var, .. }
                if !copies_into(ty) && self.is_derived(value) && self.root(value).is_some() =>
            {
                self.expr(value);
                let root = self.root(value).unwrap_or_default();
                self.out.var_alias_roots.insert(root.clone());
                self.aliases.insert(name.clone(), root);
                self.out.loop_aliases.insert(name.clone());
                self.var_types.insert(name.clone(), ty.clone());
            }
            Stmt::Var { name, ty, value, kind, .. } => {
                self.kept(value, Some(ty));
                self.var_types.insert(name.clone(), ty.clone());
                if !matches!(kind, crate::parsing::ast::VarKind::Var) { self.owned_vars.insert(name.clone()); }
            }
            Stmt::Const { name, ty, value, .. } => {
                self.kept(value, Some(ty));
                self.var_types.insert(name.clone(), ty.clone());
            }
            Stmt::Raise { value, .. } | Stmt::Emit { value, .. } => self.kept(value, None),
            Stmt::Assign { target, value, .. } => {
                self.expr(target);
                let ty = self.assign_target_type(target);
                self.kept(value, ty.as_ref());
            }
            // Pas de copie au retour (une méthode d'accès copierait à chaque
            // appel) : seul un scalaire retourné ne dépend plus du conteneur.
            Stmt::Return { value, .. } | Stmt::Result { value, .. } => {
                let scalar = self.ctx.ret_ty.filter(|t| matches!(t, Type::Int | Type::Float | Type::Bool));
                if let Some(v) = value { self.kept(v, scalar); }
            }
            Stmt::Expr(e) => self.expr(e),
            Stmt::If { condition, then_block, elseif, else_block, .. } => {
                self.expr(condition);
                self.block(then_block);
                for (c, b) in elseif { self.expr(c); self.block(b); }
                if let Some(b) = else_block { self.block(b); }
            }
            Stmt::Switch { subject, cases, default, .. } => {
                self.expr(subject);
                cases.iter().for_each(|c| self.block(&c.body));
                if let Some(b) = default { self.block(b); }
            }
            Stmt::While { condition, body, .. } => {
                self.expr(condition);
                self.block(body);
            }
            Stmt::ForIn { var, iter, body, .. } => self.loop_over(&[var], iter, body),
            Stmt::ForMap { key, value, iter, body, .. } => self.loop_over(&[key, value], iter, body),
            Stmt::Try { body, handlers, .. } => {
                self.block(body);
                handlers.iter().for_each(|h| self.block(&h.body));
            }
            Stmt::Break { .. } | Stmt::Continue { .. } => {}
        }
    }

    fn loop_over(&mut self, vars: &[&String], iter: &Expr, body: &Block) {
        self.expr(iter);
        if let Some(root) = self.root(iter) {
            for v in vars {
                self.aliases.insert((*v).clone(), root.clone());
                self.out.loop_aliases.insert((*v).clone());
            }
        }
        self.block(body);
    }

    /// Clé `param_types`/`param_keeps` de l'appelé, si c'est du code utilisateur.
    fn callee_key(&self, expr: &Expr) -> Option<String> {
        let key = match expr {
            Expr::Call { callee, .. } => match callee.as_ref() {
                Expr::Ident(name, _) => name.clone(),
                Expr::Field { object, field, .. } if matches!(object.as_ref(), Expr::SelfExpr(_)) => {
                    format!("{}_{}", self.ctx.current_class?, field)
                }
                _ => return None,
            },
            Expr::StaticCall { class, method, .. } => match class.as_str() {
                "<self>" => format!("{}_{}", self.ctx.current_class?, method),
                "<parent>" => return None,
                c => format!("{}_{}", c, method),
            },
            Expr::New { class, .. } => format!("{}_init", class),
            _ => return None,
        };
        self.ctx.param_types.contains_key(&key).then_some(key)
    }

    fn call_args(&mut self, call: &Expr, args: &[Expr]) {
        if let Expr::StaticCall { class, method, .. } = call {
            if crate::sema::escape::is_pure_builtin(class, method) {
                args.iter().for_each(|a| self.expr(a));
                return;
            }
        }
        let Some(key) = self.callee_key(call) else {
            args.iter().for_each(|a| self.kept(a, None));
            return;
        };
        let types = self.ctx.param_types.get(&key).cloned().unwrap_or_default();
        let keeps = self.ctx.param_keeps.get(&key).cloned().unwrap_or_default();
        for (i, arg) in args.iter().enumerate() {
            let ty = types.get(i).or(types.last());
            let kept = keeps.get(i).or(keeps.last()).copied().unwrap_or(true);
            if kept { self.kept(arg, ty); } else { self.expr(arg); }
        }
    }

    fn expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Call { callee, args, .. } => {
                self.expr(callee);
                self.call_args(expr, args);
            }
            Expr::StaticCall { args, .. } | Expr::New { args, .. } => self.call_args(expr, args),
            Expr::Array { .. } | Expr::Map { .. } => self.kept(expr, None),
            Expr::NamedArg { value, .. } => self.kept(value, None),
            Expr::Field { object: e, .. } | Expr::Unary { operand: e, .. } | Expr::Resolve { expr: e, .. }
            | Expr::IsCheck { expr: e, .. } | Expr::IncDec { target: e, .. } => self.expr(e),
            Expr::Binary { left: a, right: b, .. } | Expr::Index { object: a, index: b, .. } | Expr::Range { start: a, end: b, .. } => {
                self.expr(a);
                self.expr(b);
            }
            Expr::Template { parts, .. } => parts.iter().for_each(|p| {
                if let TemplatePartExpr::Expr(e) = p { self.expr(e); }
            }),
            Expr::Match { subject, arms, .. } => {
                self.expr(subject);
                arms.iter().for_each(|arm| self.expr(&arm.body));
            }
            // Corps analysé à part (son propre builder, voir nameless.rs) ;
            // une capture d'alias est conservée par la closure.
            Expr::Nameless { body, .. } => {
                let mut refs = HashSet::new();
                crate::sema::escape::collect_ident_refs(body, &mut refs);
                for name in refs {
                    if self.aliases.contains_key(&name) {
                        if let Some(root) = self.aliases.get(&name).cloned() { self.out.shallow.insert(root); }
                    }
                }
            }
            Expr::Literal(..) | Expr::Ident(..) | Expr::SelfExpr(_) | Expr::ParentExpr(_) | Expr::StaticConst { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{analyze, EscapeCtx};
    use crate::parsing::{lexer::Lexer, parser::Parser};
    use std::collections::HashMap;

    fn shallow(src: &str) -> Vec<String> {
        let program = Parser::new(Lexer::new(src).tokenize().unwrap()).parse_program().unwrap();
        let f = &program.functions[0];
        let empty: HashMap<String, Vec<crate::parsing::ast::Type>> = HashMap::new();
        let no_keeps: HashMap<String, Vec<bool>> = HashMap::new();
        let fields = HashMap::new();
        let none = std::collections::HashSet::new();
        let ctx = EscapeCtx { param_types: &empty, param_keeps: &no_keeps, field_types: &fields, current_class: None, ret_ty: Some(&f.ret_ty), auto_free: &none };
        let mut out: Vec<String> = analyze(&f.body, &f.params, &ctx).shallow.into_iter().collect();
        out.sort();
        out
    }

    fn body(b: &str) -> String {
        format!("function f(): int {{\n{}\n    return 0\n}}\n", b)
    }

    #[test]
    fn copied_string_and_scalar_extractions_keep_deep_free() {
        assert!(shallow(&body("var a:string = rows[0][\"name\"]\n    var n:int = rows[0][\"id\"]")).is_empty());
        assert!(shallow(&body("for row in rows {\n    var n:string = row[\"name\"]\n}")).is_empty());
        assert!(shallow("function f(rows:array<int>): int {\n    return rows[0]\n}\n").is_empty());
    }

    #[test]
    fn composite_or_unknown_kept_elements_are_shallow() {
        assert_eq!(shallow(&body("var m:map<string, mixed> = rows[0]\n    items.push(m)")), vec!["rows"]);
        assert!(shallow(&body("var m:map<string, mixed> = rows[0]\n    var s:string = m[\"name\"]")).is_empty());
        assert_eq!(shallow(&body("for row in rows {\n    items.push(row)\n}")), vec!["rows"]);
        assert_eq!(shallow(&body("for k has v in m {\n    out[k] = v\n}")), vec!["m"]);
    }

    #[test]
    fn transient_reads_do_not_count() {
        assert!(shallow(&body("for row in rows {\n    IO::writeln(`${row[\"name\"]}`)\n    IO::writeln(row[\"x\"])\n}")).is_empty());
        assert!(shallow(&body("var total:int = rows.len() + 1\n    var j:string = JSON::encode(rows)")).is_empty());
    }
}
