/// Propriété des objets d'un conteneur (`array<Classe>`/`map<K, Classe>`) :
/// un conteneur libéré ne libère ses instances (`__array_free_objects`) que
/// si CHAQUE objet qui y entre est NEUF et qu'aucun n'en ressort — personne
/// d'autre ne peut alors les référencer. Sinon seule la structure est
/// libérée.
///
/// Un même parcours (`Scan`) sert, corps par corps :
/// - aux conteneurs locaux (`object_owners`) ;
/// - aux faits du programme (`object_facts`) : fonctions qui retournent des
///   objets neufs ou un conteneur neuf, paramètres qui préservent la
///   propriété, champs propriétaires de leurs objets.
///
/// Neuf : `use Classe(...)`, appel d'une fonction qui ne retourne que des
/// objets neufs, variable locale initialisée ainsi et référencée une seule
/// fois. Conteneur neuf : littéral d'objets neufs, appel d'une fonction qui
/// retourne un conteneur neuf, conteneur local propriétaire déplacé.
///
/// Disqualifie un candidat : élément non neuf, réaffectation, passage à un
/// paramètre qui ne préserve pas la propriété, méthode autre que `push`/
/// `len`, capture par une closure, conservation du conteneur ou d'un de ses
/// éléments (hors copie de chaîne/scalaire).
use std::collections::{HashMap, HashSet};

use crate::parsing::ast::{Block, Expr, Param, Stmt, TemplatePartExpr, Type, VarKind};
use super::object_ast::{collect_returns, count_refs_block, for_each_block};
pub use super::object_ast::{is_object_container, object_elem_class};

/// Faits du programme (voir `object_facts`) et contexte du corps analysé.
pub struct FreshCtx<'a> {
    pub fresh_returns:    &'a HashSet<String>,
    pub fresh_containers: &'a HashSet<String>,
    /// `"Classe_methode"` → le paramètre `i` préserve-t-il la propriété ?
    pub preserving:       &'a HashMap<String, Vec<bool>>,
    /// Types des paramètres des appelés (un scalaire passé est copié).
    pub param_types:      &'a HashMap<String, Vec<Type>>,
    /// Champs par classe (hérités compris) : type d'un accès `x.champ`.
    pub field_types:      &'a HashMap<String, Vec<(String, Type)>>,
    /// `"Classe.champ"` → `"ClasseDéclarante.champ"` : un champ hérité est
    /// le même stockage que celui du parent.
    pub field_decl:       &'a HashMap<String, String>,
    /// Types de retour déclarés (`"fonction"`, `"Classe_methode"`) : classe
    /// d'un receveur `f().champ`.
    pub ret_types:        &'a HashMap<String, Type>,
    pub current_class:    Option<&'a str>,
    /// Classe parente de `current_class` (`parent::m(...)`).
    pub parent_class:     Option<&'a str>,
}

/// Conteneurs locaux propriétaires de leurs objets dans `body`.
pub fn object_owners(body: &Block, ctx: &FreshCtx) -> HashSet<String> {
    let no_fields = HashSet::new();
    let mut scan = Scan::new(ctx, body, &[], &no_fields);
    scan.block(body);
    scan.finish();
    scan.owners()
}

/// Résultat d'un parcours, pour `object_facts`.
pub struct BodyFacts {
    /// Toutes les valeurs retournées sont des objets neufs.
    pub returns_fresh_object:    bool,
    /// Toutes les valeurs retournées sont des conteneurs neufs.
    pub returns_fresh_container: bool,
    /// Paramètres conteneurs qui préservent la propriété (`None` : pas un conteneur d'objets).
    pub preserving_params:       Vec<Option<bool>>,
    /// Champs (`ClasseDéclarante.champ`) disqualifiés dans ce corps.
    pub disqualified_fields:     HashSet<String>,
    /// `self.champ = param` : (champ, indice du paramètre).
    pub field_from_param:        Vec<(String, usize)>,
    /// Appels : (clé de l'appelé, ou `".methode"` si le receveur est inconnu ;
    /// pour chaque argument, est-ce un conteneur neuf ?).
    pub calls:                   Vec<(String, Vec<bool>)>,
    /// Fonctions/méthodes référencées comme valeur (appelables indirectement).
    pub value_refs:              HashSet<String>,
}

