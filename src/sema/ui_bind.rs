/// `ui.bind("user", user)` — liaisons déclaratives d'`ocara.UI`
/// (`bind: user.name` hors d'une liste, docs/oc-ui.md).
///
/// Le runtime ne lit pas un champ par son nom : la sema génère, pour la
/// classe de l'objet, un accesseur `nameless(objet, rang, valeur): mixed`
/// (rang ≥ 0 : lecture du chemin, `-rang - 1` : écriture convertie) et la
/// liste de ses chemins, avec les natures de valeur (`text`, `number`,
/// `bool`) que chacun accepte. L'accesseur est du code Ocara relu par le
/// parseur puis vérifié comme le reste (visibilité, conversions `mixed`) ;
/// l'appel devient `ui.bind(racine, objet, chemins, accesseur)`
/// (`AstRewrites::calls`). Les chemins du `.oc.ui` sont vérifiés ici quand
/// l'interface est connue (argument ajouté par `core::ui_file`), sinon au
/// `bind` par le runtime (`UIException` 107).

use std::collections::BTreeMap;

use crate::parsing::ast::{Expr, Literal, Type, Visibility};
use crate::parsing::lexer::Lexer;
use crate::parsing::parser::Parser;
use crate::parsing::token::Span;
use crate::sema::error::SemaError;
use crate::sema::named_args::site_key;
use crate::sema::typecheck::{type_name, TypeChecker};

/// Niveaux de champs suivis depuis l'objet (`address.city` : 2).
const MAX_DEPTH: usize = 3;

/// Chemin liable : champs traversés (nom, nullable) et type de la feuille.
struct BindPath {
    fields: Vec<(String, bool)>,
    leaf:   Type,
}

impl BindPath {
    fn name(&self) -> String {
        self.fields.iter().map(|(f, _)| f.as_str()).collect::<Vec<_>>().join(".")
    }
}

/// Natures de valeur qu'un champ de type `ty` peut afficher ou recevoir.
fn leaf_kinds(ty: &Type) -> Option<&'static [&'static str]> {
    match ty {
        Type::String => Some(&["text"]),
        Type::Int | Type::Float => Some(&["text", "number"]),
        Type::Bool => Some(&["bool", "text"]),
        Type::Mixed => Some(&["text", "number", "bool"]),
        _ => None,
    }
}

/// `T|null` → (`T`, vrai) ; un autre type tel quel.
fn without_null(ty: &Type) -> (Type, bool) {
    if let Type::Union(variants) = ty {
        let others: Vec<&Type> = variants.iter().filter(|v| **v != Type::Null).collect();
        if let [single] = others.as_slice() {
            return ((*single).clone(), others.len() < variants.len());
        }
    }
    (ty.clone(), false)
}

/// Écriture de `value` (mixed) dans `target`, convertie au type du champ.
fn assignment(target: &str, leaf: &Type) -> String {
    let convert = |other: &str, method: &str| format!(
        "if __v is string {{\n{target} = __v.{method}()\n}} elseif __v is {other} {{\n{target} = __v.{method}()\n}} else {{\n{target} = __v\n}}\n");
    match leaf {
        Type::Int => convert("float", "toInt"),
        Type::Float => convert("int", "toFloat"),
        _ => format!("{target} = __v\n"),
    }
}

/// Source de l'accesseur de `class` pour `paths`.
fn accessor_source(class: &str, paths: &[BindPath]) -> String {
    let mut body = String::new();
    for (rank, path) in paths.iter().enumerate() {
        let mut guards = String::new();
        let mut prefix = "__o".to_string();
        for (field, nullable) in &path.fields[..path.fields.len() - 1] {
            prefix = format!("{prefix}.{field}");
            if *nullable {
                guards.push_str(&format!("if {prefix} equal null {{\nreturn null\n}}\n"));
            }
        }
        let target = format!("__o.{}", path.name());
        body.push_str(&format!("if __i equal {rank} {{\n{guards}return {target}\n}}\n"));
        body.push_str(&format!("if __i equal {} {{\n{guards}{}return null\n}}\n", -(rank as i64) - 1, assignment(&target, &path.leaf)));
    }
    format!("nameless(__o:{class}, __i:int, __v:mixed): mixed {{\n{body}return null\n}}")
}

/// Expression de `source`, chaque position rattachée au pseudo-fichier
/// `origin` (clés de réécriture distinctes du code écrit).
fn parse_generated(source: &str, origin: &str) -> Option<Expr> {
    let mut tokens = Lexer::new(source).tokenize().ok()?;
    for token in &mut tokens {
        token.span.file = Some(origin.to_string());
    }
    Parser::new(tokens).parse_expr().ok()
}

impl<'a> TypeChecker<'a> {
    /// Le champ (déclaré dans `owner`) est-il lisible ici ?
    fn field_accessible(&self, owner: &str, vis: &Visibility) -> bool {
        match vis {
            Visibility::Public => true,
            Visibility::Private => self.current_class.as_deref() == Some(owner),
            Visibility::Protected => self.current_class.as_deref().is_some_and(|c| self.symbols.class_matches(c, owner)),
        }
    }

    /// Champs de `class` et de ses ancêtres (le plus proche l'emporte).
    fn class_fields(&self, class: &str) -> BTreeMap<String, (String, Type, Visibility)> {
        let mut fields = BTreeMap::new();
        let mut current = Some(class.to_string());
        while let Some(name) = current {
            let Some(info) = self.symbols.lookup_class(&name) else { break };
            for (field, f) in &info.fields {
                fields.entry(field.clone()).or_insert_with(|| (name.clone(), f.ty.clone(), f.vis.clone()));
            }
            current = info.extends.clone();
        }
        fields
    }

