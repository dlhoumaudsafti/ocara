/// Éléments d'un conteneur conservés au-delà de lui : `var n = rows[0]["name"]`,
/// `for row in rows { items.push(build(row)) }`… La valeur extraite est un
/// alias d'un élément, pas une copie : la libération profonde du conteneur
/// (`scoped`/`consumed`, `var` libéré automatiquement) la rendrait pendante
/// (use-after-free confirmé : chaîne vide puis SIGSEGV). Pour ces conteneurs,
/// `ownership::drop_func_for` libère seulement la structure du conteneur —
/// les éléments fuient, rien n'est jamais libéré deux fois.
///
/// « Conservé » : initialiseur de variable, valeur affectée, `return`/
/// `result`/`raise`/`emit`, argument d'appel ou de constructeur, élément de
/// littéral. Une lecture transitoire (comparaison, arithmétique, template
/// `${row["x"]}`) ne compte pas. Analyse par nom sur tout le corps de la
/// fonction — prudente en cas de masquage.
use std::collections::{HashMap, HashSet};

use crate::parsing::ast::{Block, Expr, Stmt, TemplatePartExpr};

pub fn compute_element_escapes(body: &Block) -> HashSet<String> {
    let mut walker = Walker::default();
    walker.block(body);
    walker.escaped
}

#[derive(Default)]
struct Walker {
    /// Variable de boucle → conteneur racine qu'elle parcourt.
    aliases: HashMap<String, String>,
    escaped: HashSet<String>,
}

impl Walker {
    /// Conteneur racine dont `expr` est un élément (dérivé), s'il y en a un.
    fn derived_root(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Ident(name, _) => self.aliases.get(name).cloned(),
            Expr::Index { object, .. } | Expr::Field { object, .. } => self.root(object),
            _ => None,
        }
    }

    /// Racine d'un chemin `x`, `x[i]`, `x.f`, alias de boucle compris.
    fn root(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Ident(name, _) => Some(self.aliases.get(name).cloned().unwrap_or_else(|| name.clone())),
            Expr::Index { object, .. } | Expr::Field { object, .. } => self.root(object),
            _ => None,
        }
    }

    fn kept(&mut self, expr: &Expr) {
        if let Some(root) = self.derived_root(expr) {
            self.escaped.insert(root);
        }
        self.expr(expr);
    }

    fn block(&mut self, block: &Block) {
        block.stmts.iter().for_each(|s| self.stmt(s));
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Var { value, .. } | Stmt::Const { value, .. } | Stmt::Raise { value, .. } | Stmt::Emit { value, .. } => self.kept(value),
            Stmt::Assign { target, value, .. } => {
                self.expr(target);
                self.kept(value);
            }
            Stmt::Return { value, .. } | Stmt::Result { value, .. } => {
                if let Some(v) = value { self.kept(v); }
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
            for v in vars { self.aliases.insert((*v).clone(), root.clone()); }
        }
        self.block(body);
    }

    fn args(&mut self, args: &[Expr]) {
        args.iter().for_each(|a| self.kept(a));
    }

    fn expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Call { callee, args, .. } => {
                self.expr(callee);
                self.args(args);
            }
            Expr::StaticCall { args, .. } | Expr::New { args, .. } => self.args(args),
            Expr::Array { elements, .. } => self.args(elements),
            Expr::Map { entries, .. } => entries.iter().for_each(|(k, v)| { self.expr(k); self.kept(v); }),
            Expr::NamedArg { value, .. } => self.kept(value),
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
                arms.iter().for_each(|arm| self.kept(&arm.body));
            }
            Expr::Nameless { body, .. } => self.block(body),
            Expr::Literal(..) | Expr::Ident(..) | Expr::SelfExpr(_) | Expr::ParentExpr(_) | Expr::StaticConst { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::compute_element_escapes;
    use crate::parsing::{lexer::Lexer, parser::Parser};

    fn escapes(body: &str) -> Vec<String> {
        let src = format!("function f(): int {{\n{}\n    return 0\n}}\n", body);
        let program = Parser::new(Lexer::new(&src).tokenize().unwrap()).parse_program().unwrap();
        let mut out: Vec<String> = compute_element_escapes(&program.functions[0].body).into_iter().collect();
        out.sort();
        out
    }

    #[test]
    fn kept_elements_mark_their_container() {
        assert_eq!(escapes("var a:string = rows[0][\"name\"]"), vec!["rows"]);
        assert_eq!(escapes("for row in rows {\n    items.push(build(row))\n}"), vec!["rows"]);
        assert_eq!(escapes("for row in rows {\n    var n:string = row[\"name\"]\n}"), vec!["rows"]);
        assert_eq!(escapes("for k has v in m {\n    out[k] = v\n}"), vec!["m"]);
    }

    #[test]
    fn transient_reads_and_the_container_itself_do_not() {
        assert!(escapes("for row in rows {\n    IO::writeln(`${row[\"name\"]}`)\n}").is_empty());
        assert!(escapes("var total:int = rows.len() + 1\n    var j:string = JSON::encode(rows)").is_empty());
    }
}