pub fn body_facts(body: &Block, params: &[Param], ctx: &FreshCtx, watched_fields: &HashSet<String>) -> BodyFacts {
    let mut scan = Scan::new(ctx, body, params, watched_fields);
    scan.allow_return = true;
    scan.block(body);
    scan.finish();
    let mut returns = Vec::new();
    collect_returns(body, &mut returns);
    let returns_fresh_object = !returns.is_empty() && returns.iter().all(|e| scan.is_fresh(e));
    let returns_fresh_container = !returns.is_empty() && returns.iter().all(|e| scan.returns_fresh_container(e));
    let preserving_params = params.iter()
        .map(|p| is_object_container(&p.ty).then(|| !scan.disqualified.contains(&p.name) && !scan.returned.contains(&p.name)))
        .collect();
    BodyFacts {
        returns_fresh_object,
        returns_fresh_container,
        preserving_params,
        disqualified_fields: scan.disqualified.iter().filter_map(|n| n.strip_prefix('#').map(str::to_string)).collect(),
        field_from_param: scan.field_from_param,
        calls: scan.calls,
        value_refs: scan.value_refs,
    }
}

/// Déplacement d'un conteneur local (voir `Scan::finish`).
struct Move {
    var:     String,
    /// Indice de l'appel dans `calls` (transfert en argument).
    call:    Option<usize>,
    /// Position de la référence déplacée.
    pos:     (usize, usize),
    /// Vers un champ de `self`, qui survit à la méthode.
    to_self: bool,
    /// Variable porteuse initialisée par l'appel de transfert, et blocs ouverts.
    holder:  Option<String>,
    path:    Vec<usize>,
    in_loop: bool,
    /// Champ de destination (déplacement vers un champ) : disqualifié si le
    /// conteneur n'était pas propriétaire.
    field:   Option<String>,
}

struct Scan<'a> {
    ctx: &'a FreshCtx<'a>,
    /// Variable locale (objet) initialisée par un objet neuf.
    fresh_vars: HashSet<String>,
    var_kinds:  HashMap<String, VarKind>,
    var_types:  HashMap<String, Type>,
    params:     Vec<String>,
    /// Champs suivis (`ClasseDéclarante.champ`), candidats `#Classe.champ` ;
    /// `#?champ` désigne un accès dont la classe du receveur est inconnue.
    watched:    &'a HashSet<String>,
    refs:       HashMap<String, usize>,
    /// Variable de boucle → candidat parcouru.
    aliases:    HashMap<String, String>,
    candidates: HashSet<String>,
    disqualified: HashSet<String>,
    returned:   HashSet<String>,
    allow_return: bool,
    /// Conteneurs locaux déplacés (vers un champ, un argument de transfert) :
    /// neufs seulement s'ils restent propriétaires jusqu'au bout.
    moved:      Vec<Move>,
    /// Profondeur de boucle courante, et à la déclaration de chaque variable.
    loop_depth: usize,
    var_loop_depth: HashMap<String, usize>,
    /// Positions des références à chaque variable (voir `RefPos`).
    positions:  HashMap<String, Vec<super::object_ast::RefPos>>,
    /// Chemin des blocs ouverts, numérotés comme `object_ast::ident_positions`.
    block_path: Vec<usize>,
    next_block: usize,
    /// Alias d'un conteneur suivi (`const cars = dto.cars`) : même conteneur.
    container_aliases: HashMap<String, String>,
    /// Variables conservées quelque part, ou réaffectées.
    kept_vars:  HashSet<String>,
    reassigned: HashSet<String>,
    /// Déclaration en cours dont la valeur est un appel (porteur d'un transfert).
    pending_holder: Option<String>,
    /// `x:Resolvable<…>` initialisé par un appel qui retourne un conteneur
    /// (resp. un objet) neuf : `resolve x` l'est aussi.
    fresh_task_containers: HashSet<String>,
    fresh_task_objects: HashSet<String>,
    field_from_param: Vec<(String, usize)>,
    calls:      Vec<(String, Vec<bool>)>,
    value_refs: HashSet<String>,
    /// Références d'un conteneur dues à `push` (voir `finish`).
    push_refs:  HashMap<String, usize>,
}