    /// Chemins liables de `class`, préfixés par `prefix`.
    fn bind_paths(&self, class: &str, prefix: &[(String, bool)], out: &mut Vec<BindPath>) {
        for (field, (owner, ty, vis)) in self.class_fields(class) {
            if !self.field_accessible(&owner, &vis) {
                continue;
            }
            let (ty, nullable) = without_null(&ty);
            let mut fields = prefix.to_vec();
            fields.push((field, nullable));
            if leaf_kinds(&ty).is_some() {
                out.push(BindPath { fields, leaf: ty });
            } else if let Type::Named(inner) = &ty {
                if fields.len() < MAX_DEPTH && self.symbols.lookup_class(inner).is_some() {
                    self.bind_paths(inner, &fields, out);
                }
            }
        }
    }

    /// Pourquoi `path` n'est pas un chemin liable de `class`.
    fn bind_path_issue(&self, class: &str, path: &str) -> String {
        let mut current = class.to_string();
        let segments: Vec<&str> = path.split('.').collect();
        for (depth, segment) in segments.iter().enumerate() {
            let Some((owner, info)) = self.symbols.lookup_field_owner(&current, segment) else {
                return format!("class '{current}' has no field '{segment}'");
            };
            if !self.field_accessible(owner, &info.vis) {
                return format!("field '{segment}' of '{owner}' is not accessible here");
            }
            let (ty, _) = without_null(&info.ty);
            let last = depth + 1 == segments.len();
            match (&ty, last) {
                (Type::Named(inner), false) if self.symbols.lookup_class(inner).is_some() => current = inner.clone(),
                (Type::Named(_), true) => return format!("'{segment}' is an object: bind one of its fields"),
                _ if leaf_kinds(&ty).is_none() => {
                    return format!("field '{segment}' has type '{}': only string, int, float, bool, mixed and object fields can be bound", type_name(&ty));
                }
                _ if last => break,
                _ => return format!("'{segment}' has no fields"),
            }
        }
        format!("paths are followed {MAX_DEPTH} fields deep at most")
    }

    /// Chemins du `.oc.ui` (`spec` : `roots.<racine>`) contre ceux de la classe.
    fn check_bind_spec(&mut self, root: &str, class: &str, paths: &[BindPath], spec: &str, span: &Span) {
        let Ok(spec) = serde_json::from_str::<serde_json::Value>(spec) else { return };
        let names: Vec<&str> = spec["paths"].as_array().into_iter().flatten().filter_map(|p| p.as_str()).collect();
        for node in spec["nodes"].as_array().into_iter().flatten() {
            let (Some(index), Some(kind)) = (node["path"].as_u64(), node["kind"].as_str()) else { continue };
            let Some(name) = names.get(index as usize) else { continue };
            let message = match paths.iter().find(|p| p.name() == *name) {
                None => self.bind_path_issue(class, name),
                Some(p) if !leaf_kinds(&p.leaf).is_some_and(|k| k.contains(&kind)) => format!(
                    "field type '{}' does not fit '{}' of node '{}' (a {kind} value)",
                    type_name(&p.leaf), node["prop"].as_str().unwrap_or(""), node["id"].as_str().unwrap_or("")),
                Some(_) => continue,
            };
            self.errors.push(SemaError::UiBind { path: format!("{root}.{name}"), message, span: self.with_runtime_ctx(span) });
        }
    }

    /// `ui.bind(racine, objet[, table de la racine])` : `None` si l'appel
    /// n'en est pas un, sinon son type (`void`), réécriture enregistrée.
    pub(crate) fn resolve_ui_bind(&mut self, callee: &Expr, obj_ty: &Type, method: &str, args: &[Expr], span: &Span) -> Option<Type> {
        if method != "bind" || !matches!(obj_ty, Type::Named(n) if n == "UI") {
            return None;
        }
        let (root, object, spec) = match args {
            [root, object] => (root, object, None),
            [root, object, Expr::Literal(Literal::String(spec), _)] => (root, object, Some(spec.as_str())),
            _ => return None,
        };
        self.infer_expr(root);
        let object_ty = self.infer_expr(object);
        let root_name = match root {
            Expr::Literal(Literal::String(name), _) => name.clone(),
            _ => String::new(),
        };
        let class = match &object_ty {
            Type::Named(class) if self.symbols.lookup_class(class).is_some() => class.clone(),
            other => {
                let message = format!("an object of a class is expected, found '{}'", type_name(other));
                self.errors.push(SemaError::UiBind { path: root_name.clone(), message, span: self.with_runtime_ctx(span) });
                return Some(Type::Void);
            }
        };
        let mut paths = Vec::new();
        self.bind_paths(&class, &[], &mut paths);
        if let Some(spec) = spec {
            self.check_bind_spec(&root_name, &class, &paths, spec, span);
        }
        let origin = format!("{}#ui.bind:{}:{}", span.file.as_deref().unwrap_or(""), span.line, span.col);
        let accessor = parse_generated(&accessor_source(&class, &paths), &origin)?;
        self.infer_expr(&accessor);
        let descriptors: Vec<serde_json::Value> = paths.iter()
            .map(|p| serde_json::json!({ "path": p.name(), "kinds": leaf_kinds(&p.leaf).unwrap_or_default() }))
            .collect();
        let descriptors = Expr::Literal(Literal::String(serde_json::Value::Array(descriptors).to_string()), span.clone());
        let call = Expr::Call {
            callee: Box::new(callee.clone()),
            args:   vec![root.clone(), object.clone(), descriptors, accessor],
            span:   span.clone(),
        };
        self.rewrites.calls.insert(site_key(span), call);
        Some(Type::Void)
    }
}
