/// Lowering des classes

use std::rc::Rc;
use std::collections::{HashMap, HashSet};
use crate::parsing::ast::*;
use crate::ir::module::IrModule;
use crate::ir::types::IrType;
use super::functions::lower_func;

pub fn lower_class(
    module: &mut IrModule,
    class: &ClassDecl,
    all_classes: &[ClassDecl],
    all_modules: &[ModuleDecl],
    consts: &[crate::parsing::ast::ConstDecl],
    fn_ret_types: &Rc<HashMap<String, IrType>>,
    fn_param_types: &Rc<HashMap<String, Vec<IrType>>>,
    fn_param_names: &Rc<HashMap<String, Vec<String>>>,
    fn_variadic_info: &Rc<HashMap<String, (usize, IrType)>>,
    func_default_args: &Rc<HashMap<String, Vec<Option<Expr>>>>,
) {
    // Collecte des noms de méthodes propres (pour détecter les surcharges)
    let own_methods: HashSet<String> = class.members.iter()
        .filter_map(|m| if let ClassMember::Method { decl, .. } = m { Some(decl.name.clone()) } else { None })
        .collect();

    for member in &class.members {
        match member {
            ClassMember::Method { decl, is_static, .. } => {
                if *is_static {
                    // Méthode statique : pas de self, appelée via Class::method()
                    // Mais on passe class_name pour résoudre self::method() correctement
                    let mangled = FuncDecl {
                        name: format!("{}_{}", class.name, decl.name),
                        ..decl.clone()
                    };
                    if super::message_gen::is_message_func(&mangled.ret_ty) {
                        // Générateur statique (`emit`/`message<T>`) : pas de
                        // `self`, deux fonctions IR séparées — voir
                        // `super::message_gen::lower_message_func`.
                        super::message_gen::lower_message_func(module, &mangled, fn_ret_types, fn_param_types, fn_param_names);
                    } else {
                        lower_func(module, &mangled, consts, fn_ret_types, fn_param_types, fn_param_names, fn_variadic_info, func_default_args, Some(&class.name), class.extends.as_deref());
                    }
                } else {
                    // Méthode d'instance : self en premier paramètre
                    let self_param = crate::parsing::ast::Param {
                        name: "self".into(),
                        ty:   Type::Mixed,
                        is_variadic: false,
                        default_value: None,
                        span: decl.span.clone(),
                    };
                    let mut full_params = vec![self_param];
                    full_params.extend(decl.params.clone());
                    let mangled = FuncDecl {
                        name:   format!("{}_{}", class.name, decl.name),
                        params: full_params,
                        ..decl.clone()
                    };
                    if super::message_gen::is_message_func(&mangled.ret_ty) {
                        // Générateur d'instance : `self` est juste un champ
                        // de plus dans le frame heap (voir la doc de
                        // `message_gen`), aucun traitement spécial requis
                        // au-delà du `self` déjà préfixé aux params ci-dessus.
                        super::message_gen::lower_message_func(module, &mangled, fn_ret_types, fn_param_types, fn_param_names);
                    } else {
                        lower_func(module, &mangled, consts, fn_ret_types, fn_param_types, fn_param_names, fn_variadic_info, func_default_args, Some(&class.name), class.extends.as_deref());
                    }
                }
            }
            // Émis une seule fois plus bas, hérité compris (voir `nearest_constructor`).
            ClassMember::Constructor { .. } => {}
            ClassMember::Const { name, value, .. } => {
                use crate::ir::module::IrGlobal;
                let bytes = match value.const_literal() {
                    Some(Literal::Int(n))    => n.to_le_bytes().to_vec(),
                    Some(Literal::Float(f))  => f.to_le_bytes().to_vec(),
                    Some(Literal::Bool(b))   => vec![b as u8],
                    Some(Literal::String(s)) => s.as_bytes().to_vec(),
                    _ => vec![],
                };
                module.add_global(IrGlobal {
                    name: format!("{}__{}" , class.name, name),
                    bytes,
                });
            }
            ClassMember::Field { .. } => {}
        }
    }

    // Émettre les méthodes des modules pour cette classe
    for member in module_methods(class, all_modules) {
        if let ClassMember::Method { decl, is_static, .. } = member {
            let mut full_params = Vec::new();
            if !is_static {
                full_params.push(crate::parsing::ast::Param {
                    name: "self".into(),
                    ty:   Type::Mixed,
                    is_variadic: false,
                    default_value: None,
                    span: decl.span.clone(),
                });
            }
            full_params.extend(decl.params.clone());
            let mangled = FuncDecl {
                name:   format!("{}_{}", class.name, decl.name),
                params: full_params,
                ..decl.clone()
            };
            // Contexte de la classe (layouts corrects)
            lower_func(module, &mangled, consts, fn_ret_types, fn_param_types, fn_param_names, fn_variadic_info, func_default_args, Some(&class.name), class.extends.as_deref());
        }
    }

    // Émettre les méthodes héritées non surchargées comme Child_method — en
    // remontant TOUTE la chaîne d'ancêtres, pas seulement le parent direct :
    // une méthode déclarée par un GRAND-parent (ou plus loin), jamais
    // redéclarée par aucune classe intermédiaire, doit quand même être
    // copiée jusqu'à `class` — sinon elle n'existe tout simplement jamais
    // pour `class` dès que la chaîne dépasse un niveau (confirmé par
    // reproduction : `FilledCircle extends Circle extends Shape`, seul
    // `Shape` déclare `name()` — un appel PUREMENT STATIQUE `f.name()` sur
    // `f:FilledCircle` échouait déjà, indépendamment de tout dispatch
    // dynamique). `already_emitted`
    // accumule au fur et à mesure : une méthode déjà trouvée à un niveau
    // plus proche n'est jamais réémise depuis un ancêtre plus lointain
    // (priorité à la redéclaration la plus proche, sémantique normale de
    // l'héritage).
    if let Some(owner) = constructor_owner(all_classes, class) {
        let (params, body, span) = own_constructor(owner).expect("constructor_owner declares a constructor");
        let self_param = crate::parsing::ast::Param {
            name: "self".into(),
            ty:   Type::Mixed,
            is_variadic: false,
            default_value: None,
            span: span.clone(),
        };
        let mut full_params = vec![self_param];
        full_params.extend(params.iter().cloned());
        let init_func = FuncDecl {
            name:     format!("{}_init", class.name),
            params:   full_params,
            ret_ty:   Type::Void,
            body:     body.clone(),
            is_async: false,
            span:     span.clone(),
        };
        lower_func(module, &init_func, consts, fn_ret_types, fn_param_types, fn_param_names, fn_variadic_info, func_default_args, Some(&class.name), owner.extends.as_deref());
    }

    let mut already_emitted = own_methods;
    already_emitted.extend(module_methods(class, all_modules).into_iter()
        .filter_map(|m| if let ClassMember::Method { decl, .. } = m { Some(decl.name.clone()) } else { None }));
    let mut current_parent = class.extends.clone();
    while let Some(parent_name) = current_parent {
        let Some(parent) = all_classes.iter().find(|c| c.name == parent_name) else { break };
        // Méthodes du parent, y compris celles reçues de ses `modules`.
        for member in parent.members.iter().chain(module_methods(parent, all_modules)) {
            if let ClassMember::Method { decl, is_static, .. } = member {
                if !already_emitted.contains(&decl.name) {
                    already_emitted.insert(decl.name.clone());
                    // Méthode statique héritée : pas de `self` (comme une
                    // méthode statique propre) — sinon `Enfant::m(x)`
                    // appelait une fonction à un paramètre de trop.
                    let mut full_params = Vec::new();
                    if !is_static {
                        full_params.push(crate::parsing::ast::Param {
                            name: "self".into(),
                            ty:   Type::Mixed,
                            is_variadic: false,
                            default_value: None,
                            span: decl.span.clone(),
                        });
                    }
                    full_params.extend(decl.params.clone());
                    let mangled = FuncDecl {
                        name:   format!("{}_{}", class.name, decl.name),
                        params: full_params,
                        ..decl.clone()
                    };
                    // Contexte de la classe enfant (layouts corrects), mais
                    // `parent::` part du parent de la classe déclarante.
                    lower_func(module, &mangled, consts, fn_ret_types, fn_param_types, fn_param_names, fn_variadic_info, func_default_args, Some(&class.name), parent.extends.as_deref());
                }
            }
        }
        current_parent = parent.extends.clone();
    }
}