impl<'a> Scan<'a> {
    fn new(ctx: &'a FreshCtx<'a>, body: &Block, params: &[Param], watched: &'a HashSet<String>) -> Self {
        let mut refs = HashMap::new();
        count_refs_block(body, &mut refs);
        let positions = super::object_ast::ident_positions(body);
        let mut scan = Scan {
            ctx, fresh_vars: HashSet::new(), var_kinds: HashMap::new(),
            var_types: params.iter().map(|p| (p.name.clone(), p.ty.clone())).collect(),
            params: params.iter().map(|p| p.name.clone()).collect(),
            watched, refs, aliases: HashMap::new(), candidates: HashSet::new(), disqualified: HashSet::new(),
            returned: HashSet::new(), allow_return: false, moved: Vec::new(), field_from_param: Vec::new(), calls: Vec::new(),
            value_refs: HashSet::new(), push_refs: HashMap::new(),
            loop_depth: 0, var_loop_depth: HashMap::new(), positions,
            block_path: Vec::new(), next_block: 0, container_aliases: HashMap::new(),
            kept_vars: HashSet::new(), reassigned: HashSet::new(), pending_holder: None,
            fresh_task_containers: HashSet::new(), fresh_task_objects: HashSet::new(),
        };
        for p in params.iter().filter(|p| is_object_container(&p.ty)) {
            scan.candidates.insert(p.name.clone());
        }
        for f in watched {
            scan.candidates.insert(format!("#{}", f));
        }
        scan
    }

    fn owners(&self) -> HashSet<String> {
        self.candidates.iter()
            .filter(|c| !c.starts_with('#') && !self.params.contains(c) && !self.disqualified.contains(*c))
            .cloned()
            .collect()
    }

    /// Un conteneur local déplacé est disqualifié comme propriétaire local
    /// (il ne doit plus être libéré ici) ; s'il n'était pas propriétaire,
    /// la destination est disqualifiée à son tour.
    /// Un déplacement transfère la propriété s'il est unique, hors boucle,
    /// et qu'aucune référence au conteneur ne le suit — sauf vers un champ de
    /// `self`, qui survit à la méthode (le conteneur reste alors lisible).
    fn finish(&mut self) {
        let moved = std::mem::take(&mut self.moved);
        for m in &moved {
            let moves = moved.iter().filter(|o| o.var == m.var).count();
            let later: Vec<&Vec<usize>> = self.positions.get(&m.var)
                .map(|ps| ps.iter().filter(|(p, _)| *p > m.pos).map(|(_, path)| path).collect())
                .unwrap_or_default();
            // Relu après le transfert : le porteur doit vivre au moins
            // jusqu'aux lectures — champ de `self`, ou variable porteuse jamais
            // conservée ni réaffectée, lectures dans son bloc de déclaration.
            let holder_outlives = m.holder.as_ref().is_some_and(|h| {
                !self.kept_vars.contains(h) && !self.reassigned.contains(h)
                    && !matches!(self.var_kinds.get(h), Some(VarKind::Consumed))
                    && later.iter().all(|path| path.starts_with(&m.path))
            });
            let owner = self.candidates.contains(&m.var) && !self.disqualified.contains(&m.var)
                && matches!(self.var_kinds.get(&m.var), Some(VarKind::Var))
                && moves == 1 && !m.in_loop && (m.to_self || later.is_empty() || holder_outlives);
            if let Some(i) = m.call {
                if let Some((_, args)) = self.calls.get_mut(i) {
                    for a in args.iter_mut() { *a = *a && owner; }
                }
            }
            if let (false, Some(field)) = (owner, &m.field) {
                self.disqualified.insert(field.clone());
            }
        }
        for m in moved {
            self.disqualified.insert(m.var);
        }
        // `#?champ` : receveur inconnu, tous les champs de ce nom.
        let ambiguous: Vec<String> = self.disqualified.iter().filter_map(|k| k.strip_prefix("#?").map(str::to_string)).collect();
        for name in ambiguous {
            let suffix = format!(".{}", name);
            for w in self.watched.iter().filter(|w| w.ends_with(&suffix)) {
                self.disqualified.insert(format!("#{}", w));
            }
        }
    }

