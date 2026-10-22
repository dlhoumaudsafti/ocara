/// Corps de `for k has v in m` qui ne peut pas modifier la map parcourue :
/// ni appel (une fonction peut l'atteindre par un alias), ni écriture
/// indexée, ni code qui rend la main (`try`, `emit`, `resolve`, fermeture).
/// `lower_for_map` la parcourt alors en place, sans relever ses clés.
use crate::parsing::ast::{Block, Expr, Stmt, TemplatePartExpr};

/// Ce que fait le corps d'une boucle : s'il garde les maps intactes, et les
/// variables qu'il réaffecte.
pub struct BodyScan {
    pub keeps_maps: bool,
    pub assigned: Vec<String>,
}

pub fn scan_body(block: &Block) -> BodyScan {
    let mut scan = BodyScan { keeps_maps: true, assigned: Vec::new() };
    scan.block(block);
    scan
}

impl BodyScan {
    fn block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.stmt(stmt);
        }
    }

    fn target(&mut self, target: &Expr) {
        match target {
            Expr::Ident(name, _) => self.assigned.push(name.clone()),
            Expr::Index { .. } => self.keeps_maps = false,
            _ => {}
        }
        self.expr(target);
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Var { value, .. } | Stmt::Const { value, .. } | Stmt::Expr(value) | Stmt::Raise { value, .. } => self.expr(value),
            Stmt::Assign { target, value, .. } => {
                self.target(target);
                self.expr(value);
            }
            Stmt::Return { value, .. } | Stmt::Result { value, .. } => value.iter().for_each(|v| self.expr(v)),
            Stmt::Break { .. } | Stmt::Continue { .. } => {}
            Stmt::If { condition, then_block, elseif, else_block, .. } => {
                self.expr(condition);
                self.block(then_block);
                for (c, b) in elseif {
                    self.expr(c);
                    self.block(b);
                }
                else_block.iter().for_each(|b| self.block(b));
            }
            Stmt::While { condition, body, .. } => {
                self.expr(condition);
                self.block(body);
            }
            Stmt::ForIn { iter, body, .. } | Stmt::ForMap { iter, body, .. } => {
                self.expr(iter);
                self.block(body);
            }
            Stmt::Switch { subject, cases, default, .. } => {
                self.expr(subject);
                cases.iter().for_each(|c| self.block(&c.body));
                default.iter().for_each(|b| self.block(b));
            }
            Stmt::Emit { .. } | Stmt::Try { .. } => self.keeps_maps = false,
        }
    }

    fn expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Call { .. } | Expr::StaticCall { .. } | Expr::New { .. } | Expr::Nameless { .. } | Expr::Resolve { .. } => {
                self.keeps_maps = false
            }
            Expr::IncDec { target, .. } => self.target(target),
            Expr::Literal(..) | Expr::Ident(..) | Expr::SelfExpr(_) | Expr::ParentExpr(_) | Expr::StaticConst { .. } => {}
            Expr::Field { object, .. } => self.expr(object),
            Expr::Binary { left, right, .. } => {
                self.expr(left);
                self.expr(right);
            }
            Expr::Unary { operand, .. } => self.expr(operand),
            Expr::Array { elements, .. } => elements.iter().for_each(|e| self.expr(e)),
            Expr::Map { entries, .. } => entries.iter().for_each(|(k, v)| {
                self.expr(k);
                self.expr(v);
            }),
            Expr::Template { parts, .. } => parts.iter().for_each(|p| {
                if let TemplatePartExpr::Expr(e) = p { self.expr(e) }
            }),
            Expr::Index { object, index, .. } => {
                self.expr(object);
                self.expr(index);
            }
            Expr::Range { start, end, .. } => {
                self.expr(start);
                self.expr(end);
            }
            Expr::Match { subject, arms, .. } => {
                self.expr(subject);
                arms.iter().for_each(|a| self.expr(&a.body));
            }
            Expr::IsCheck { expr, .. } | Expr::NamedArg { value: expr, .. } => self.expr(expr),
        }
    }
}
