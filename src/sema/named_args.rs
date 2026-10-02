/// Arguments nommés à l'appel (`f(name: expr, ...)`) — voir
/// docs/roadmap.d/langage-named-arguments.md et `docs/EBNF.md` §14.
///
/// Règles :
///   - un appel est soit 100 % positionnel, soit 100 % nommé (E45) ;
///   - un nom doit désigner un paramètre de la cible (E46), une seule fois (E47) ;
///   - un paramètre variadic n'est jamais nommable (E48) ;
///   - tout paramètre sans valeur par défaut doit être fourni (E49) ;
///   - la cible doit avoir des noms de paramètres statiquement connus (E50) —
///     jamais le cas d'un appel à travers une valeur `Function<T(...)>`.
///
/// La sema résout chaque appel nommé en une liste POSITIONNELLE complète
/// (valeurs par défaut clonées pour tout paramètre omis), enregistrée par
/// site d'appel ; `core::named_args::rewrite_named_args` la réinjecte ensuite
/// dans l'AST, si bien que le lowering ne voit jamais d'argument nommé.

use std::borrow::Cow;
use std::collections::HashMap;
use crate::parsing::ast::{ClassMember, Expr, Param, Program};
use crate::parsing::token::Span;
use crate::sema::error::SemaError;
use crate::sema::symbols::FuncSig;
use crate::sema::typecheck::TypeChecker;

/// Identifie un site d'appel nommé par le span de son premier argument —
/// unique par appel (contrairement au span de l'appel lui-même, partagé par
/// les maillons d'une chaîne `a.b(...).c(...)`).
pub type ArgSiteKey = (Option<String>, usize, usize);

pub fn site_key(span: &Span) -> ArgSiteKey {
    (span.file.clone(), span.line, span.col)
}

/// Réécritures de l'AST décidées par la sema, appliquées après elle par
/// `core::named_args::rewrite_program`.
#[derive(Default)]
pub struct AstRewrites {
    /// Liste positionnelle résolue de chaque appel à arguments nommés (clé :
    /// span de son premier argument).
    pub args:  HashMap<ArgSiteKey, Vec<Expr>>,
    /// Appel remplacé par une autre expression (clé : span de l'appel) —
    /// sucre d'instance `Convert`, voir `crate::sema::convert_sugar`.
    pub calls: HashMap<ArgSiteKey, Expr>,
    /// `s -= e` sur une `string` (clé : span de l'opérateur) : réécrit en
    /// `String::replace(s, e, "")` — voir parser.d/compound_assign.rs.
    pub string_removals: std::collections::HashSet<ArgSiteKey>,
}

/// Paramètres déclarés par callable utilisateur : `"fonction"`,
/// `"Classe::méthode"`, `"Classe::init"` (constructeur) — source des noms ET
/// des valeurs par défaut (`FuncSig` ne porte pas ces dernières).
pub type CallableParams<'a> = HashMap<String, &'a [Param]>;

pub fn collect_callable_params(program: &Program) -> CallableParams<'_> {
    let mut table: CallableParams = HashMap::new();
    for f in &program.functions {
        table.insert(f.name.clone(), &f.params);
    }
    for c in &program.classes {
        insert_class_members(&mut table, &c.name, &c.members);
        for module in program.modules.iter().filter(|m| c.modules.contains(&m.name)) {
            insert_class_members(&mut table, &c.name, &module.members);
        }
    }
    for g in &program.generics {
        insert_class_members(&mut table, &g.name, &g.members);
    }
    for m in &program.modules {
        insert_class_members(&mut table, &m.name, &m.members);
    }
    table
}

fn insert_class_members<'a>(table: &mut CallableParams<'a>, owner: &str, members: &'a [ClassMember]) {
    for member in members {
        let (name, params) = match member {
            ClassMember::Method { decl, .. } => (decl.name.as_str(), decl.params.as_slice()),
            ClassMember::Constructor { params, .. } => ("init", params.as_slice()),
            _ => continue,
        };
        table.entry(format!("{}::{}", owner, name)).or_insert(params);
    }
}

pub struct ParamSlot {
    pub name:     String,
    pub default:  Option<Expr>,
    /// Omettable sans valeur par défaut connue (paramètre optionnel d'un
    /// builtin) — seulement en fin de liste, jamais pour combler un trou.
    pub optional: bool,
}

/// Cible d'un appel nommé, vue depuis le site d'appel.
pub struct CallTarget {
    pub callee:   String,
    pub slots:    Vec<ParamSlot>,
    pub variadic: Option<String>,
}

impl CallTarget {
    pub fn from_params(callee: String, params: &[Param]) -> Self {
        let (variadic, fixed) = match params.last() {
            Some(last) if last.is_variadic => (Some(last.name.clone()), &params[..params.len() - 1]),
            _ => (None, params),
        };
        let slots = fixed.iter()
            .map(|p| ParamSlot { name: p.name.clone(), default: p.default_value.clone(), optional: false })
            .collect();
        Self { callee, slots, variadic }
    }

    /// Cible sans déclaration AST (builtin) : noms depuis la signature, aucune
    /// valeur par défaut. `skip_receiver` retire le premier paramètre, passé
    /// implicitement par le sucre d'instance (`s.trim()` → `String::trim(s)`).
    pub fn from_sig(callee: String, sig: &FuncSig, skip_receiver: bool) -> Self {
        let skip = usize::from(skip_receiver);
        let fixed_end = if sig.has_variadic { sig.params.len().saturating_sub(1) } else { sig.params.len() };
        let variadic = if sig.has_variadic { sig.params.last().map(|(n, _)| n.clone()) } else { None };
        let slots = sig.params.iter()
            .enumerate()
            .take(fixed_end)
            .skip(skip)
            .map(|(i, (name, _))| ParamSlot { name: name.clone(), default: None, optional: i >= sig.required_params_count })
            .collect();
        Self { callee, slots, variadic }
    }
}