    fn new_move(&self, var: &str, call: Option<usize>, pos: (usize, usize), to_self: bool) -> Move {
        let in_loop = self.loop_depth > self.var_loop_depth.get(var).copied().unwrap_or(0);
        Move { var: var.to_string(), call, pos, to_self, in_loop, field: None, holder: None, path: self.block_path.clone() }
    }

    /// Type statique d'une expression, quand les déclarations le donnent.
    fn static_type(&self, expr: &Expr) -> Option<Type> {
        match expr {
            Expr::Ident(name, _) => self.var_types.get(name).cloned(),
            Expr::SelfExpr(_) => self.ctx.current_class.map(|c| Type::Named(c.to_string())),
            Expr::New { class, .. } => Some(Type::Named(class.clone())),
            Expr::Field { object, field, .. } => {
                let class = self.static_class(object)?;
                self.ctx.field_types.get(&class)?.iter().find(|(f, _)| f == field).map(|(_, t)| t.clone())
            }
            Expr::Index { object, .. } => match self.static_type(object)? {
                Type::Array(inner) | Type::Map(_, inner) => Some(*inner),
                _ => None,
            },
            Expr::Call { callee, .. } => match callee.as_ref() {
                Expr::Field { object, field, .. } if !matches!(object.as_ref(), Expr::SelfExpr(_)) => {
                    let class = self.static_class(object)?;
                    self.ctx.ret_types.get(&format!("{}_{}", class, field)).cloned()
                }
                _ => self.callee_key(expr).and_then(|k| self.ctx.ret_types.get(&k)).cloned(),
            },
            Expr::StaticCall { .. } => self.callee_key(expr).and_then(|k| self.ctx.ret_types.get(&k)).cloned(),
            Expr::Resolve { expr: task, .. } => match self.static_type(task)? {
                Type::Resolvable(inner) => Some(*inner),
                other => Some(other),
            },
            _ => None,
        }
    }

    fn static_class(&self, expr: &Expr) -> Option<String> {
        let ty = self.static_type(expr)?;
        match &ty {
            Type::Named(n) => Some(n.clone()),
            _ => crate::parsing::ast::union_named_class(&ty),
        }
    }

    /// Clé du champ suivi désigné par `object.field` : `#Déclarant.champ`,
    /// `#?champ` si la classe du receveur est inconnue, `None` si aucun champ
    /// suivi ne porte ce nom.
    fn field_key(&self, object: &Expr, field: &str) -> Option<String> {
        let suffix = format!(".{}", field);
        if !self.watched.iter().any(|w| w.ends_with(&suffix)) {
            return None;
        }
        match self.static_class(object) {
            Some(class) => {
                let decl = self.ctx.field_decl.get(&format!("{}.{}", class, field))?;
                self.watched.contains(decl).then(|| format!("#{}", decl))
            }
            None => Some(format!("#?{}", field)),
        }
    }