/// Méthodes reçues des `modules` de `class` qu'elle ne redéfinit pas, une par
/// nom : si deux modules définissent la même méthode, le dernier déclaré
/// l'emporte (EBNF §19.4).
pub fn module_methods<'a>(class: &ClassDecl, all_modules: &'a [ModuleDecl]) -> Vec<&'a ClassMember> {
    let mut seen: HashSet<&str> = class.members.iter()
        .filter_map(|m| if let ClassMember::Method { decl, .. } = m { Some(decl.name.as_str()) } else { None })
        .collect();
    let mut methods = Vec::new();
    for module_decl in class.modules.iter().rev().filter_map(|name| all_modules.iter().find(|m| &m.name == name)) {
        for member in &module_decl.members {
            if let ClassMember::Method { decl, .. } = member {
                if seen.insert(decl.name.as_str()) {
                    methods.push(member);
                }
            }
        }
    }
    methods
}

/// Constructeur effectif de `class` : le sien, sinon celui de l'ancêtre le
/// plus proche qui en déclare un — une sous-classe sans `init` propre hérite
/// de celui de son parent (`use Enfant(...)` appelle `Enfant_init`, qui doit
/// donc exister et initialiser les champs hérités).
pub fn nearest_constructor<'c>(all_classes: &'c [ClassDecl], class: &'c ClassDecl) -> Option<(&'c [Param], &'c Block, &'c crate::parsing::token::Span)> {
    own_constructor(constructor_owner(all_classes, class)?)
}

/// Classe qui déclare le constructeur effectif de `class` (elle-même ou
/// l'ancêtre le plus proche) — son `extends` est la cible de `parent::init`.
fn constructor_owner<'c>(all_classes: &'c [ClassDecl], class: &'c ClassDecl) -> Option<&'c ClassDecl> {
    let mut current = class;
    loop {
        if own_constructor(current).is_some() {
            return Some(current);
        }
        let parent = current.extends.as_ref()?;
        current = all_classes.iter().find(|c| &c.name == parent)?;
    }
}

fn own_constructor(class: &ClassDecl) -> Option<(&[Param], &Block, &crate::parsing::token::Span)> {
    class.members.iter().find_map(|m| match m {
        ClassMember::Constructor { params, body, span } => Some((params.as_slice(), body, span)),
        _ => None,
    })
}
