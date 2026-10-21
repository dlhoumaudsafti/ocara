/// W06 : requête SQL construite par interpolation (`${…}`) ou concaténation
/// (`+`) puis passée à `execute`/`query`/`queryOne`/`prepare` d'une
/// connexion `SQLite`/`MySQL`/`MariaDB` — injection SQL potentielle. Les
/// valeurs passent par des paramètres nommés `:nom` (docs/builtins/SQLite.md,
/// « Requêtes paramétrées »).

use crate::parsing::ast::{BinOp, Expr, TemplatePartExpr, Type};
use crate::parsing::token::Span;
use crate::sema::error::SemaWarning;
use crate::sema::typecheck::TypeChecker;

const SQL_CONNECTIONS: [&str; 3] = ["SQLite", "MySQL", "MariaDB"];
const SQL_METHODS: [&str; 4] = ["execute", "query", "queryOne", "prepare"];

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_sql_interpolation(&mut self, receiver: &Type, method: &str, args: &[Expr]) {
        let Type::Named(class) = receiver else { return };
        if !SQL_CONNECTIONS.contains(&class.as_str()) || !SQL_METHODS.contains(&method) {
            return;
        }
        if let Some(query) = args.first().filter(|q| is_built_query(q)) {
            self.warnings.push(SemaWarning::SqlInterpolation {
                method: method.to_string(),
                span: query_start(query).clone(),
            });
        }
    }
}

fn is_built_query(query: &Expr) -> bool {
    match query {
        Expr::Template { parts, .. } => parts.iter().any(|p| matches!(p, TemplatePartExpr::Expr(_))),
        Expr::Binary { op: BinOp::Add, left, right, .. } =>
            !matches!(left.as_ref(), Expr::Literal(..)) || !matches!(right.as_ref(), Expr::Literal(..)),
        _ => false,
    }
}

fn query_start(query: &Expr) -> &Span {
    match query {
        Expr::Binary { left, .. } => query_start(left),
        _ => query.span(),
    }
}