impl CallTarget {
    /// Paramètres sans valeur par défaut (ni omettables).
    pub fn required_count(&self) -> usize {
        self.slots.iter().filter(|s| s.default.is_none() && !s.optional).count()
    }

    pub fn accepts_arg_count(&self, count: usize) -> bool {
        count >= self.required_count() && (self.variadic.is_some() || count <= self.slots.len())
    }
}

pub fn has_named(args: &[Expr]) -> bool {
    args.iter().any(|a| matches!(a, Expr::NamedArg { .. }))
}

/// Valeurs des arguments sans leurs noms, dans l'ordre d'écriture.
fn strip_names(args: &[Expr]) -> Vec<Expr> {
    args.iter()
        .map(|a| match a {
            Expr::NamedArg { value, .. } => value.as_ref().clone(),
            other => other.clone(),
        })
        .collect()
}

/// Résout un appel dont au moins un argument est nommé en sa liste
/// positionnelle complète.
pub fn reorder(args: &[Expr], target: &CallTarget) -> Result<Vec<Expr>, SemaError> {
    let mut provided: Vec<Option<Expr>> = vec![None; target.slots.len()];
    for arg in args {
        let Expr::NamedArg { name, value, span } = arg else {
            return Err(SemaError::NamedArgMixed { callee: target.callee.clone(), span: arg.span().clone() });
        };
        if target.variadic.as_deref() == Some(name.as_str()) {
            return Err(SemaError::NamedArgVariadic { callee: target.callee.clone(), name: name.clone(), span: span.clone() });
        }
        let Some(index) = target.slots.iter().position(|s| &s.name == name) else {
            return Err(SemaError::NamedArgUnknown {
                callee: target.callee.clone(),
                name:   name.clone(),
                valid:  target.slots.iter().map(|s| s.name.clone()).collect(),
                span:   span.clone(),
            });
        };
        if provided[index].is_some() {
            return Err(SemaError::NamedArgDuplicate { callee: target.callee.clone(), name: name.clone(), span: span.clone() });
        }
        provided[index] = Some(value.as_ref().clone());
    }

    let last_provided = provided.iter().rposition(Option::is_some).unwrap_or(0);
    let mut positional = Vec::with_capacity(target.slots.len());
    for (i, (slot, value)) in target.slots.iter().zip(provided).enumerate() {
        match (value, &slot.default) {
            (Some(v), _) => positional.push(v),
            (None, Some(default)) => positional.push(default.clone()),
            (None, None) if slot.optional && i > last_provided => break,
            (None, None) => {
                return Err(SemaError::NamedArgMissing {
                    callee: target.callee.clone(),
                    name:   slot.name.clone(),
                    span:   args[0].span().clone(),
                });
            }
        }
    }
    Ok(positional)
}

impl<'a> TypeChecker<'a> {
    /// Arguments avec lesquels vérifier un appel : `args` tel quel s'il est
    /// positionnel, sinon sa forme positionnelle résolue contre `target`
    /// (enregistrée pour la réécriture de l'AST). Sans cible connue, `args`
    /// est rendu tel quel : l'inférence de chaque `Expr::NamedArg` signale
    /// alors E50. `None` si l'appel nommé est invalide : l'erreur est déjà
    /// signalée et les valeurs inférées, l'appelant ne vérifie rien de plus
    /// (pas d'erreur d'arité en cascade).
    pub(crate) fn resolve_named_call<'e>(
        &mut self,
        args: &'e [Expr],
        target: impl FnOnce(&Self) -> Option<CallTarget>,
    ) -> Option<Cow<'e, [Expr]>> {
        if !has_named(args) {
            return Some(Cow::Borrowed(args));
        }
        let Some(target) = target(self) else {
            return Some(Cow::Borrowed(args));
        };
        match reorder(args, &target) {
            Ok(positional) => {
                self.rewrites.args.insert(site_key(args[0].span()), positional.clone());
                Some(Cow::Owned(positional))
            }
            Err(error) => {
                self.errors.push(error);
                for value in strip_names(args) {
                    self.infer_expr(&value);
                }
                None
            }
        }
    }

    /// Cible d'une fonction libre : déclaration utilisateur, sinon signature.
    pub(crate) fn function_target(&self, name: &str, sig: &FuncSig) -> CallTarget {
        match self.callable_params.get(name) {
            Some(params) => CallTarget::from_params(name.to_string(), params),
            None => CallTarget::from_sig(name.to_string(), sig, false),
        }
    }

    /// Paramètres déclarés de `class::method`, en remontant la chaîne `extends`.
    pub(crate) fn user_method_target(&self, class: &str, method: &str) -> Option<CallTarget> {
        let mut owner = class.to_string();
        loop {
            if let Some(params) = self.callable_params.get(&format!("{}::{}", owner, method)) {
                return Some(CallTarget::from_params(format!("{}::{}", class, method), params));
            }
            owner = self.symbols.lookup_parent_class(&owner)?;
        }
    }

    /// Cible d'une méthode : déclaration utilisateur, sinon signature builtin.
    pub(crate) fn method_target(&self, class: &str, method: &str, sig: &FuncSig, skip_receiver: bool) -> CallTarget {
        self.user_method_target(class, method)
            .unwrap_or_else(|| CallTarget::from_sig(format!("{}::{}", class, method), sig, skip_receiver))
    }
}
