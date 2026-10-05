use std::collections::{HashMap, HashSet};
use crate::ir::func::IrFunction;
use crate::ir::types::IrType;
use crate::parsing::ast::{Literal, Type};

// ─────────────────────────────────────────────────────────────────────────────
// IrModule — représentation complète d'un programme compilé
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct IrModule {
    pub name:      String,
    pub source_file: String,
    pub functions: Vec<IrFunction>,
    /// Table des chaînes littérales (index → contenu)
    pub strings:   Vec<String>,
    /// Constantes globales (nom → valeur scalaire en bytes)
    pub globals:   Vec<IrGlobal>,
    /// Modules importés (ex: ["IO", "Array", "Math"])
    pub imports:   Vec<String>,
    /// `import ocara.X as Y` → alias local ("Y") → nom builtin canonique
    /// ("X"), UNIQUEMENT pour les imports `ocara.*` (voir `lower_program`).
    /// Nécessaire pour mangler correctement un appel statique
    /// `Y::method(args)` vers le vrai symbole runtime `X_method` — sans
    /// cette résolution, `resolved_class` dans `Expr::StaticCall` (lower.rs)
    /// valait l'alias tel quel, produisant un nom mangled qui ne correspond
    /// à AUCUNE fonction connue ; `emit_calls` (codegen) ignore alors
    /// silencieusement l'appel inconnu ("résolution runtime" pour un
    /// dispatch dynamique légitime ailleurs), laissant la variable de
    /// destination non définie — un bug de résultat FAUX SILENCIEUX, pas un
    /// crash, confirmé par reproduction sur `Request::isError(res)` avec
    /// `import ocara.HTTPRequest as Request` (retournait `false` au lieu du
    /// vrai résultat). Les classes utilisateur ne sont PAS concernées : leur
    /// alias est déjà résolu vers le nom réel de la classe par un mécanisme
    /// séparé, bien avant le lowering.
    pub import_aliases: HashMap<String, String>,
    /// Layout des classes : class_name → liste ordonnée (field_name, field_type)
    pub class_layouts: HashMap<String, Vec<(String, IrType)>>,
    /// Comme `class_layouts`, mais avec le vrai type AST de chaque champ
    /// (pas `IrType`, qui réduit string/array/map/instance à `Ptr`, tous
    /// indistinguables) — indispensable pour générer les destructeurs/
    /// clones réels des instances de classe utilisateur (`scoped`/
    /// `consumed MaClasse`, voir `src/lower/builder.d/class_ownership.rs`).
    /// UNIQUEMENT pour les classes utilisateur (`program.classes`) — sert
    /// aussi de test d'appartenance : une classe absente d'ici (builtin/
    /// opaque comme Mutex/SDL/Exception) n'a PAS de `__free_<Classe>`/
    /// `__clone_<Classe>` généré, donc ne doit jamais être traitée comme un
    /// objet possédable (voir `class_ownership::has_generated_destructor`).
    pub class_field_types: HashMap<String, Vec<(String, Type)>>,
    /// Champs de type map<K,V> par classe (hérités inclus) : class_name → noms de
    /// champs. `class_layouts` réduit tout à IrType::Ptr (map/array/string
    /// indistinguables) — indispensable pour que `self.champMap[clé] = v` émette
    /// `__map_set` plutôt que `__array_set` (voir Expr::Index dans lower.rs et
    /// assignments.rs : la distinction map/array pour un accès par index ne peut
    /// se faire que via ce genre de méta-info, jamais via IrType seul).
    pub class_map_fields: HashMap<String, HashSet<String>>,
    /// Héritage : class_name → parent_name
    pub class_parents: HashMap<String, String>,
    /// Identité de classe à l'exécution : class_name → id entier unique
    /// (attribué une fois, à la compilation, dans l'ordre de déclaration —
    /// voir program.rs). Stocké dans le header de CHAQUE instance
    /// (`__alloc_class_obj`, `runtime/src/lib.rs`) pour permettre un
    /// polymorphisme réel : `is ClassName`/`is InterfaceName` et le dispatch
    /// dynamique d'une méthode appelée via une variable de type parent/
    /// interface (voir docs/roadmap.d/langage-interfaces.md).
    pub class_ids: HashMap<String, i64>,
    /// Classes ayant au moins une sous-classe (directe ou transitive) —
    /// calculé par `generate_class_dispatchers`. Une méthode d'instance
    /// appelée sur une variable/champ typé par une classe de cet ensemble
    /// est redirigée vers son dispatcher dynamique `__dispatch_Classe_
    /// méthode` (voir `class_dispatch::class_dispatcher_name`) plutôt que
    /// vers l'implémentation concrète directement — sans dispatch, l'appel
    /// résoudrait TOUJOURS vers cette classe précise, jamais vers une
    /// éventuelle surcharge du type réel de l'objet.
    pub classes_with_subclasses: HashSet<String>,
    /// Ensemble de `class_id` correspondant à un `is X` réel (voir
    /// `lower_is_check` dans `src/lower/expr.d/literals.rs`) : pour une
    /// CLASSE, elle-même et tous ses descendants transitifs ; pour une
    /// INTERFACE, les classes qui l'implémentent DIRECTEMENT (même règle que
    /// le diagnostic E09/le dispatch dynamique — pas de transitivité via
    /// `extends` pour `implements`). `x is Circle` et `x is Drawable`
    /// compilaient jusqu'ici en code STRICTEMENT IDENTIQUE (`__is_object`
    /// seul, sans jamais regarder la classe RÉELLE de `x`) — voir
    /// docs/roadmap.d/langage-interfaces.md.
    pub is_check_candidates: HashMap<String, Vec<i64>>,
    /// Paramètres échappants par fonction/méthode/constructeur utilisateur
    /// (voir `crate::sema::escape`) — calculé une fois dans `lower_program`,
    /// consulté par `lower::stmt::ownership` pour décider si un `var` peut
    /// être libéré automatiquement en fin de bloc (voir
    /// docs/roadmap.d/memoire-strategie-var.md).
    pub escaping_params: HashMap<crate::sema::escape::CalleeKey, Vec<bool>>,
    /// `class_name → membres appelables` — vue minimale du programme utilisée
    /// par `escape::resolve_user_callable` côté lowering (pas d'accès direct
    /// à `&Program` à cet endroit).
    pub class_members: crate::sema::escape::ClassMembers,
    /// Types des paramètres du constructeur : class_name → Vec<IrType>
    pub ctor_param_types: HashMap<String, Vec<IrType>>,
    /// Types AST déclarés des paramètres des fonctions, méthodes
    /// (`"Classe_methode"`) et constructeurs (`"Classe_init"`) utilisateur —
    /// un littéral `[...]`/`{...}` passé en argument est construit au type
    /// du paramètre (voir `lower_call_arg`).
    pub param_ast_types: HashMap<String, Vec<Type>>,
    /// Même clé → le paramètre `i` est-il conservé par l'appelé (lui ou une
    /// partie) ? Voir `lower::stmt::element_escape::compute_param_keeps`.
    pub param_keeps: HashMap<String, Vec<bool>>,
    /// Fonctions/méthodes qui ne retournent que des objets neufs (voir
    /// `lower::stmt::object_owners::compute_fresh_returns`).
    pub fresh_returns: std::collections::HashSet<String>,
    /// Types des paramètres des méthodes D'INSTANCE utilisateur (jamais
    /// statiques, déjà couvertes par `LowerBuilder::fn_param_types`) :
    /// "Classe_methode" → Vec<IrType> (sans `self`). Utilisé UNIQUEMENT pour
    /// la décision de boxing `mixed` d'un argument d'appel (voir
    /// `crate::lower::expr::helpers::param_type_for_call_arg`) — délibérément
    /// séparé de `fn_param_types`, qui alimente aussi la génération des
    /// wrappers `__fn_wrap_*` pour les fonctions référençables comme valeur
    /// (voir program.rs) : un wrapper généré pour une méthode d'instance sans
    /// tenir compte de `self` serait cassé (appel désaligné).
    pub method_param_types: HashMap<String, Vec<IrType>>,
    /// Constantes de classes : "ClassName__NAME" → (IrType, Literal)
    pub class_consts: HashMap<String, (IrType, Literal)>,
    /// Compteur pour nommer les closures anonymes (__anon_0, __anon_1, ...)
    pub anon_counter: usize,
    /// Fonctions/méthodes générateurs (`emit`/`message<T>`, voir
    /// docs/roadmap.d/langage-emit-iterable.md) : nom mangled ("truc",
    /// "Classe_methode") → type IR de la valeur émise. Consulté par les 3
    /// formes de consommation (`for`, scalaire directe, `Array::fromMessage`)
    /// pour savoir qu'un appel `nom(...)` doit passer par `nom__new`/
    /// `nom__resume` plutôt que par un appel normal — voir
    /// `src/lower/builder.d/message_gen.rs`.
    pub message_funcs: HashMap<String, IrType>,
    /// Compteur pour nommer les fonctions try/handler (__try_body_0, __try_handler_0, ...).
    /// Doit être un compteur dédié, PAS `functions.len()` : un `try` imbriqué dans le
    /// corps d'un autre `try` est lowered (et ajoute ses propres fonctions au module)
    /// AVANT que le `try` englobant n'ajoute les siennes (voir lower_try) — dériver
    /// l'id de `functions.len()` fait alors lire la même longueur pour l'englobant et
    /// l'imbriqué, produisant deux fonctions au même nom (collision de signature au
    /// codegen). Même patron que `anon_counter` ci-dessus, qui n'a pas ce problème.
    pub try_counter: usize,
    /// Type de retour CONCRET d'une fonction LIBRE : nom de fonction → nom
    /// de classe utilisateur (`Type::Named`) OU famille builtin
    /// `"String"`/`"Array"`/`"Map"` (même convention que
    /// `resolve_chained_field_class` pour un champ) — voir
    /// docs/roadmap.d/langage-chained-call-on-free-function-result.md.
    /// Nécessaire pour résoudre un appel de méthode chaîné directement sur
    /// le résultat d'une fonction libre (`maFonction(...).methode()`, sans
    /// jamais passer par une variable nommée) : `fn_ret_types` (sur
    /// `LowerBuilder`) donne seulement l'`IrType` réduit (`Ptr` pour
    /// n'importe quel type référence — classe, string, array, map,
    /// indistinguables), jamais LE nom de classe précis nécessaire pour
    /// mangler `Classe_methode`. Même famille de bug que
    /// [langage-use-chaine-valeur-retour-perdue](../../docs/roadmap.d/langage-use-chaine-valeur-retour-perdue.md)
    /// (`use Classe(...).methode()`) et
    /// [langage-interface-wiring](../../docs/roadmap.d/langage-interface-wiring.md)'s
    /// `HTTPRequest::get(...).methode()`, mais pour un TROISIÈME
    /// déclencheur (appel de fonction libre) jamais couvert par ces
    /// correctifs — avant, `func_mangled` valait `"_method_<methode>"`, un
    /// symbole qui n'existe jamais, silencieusement sans effet au codegen.
    pub func_ret_class: HashMap<String, String>,
    /// Comme `func_ret_class`, mais pour une MÉTHODE (statique ou
    /// d'instance) d'une classe UTILISATEUR : `"Classe_methode"` → nom de
    /// classe retournée (classe utilisateur, famille builtin
    /// `"String"`/`"Array"`/`"Map"`, ou nom monomorphisé d'un générique).
    /// Absent avant ce correctif — `fn_ret_types` (IrType seul) n'était
    /// JAMAIS peuplé pour une méthode de classe ORDINAIRE (seulement
    /// fonctions libres, méthodes d'INTERFACE, et builtins), donc résoudre
    /// la classe d'un appel de méthode chaîné (`w.getCircle().shapeName()`)
    /// n'avait tout simplement AUCUNE source de vérité — voir
    /// `crate::lower::expr::helpers::resolve_receiver_class` et
    /// docs/roadmap.d/langage-chained-call-depth-limit.md. Avant ce
    /// correctif, la seule heuristique existante (voir l'historique dans
    /// `lower.rs`/`typeinfer.rs`) supposait à tort qu'une méthode chaînée
    /// retournant `Ptr` retournait la MÊME classe que son récepteur — faux
    /// dès que la méthode change de classe (`Wrapper::getCircle(): Circle`),
    /// silencieusement (mangle vers un symbole qui n'existe pas, ignoré par
    /// le codegen).
    pub method_ret_class: HashMap<String, String>,
    /// Type de retour DÉCLARÉ (AST complet, pas seulement sa classe) de
    /// chaque fonction libre (`"nom"`), méthode utilisateur et méthode
    /// builtin (`"Classe_methode"`) — type d'élément d'un résultat d'appel
    /// indexé directement (`make()[1][1]` pour `array<array<int>>`), sans
    /// lequel un scalaire brut était pris pour un pointeur (voir
    /// `crate::lower::expr::helpers::elem_type_after_index`).
    pub call_ret_types: HashMap<String, Type>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct IrGlobal {
    pub name:  String,
    pub bytes: Vec<u8>,
}

impl IrModule {
    pub fn new(name: impl Into<String>) -> Self {
        Self { 
            name: name.into(), 
            source_file: String::new(),
            ..Default::default() 
        }
    }

    /// Enregistre une chaîne littérale et retourne son index
    pub fn intern_string(&mut self, s: &str) -> u32 {
        if let Some(i) = self.strings.iter().position(|x| x == s) {
            return i as u32;
        }
        let i = self.strings.len() as u32;
        self.strings.push(s.to_string());
        i
    }

    /// Chaîne d'ancêtres d'une classe (elle-même incluse en premier), du plus
    /// spécifique au plus général — ex. `"FileNotFound|FileException|Exception"`
    /// — jointe par `|` (un nom de classe Ocara ne peut pas contenir ce
    /// caractère). Utilisée par `raise` (voir `lower_raise`) pour que
    /// `__ocara_type_matches` (runtime/src/lib.rs) puisse faire correspondre
    /// un filtre `on e is X` à une SOUS-CLASSE de `X`, pas seulement à `X`
    /// lui-même — voir docs/roadmap.d/langage-exceptions.md. `class_parents`
    /// couvre aussi bien les classes utilisateur (`extends`) que les
    /// exceptions builtin (voir `lower_program`).
    pub fn ancestor_chain(&self, class_name: &str) -> String {
        let mut chain = vec![class_name.to_string()];
        let mut seen: HashSet<&str> = HashSet::new();
        seen.insert(class_name);
        let mut current: &str = class_name;
        while let Some(parent) = self.class_parents.get(current) {
            if !seen.insert(parent.as_str()) {
                break; // extends cyclique — garde-fou, ne devrait jamais arriver
            }
            chain.push(parent.clone());
            current = parent.as_str();
        }
        chain.join("|")
    }

    pub fn add_function(&mut self, func: IrFunction) {
        self.functions.push(func);
    }

    pub fn add_global(&mut self, global: IrGlobal) {
        self.globals.push(global);
    }
}