    /// Candidat désigné par `expr` : variable/paramètre candidat, champ suivi.
    fn cand_of(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Ident(name, _) if self.candidates.contains(name) => Some(name.clone()),
            Expr::Ident(name, _) => self.container_aliases.get(name).cloned(),
            Expr::Field { object, field, .. } => self.field_key(object, field),
            _ => None,
        }
    }

    /// Candidat dont `expr` est un ÉLÉMENT (index, variable de boucle, champ d'un élément).
    fn element_root(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Ident(name, _) => self.aliases.get(name).cloned(),
            Expr::Index { object, .. } => self.cand_of(object).or_else(|| self.element_root(object)),
            Expr::Field { object, field, .. } if self.field_key(object, field).is_none() => self.element_root(object),
            _ => None,
        }
    }

    fn callee_key(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Call { callee, .. } => match callee.as_ref() {
                Expr::Ident(name, _) => Some(name.clone()),
                Expr::Field { object, field, .. } if matches!(object.as_ref(), Expr::SelfExpr(_)) => {
                    Some(format!("{}_{}", self.ctx.current_class?, field))
                }
                Expr::Field { field, .. } => Some(format!(".{}", field)),
                _ => None,
            },
            Expr::StaticCall { class, method, .. } => match class.as_str() {
                "<self>" => Some(format!("{}_{}", self.ctx.current_class?, method)),
                "<parent>" => Some(match self.ctx.parent_class {
                    Some(parent) => format!("{}_{}", parent, method),
                    None => format!(".{}", method),
                }),
                c => Some(format!("{}_{}", c, method)),
            },
            Expr::New { class, .. } => Some(format!("{}_init", class)),
            _ => None,
        }
    }

    fn is_fresh(&self, expr: &Expr) -> bool {
        match expr {
            Expr::New { .. } | Expr::Literal(crate::parsing::ast::Literal::Null, _) => true,
            Expr::Call { .. } | Expr::StaticCall { .. } => self.callee_key(expr).is_some_and(|k| self.ctx.fresh_returns.contains(&k)),
            Expr::Ident(name, _) => self.fresh_vars.contains(name) && self.refs.get(name).copied().unwrap_or(0) <= 1,
            Expr::Resolve { expr: task, .. } => match task.as_ref() {
                Expr::Ident(name, _) => self.fresh_task_objects.contains(name) && self.refs.get(name).copied().unwrap_or(0) <= 1,
                other => self.is_fresh(other),
            },
            _ => false,
        }
    }

    /// Conteneur neuf (hors conteneur local déplacé, traité par `finish`).
    fn is_fresh_container_value(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Array { elements, .. } => elements.iter().all(|e| self.is_fresh(e)),
            Expr::Map { entries, .. } => entries.iter().all(|(_, v)| self.is_fresh(v)),
            Expr::Call { .. } | Expr::StaticCall { .. } => self.callee_key(expr).is_some_and(|k| self.ctx.fresh_containers.contains(&k)),
            // `resolve t` d'un appel `async` qui retourne un conteneur neuf.
            Expr::Resolve { expr: task, .. } => match task.as_ref() {
                Expr::Ident(name, _) => self.fresh_task_containers.contains(name) && self.refs.get(name).copied().unwrap_or(0) <= 1,
                other => self.is_fresh_container_value(other),
            },
            _ => false,
        }
    }

    fn returns_fresh_container(&self, expr: &Expr) -> bool {
        self.is_fresh_container_value(expr) || matches!(expr, Expr::Ident(n, _)
            if self.returned.contains(n) && !self.disqualified.contains(n) && !self.params.contains(n))
    }

    fn insertion(&mut self, container: &str, value: &Expr) {
        if let Expr::Ident(name, _) = value { self.kept_vars.insert(name.clone()); }
        if !self.is_fresh(value) {
            self.disqualified.insert(container.to_string());
        }
        self.expr(value);
    }

    /// Valeur conservée vers une cible de type `target` (inconnu : `None`).
    fn kept(&mut self, expr: &Expr, target: Option<&Type>) {
        if let Expr::Ident(name, _) = expr { self.kept_vars.insert(name.clone()); }
        if let Some(c) = self.cand_of(expr) {
            self.disqualified.insert(c);
        } else if let Some(root) = self.element_root(expr) {
            if !target.is_some_and(crate::lower::stmt::element_escape::copies_into) {
                self.disqualified.insert(root);
            }
        }
        match expr {
            Expr::Match { subject, arms, .. } => {
                self.expr(subject);
                arms.iter().for_each(|arm| self.kept(&arm.body, None));
            }
            Expr::Array { elements, .. } => elements.iter().for_each(|e| self.kept(e, None)),
            Expr::Map { entries, .. } => entries.iter().for_each(|(k, v)| { self.expr(k); self.kept(v, None); }),
            _ => self.expr(expr),
        }
    }

    fn block(&mut self, block: &Block) {
        self.block_path.push(self.next_block);
        self.next_block += 1;
        block.stmts.iter().for_each(|s| self.stmt(s));
        self.block_path.pop();
    }

    fn declare(&mut self, name: &str, ty: &Type, kind: VarKind, value: &Expr) {
        self.var_types.insert(name.to_string(), ty.clone());
        self.var_kinds.insert(name.to_string(), kind);
        self.var_loop_depth.insert(name.to_string(), self.loop_depth);
        if let Type::Resolvable(inner) = ty {
            if is_object_container(inner) && self.is_fresh_container_value(value) {
                self.fresh_task_containers.insert(name.to_string());
            } else if matches!(inner.as_ref(), Type::Named(_)) && self.is_fresh(value) {
                self.fresh_task_objects.insert(name.to_string());
            }
        }
        if is_object_container(ty) {
            match value {
                Expr::Array { elements, .. } => {
                    self.candidates.insert(name.to_string());
                    elements.iter().for_each(|e| self.insertion(name, e));
                }
                Expr::Map { entries, .. } => {
                    self.candidates.insert(name.to_string());
                    entries.iter().for_each(|(k, v)| { self.expr(k); self.insertion(name, v); });
                }
                _ if self.is_fresh_container_value(value) => {
                    self.candidates.insert(name.to_string());
                    self.expr(value);
                }
                // `var`/`const x = <conteneur suivi>` : alias, seuls ses
                // usages conservés comptent (`for c in x`, `Array::len(x)` non).
                _ if matches!(kind, VarKind::Var) && self.cand_of(value).is_some() => {
                    let root = self.cand_of(value).unwrap_or_default();
                    self.container_aliases.insert(name.to_string(), root);
                }
                _ => self.kept(value, Some(ty)),
            }
        } else {
            if matches!(value, Expr::New { .. } | Expr::Call { .. } | Expr::StaticCall { .. }) && !matches!(kind, VarKind::Consumed) {
                self.pending_holder = Some(name.to_string());
            }
            if matches!(ty, Type::Named(_)) && self.is_fresh(value) {
                self.fresh_vars.insert(name.to_string());
            }
            self.kept(value, Some(ty));
            self.pending_holder = None;
        }
    }

    fn assign(&mut self, target: &Expr, value: &Expr) {
        if let Expr::Index { object, index, .. } = target {
            if let Some(c) = self.cand_of(object) {
                self.expr(index);
                return self.insertion(&c, value);
            }
        }
        if let Some(c) = self.cand_of(target) {
            if c.starts_with('#') {
                let to_self = matches!(target, Expr::Field { object, .. } if matches!(object.as_ref(), Expr::SelfExpr(_)));
                self.field_assign(&c, to_self, value);
                return;
            }
            self.disqualified.insert(c);
        }
        if let Expr::Ident(name, _) = target {
            self.fresh_vars.remove(name);
            self.reassigned.insert(name.clone());
            self.container_aliases.remove(name);
        }
        self.expr(target);
        let ty = match target { Expr::Ident(n, _) => self.var_types.get(n).cloned(), _ => None };
        self.kept(value, ty.as_ref());
    }

    /// `x.champ = valeur` d'un champ suivi.
    fn field_assign(&mut self, field_key: &str, to_self: bool, value: &Expr) {
        if field_key.starts_with("#?") {
            self.disqualified.insert(field_key.to_string());
            return self.kept(value, None);
        }
        let field = field_key.trim_start_matches('#').to_string();
        if self.is_fresh_container_value(value) {
            return self.expr(value);
        }
        if let Expr::Ident(name, _) = value {
            if let Some(i) = self.params.iter().position(|p| p == name) {
                // Conservé par le champ : ce paramètre ne préserve plus la propriété.
                self.disqualified.insert(name.clone());
                self.field_from_param.push((field, i));
                return;
            }
            if self.candidates.contains(name) {
                let Expr::Ident(_, span) = value else { unreachable!() };
                let mut m = self.new_move(name, None, (span.line, span.col), to_self);
                m.field = Some(field_key.to_string());
                self.moved.push(m);
                if !matches!(self.var_kinds.get(name), Some(VarKind::Var)) {
                    self.disqualified.insert(field_key.to_string());
                }
                return;
            }
        }
        self.disqualified.insert(field_key.to_string());
        self.kept(value, None);
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Var { name, ty, value, kind, .. } => self.declare(name, ty, *kind, value),
            Stmt::Const { name, ty, value, .. } => self.declare(name, ty, VarKind::Var, value),
            Stmt::Assign { target, value, .. } => self.assign(target, value),
            Stmt::Expr(e) => self.expr(e),
            Stmt::Return { value: Some(v), .. } | Stmt::Result { value: Some(v), .. } => match self.cand_of(v) {
                Some(c) if self.allow_return && !c.starts_with('#') => { self.returned.insert(c); }
                _ => self.kept(v, None),
            },
            Stmt::Raise { value: v, .. } | Stmt::Emit { value: v, .. } => self.kept(v, None),
            Stmt::If { condition, .. } => {
                self.expr(condition);
                for_each_block(stmt, &mut |b| self.block(b));
            }
            Stmt::While { condition, .. } => {
                self.expr(condition);
                self.loop_depth += 1;
                for_each_block(stmt, &mut |b| self.block(b));
                self.loop_depth -= 1;
            }
            Stmt::Switch { subject, .. } => {
                self.expr(subject);
                for_each_block(stmt, &mut |b| self.block(b));
            }
            Stmt::ForIn { var, iter, .. } => self.loop_over(&[var], iter, stmt),
            Stmt::ForMap { key, value, iter, .. } => self.loop_over(&[key, value], iter, stmt),
            Stmt::Try { .. } => for_each_block(stmt, &mut |b| self.block(b)),
            Stmt::Return { .. } | Stmt::Result { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => {}
        }
    }

    fn loop_over(&mut self, vars: &[&String], iter: &Expr, stmt: &Stmt) {
        match self.cand_of(iter).or_else(|| self.element_root(iter)) {
            Some(root) => vars.iter().for_each(|v| { self.aliases.insert((*v).clone(), root.clone()); }),
            None => self.expr(iter),
        }
        // Type d'élément connu : la variable de boucle a une classe statique.
        if let Some(Type::Array(inner) | Type::Map(_, inner)) = self.static_type(iter) {
            if let Some(v) = vars.last() { self.var_types.insert((*v).clone(), *inner); }
        }
        self.loop_depth += 1;
        for_each_block(stmt, &mut |b| self.block(b));
        self.loop_depth -= 1;
    }

    fn call(&mut self, call: &Expr, args: &[Expr]) {
        if let Expr::StaticCall { class, method, .. } = call {
            if crate::sema::escape::is_pure_builtin(class, method) {
                return args.iter().for_each(|a| self.expr(a));
            }
        }
        let holder = self.pending_holder.take();
        let key = self.callee_key(call);
        let preserving = key.as_ref().and_then(|k| self.ctx.preserving.get(k)).cloned().unwrap_or_default();
        let types = key.as_ref().and_then(|k| self.ctx.param_types.get(k)).cloned().unwrap_or_default();
        let mut fresh_args = Vec::new();
        let call_index = self.calls.len();
        for (i, arg) in args.iter().enumerate() {
            fresh_args.push(self.is_fresh_container_value(arg));
            match self.cand_of(arg) {
                // Paramètre qui préserve la propriété : simple prêt.
                Some(_) if preserving.get(i).copied().unwrap_or(false) => {}
                // Sinon, peut-être un transfert (`use Dto(items)`) d'un
                // conteneur local, résolu par `finish`.
                Some(c) if !c.starts_with('#') && !self.params.contains(&c) => {
                    fresh_args[i] = true;
                    let pos = match arg { Expr::Ident(_, span) => (span.line, span.col), _ => (0, 0) };
                    let mut m = self.new_move(&c, Some(call_index), pos, false);
                    m.holder = holder.clone();
                    self.moved.push(m);
                }
                _ => self.kept(arg, types.get(i)),
            }
        }
        if let Some(k) = key { self.calls.push((k, fresh_args)); }
    }

    fn expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Call { callee, args, .. } => {
                if let Expr::Field { object, field, .. } = callee.as_ref() {
                    if let Some(c) = self.cand_of(object) {
                        match (field.as_str(), args.as_slice()) {
                            ("push", [value]) => {
                                *self.push_refs.entry(c.clone()).or_default() += 1;
                                return self.insertion(&c, value);
                            }
                            ("len", []) => return,
                            _ => { self.disqualified.insert(c); }
                        }
                    } else {
                        self.read(object);
                    }
                } else if !matches!(callee.as_ref(), Expr::Ident(..)) {
                    self.expr(callee);
                }
                self.call(expr, args);
            }
            Expr::StaticCall { class, method, args, .. } => {
                if class == "Array" && method == "push" && args.len() == 2 {
                    if let Some(c) = self.cand_of(&args[0]) {
                        return self.insertion(&c, &args[1]);
                    }
                }
                self.call(expr, args);
            }
            Expr::New { args, .. } => self.call(expr, args),
            Expr::Array { elements, .. } => elements.iter().for_each(|e| self.kept(e, None)),
            Expr::Map { entries, .. } => entries.iter().for_each(|(k, v)| { self.expr(k); self.kept(v, None); }),
            Expr::NamedArg { value, .. } => self.kept(value, None),
            Expr::Index { object, index, .. } => { self.read(object); self.expr(index); }
            Expr::Field { object, field, .. } => {
                self.value_refs.insert(format!(".{}", field));
                self.read(object);
            }
            Expr::Unary { operand: e, .. } | Expr::Resolve { expr: e, .. } | Expr::IsCheck { expr: e, .. } | Expr::IncDec { target: e, .. } => self.expr(e),
            Expr::Binary { left: a, right: b, .. } | Expr::Range { start: a, end: b, .. } => { self.expr(a); self.expr(b); }
            Expr::Template { parts, .. } => parts.iter().for_each(|p| {
                if let TemplatePartExpr::Expr(e) = p { self.expr(e); }
            }),
            Expr::Match { subject, arms, .. } => {
                self.expr(subject);
                arms.iter().for_each(|arm| self.kept(&arm.body, None));
            }
            Expr::Nameless { body, .. } => {
                let mut refs = HashSet::new();
                crate::sema::escape::collect_ident_refs(body, &mut refs);
                for name in refs {
                    if let Some(root) = self.aliases.get(&name).cloned() { self.disqualified.insert(root); }
                    self.disqualified.insert(name.clone());
                    self.fresh_vars.remove(&name);
                }
                // Un champ suivi manipulé dans une closure : prudence.
                let mut inner = HashMap::new();
                count_refs_block(body, &mut inner);
                for name in inner.keys().filter_map(|k| k.strip_prefix('#')) {
                    let suffix = format!(".{}", name);
                    if self.watched.iter().any(|w| w.ends_with(&suffix)) {
                        self.disqualified.insert(format!("#?{}", name));
                    }
                }
            }
            // Hors position d'appel : une fonction peut être passée comme valeur.
            Expr::Ident(name, _) => { self.value_refs.insert(name.clone()); }
            Expr::StaticConst { class, name, .. } => { self.value_refs.insert(format!("{}_{}", class, name)); }
            Expr::Literal(..) | Expr::SelfExpr(_) | Expr::ParentExpr(_) => {}
        }
    }

    /// Lecture (itération, index, champ) : sans effet sur la propriété.
    fn read(&mut self, expr: &Expr) {
        if self.cand_of(expr).is_none() && !matches!(expr, Expr::Ident(..)) {
            self.expr(expr);
        }
    }
}
