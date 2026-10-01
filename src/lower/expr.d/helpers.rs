/// Helpers pour le lowering des expressions

use std::collections::HashMap;
use std::sync::OnceLock;
use crate::parsing::ast::{Expr, Literal, Type};
use crate::ir::inst::{Inst, Value};
use crate::ir::types::IrType;
use crate::lower::builder::LowerBuilder;
use crate::codegen::runtime::builtins;

/// Types de paramètres (I64/F64/Bool/Ptr) de chaque méthode BUILTIN (statique
/// OU d'instance), indexé par nom manglé `"Classe_methode"` — construit une
/// seule fois à partir de `crate::builtins::all_builtins()`.
///
/// Utilisé UNIQUEMENT pour la décision de boxing `mixed` des arguments d'un
/// appel de méthode/statique (voir `box_arg_for_mixed_param` et ses
/// appelants) — délibérément séparé de `LowerBuilder::fn_param_types`
/// (réservé au programme utilisateur : fonctions libres + méthodes
/// statiques) car CETTE table alimente aussi la génération des wrappers
/// `__fn_wrap_*` pour les fonctions référençables comme valeur (voir
/// `program.rs`) — y ajouter les ~centaines de méthodes builtin générerait
/// autant de wrappers jamais utilisés dans chaque programme compilé.
///
/// ATTENTION avant d'ajouter un nouveau consommateur de cette table : une
/// méthode builtin est écrite en Rust et peut lire directement le bit brut
/// d'un `mixed` plutôt que par les accesseurs "boxing-aware"
/// (`cmp_primitive`/`unbox_numeric_i64`/...). `UnitTest_assertTrue`/
/// `assertFalse` en sont un exemple réel, corrigé pour rester compatibles
/// (voir runtime/src/lib.rs) après une régression confirmée pendant ce
/// chantier (`make regression` cassé universellement sur `assertTrue`/
/// `assertFalse`, un bool boxé — donc un pointeur, toujours non nul — brisant
/// leur test `value != 0`/`== 0`). Si un autre builtin `mixed` regresse après
/// avoir touché cette table, la même correction (rendre CE builtin
/// boxing-aware) est la bonne réponse — pas retirer le boxing lui-même,
/// qui reste nécessaire pour les mêmes raisons que documentées dans
/// docs/roadmap.d/memoire-fiabilite-runtime-bas-niveau.md.
pub fn builtin_method_param_types() -> &'static HashMap<String, Vec<IrType>> {
    static CACHE: OnceLock<HashMap<String, Vec<IrType>>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let mut map = HashMap::new();
        for (class_name, info) in crate::builtins::all_builtins() {
            for (method_name, sig) in &info.methods {
                let mangled = format!("{}_{}", class_name, method_name);
                let param_types: Vec<IrType> = sig.params.iter()
                    .map(|(_, ty)| IrType::from_ast(ty))
                    .collect();
                map.insert(mangled, param_types);
            }
        }
        map
    })
}

/// Forme d'un appel de méthode/fonction — distingue la forme STATIQUE
/// (`Classe::methode(obj, args...)` ou une fonction libre, où le récepteur
/// éventuel est le premier argument EXPLICITE) du SUCRE d'instance
/// (`obj.methode(args...)`, où le récepteur n'apparaît JAMAIS dans `args`).
/// Seule `builtin_method_param_types()` a besoin de cette distinction (voir
/// `param_type_for_call_arg`) — `fn_param_types`/`module.method_param_types`
/// (méthodes utilisateur) ne déclarent jamais de récepteur implicite, `i` s'y
/// applique identiquement pour les deux formes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CallForm {
    Static,
    Sugar,
}

/// Type du `i`-ème paramètre (fixe) de `mangled` ("Classe_methode" ou nom de
/// fonction libre), pour un appel de forme `form` — SEULE source de vérité
/// pour les formes statique ET sucre (même patron que
/// `resolve_method_return_type` dans `typeinfer.rs` pour le type de RETOUR ;
/// voir docs/roadmap.d/qualite-parite-sucre-statique-param-types.md). Avant
/// ce regroupement, `param_type_for_call_arg`/`param_type_for_sugar_call_arg`
/// étaient deux fonctions séparées avec un décalage d'index maintenu à la
/// main entre les deux — même classe de risque que le SEGFAULT déjà confirmé
/// (voir plus bas) : un correctif appliqué à l'une n'était jamais
/// automatiquement répercuté sur l'autre.
///
/// Cherche dans l'ordre : `LowerBuilder::fn_param_types` (fonctions libres +
/// méthodes STATIQUES utilisateur), puis `IrModule::method_param_types`
/// (méthodes D'INSTANCE utilisateur) — `i` s'y applique identiquement pour
/// les deux formes, aucun récepteur implicite n'y est jamais déclaré — puis
/// `builtin_method_param_types()` (toute méthode builtin, statique ou
/// d'instance) : chaque signature y est écrite comme la forme STATIQUE
/// (`Array::get(arr, idx)`), récepteur INCLUS en position 0 — la même table
/// sert aux deux formes. Pour `form == CallForm::Sugar`, l'argument explicite
/// `i` correspond donc à la position `i + 1` de cette table, jamais `i` (le
/// récepteur n'apparaît jamais dans les arguments explicites du sucre).
///
/// `None` si `mangled`/`i` reste inconnu (variadic au-delà des paramètres
/// fixes, méthode non répertoriée...) — le boxing est alors simplement
/// sauté, comme avant l'introduction de ce mécanisme.
///
/// Bug historique que cette distinction corrige : utiliser l'index `i` sans
/// décalage pour le sucre décalait chaque lookup d'une position pour TOUT
/// builtin à double forme — invisible tant que la position ainsi confondue
/// avec la vraie visait aussi `Ptr` (le cas le plus fréquent, `val`/`sep`...),
/// mais un décalage réel pour `arr.get(idx)` : `idx` (type réel `Int`, jamais
/// à boxer) se voyait attribuer le type du RÉCEPTEUR (`arr`, toujours `Ptr`),
/// et était donc boxé à tort par `box_arg_for_mixed_param` — sans effet
/// observable tant que `box_int_if_needed` ne boxait jamais un `int` en
/// dessous du seuil pointeur (l'index boxé à tort redevenait la valeur brute
/// une fois déboxé). Confirmé par un SEGFAULT reproductible dès que `0` a dû
/// devenir boxé lui aussi (voir docs/roadmap.d/langage-array-get-display-bug.md,
/// section ambiguïté `0`/`null`) : `arr.get(0)` boxait l'index `0`,
/// `__array_get` recevait alors un pointeur boxé arbitraire au lieu de
/// l'entier `0`, hors-borne — retournait `null`, déréférencé ensuite comme un
/// objet valide.
pub fn param_type_for_call_arg(builder: &LowerBuilder, mangled: &str, i: usize, form: CallForm) -> Option<IrType> {
    if let Some(pts) = builder.fn_param_types.get(mangled) {
        return pts.get(i).cloned();
    }
    if let Some(pts) = builder.module.method_param_types.get(mangled) {
        return pts.get(i).cloned();
    }
    let builtin_index = match form {
        CallForm::Static => i,
        CallForm::Sugar   => i + 1,
    };
    builtin_method_param_types().get(mangled).and_then(|pts| pts.get(builtin_index).cloned())
}

/// Boxe `val` (déjà lowered, de type IR `arg_ty`) si le paramètre cible
/// (`param_ty`) est `mixed` (`Ptr`) et que `arg_ty` est F64/Bool/I64 connu
/// statiquement — même logique que `box_for_any`/`box_for_dyn_arith` pour la
/// direction concret→mixed (voir `src/lower/stmt.d/statements.d/helpers.rs`),
/// factorisée ici pour les arguments d'appel (fonction libre, méthode
/// d'instance, appel statique, constructeur) : sans ce boxing, un `float`/
/// `bool` passé où le paramètre est `mixed` reste indistinguable d'un entier
/// au runtime, et un `int` assez grand pour ressembler à un pointeur heap
/// risque le SEGFAULT documenté dans
/// docs/roadmap.d/memoire-fiabilite-runtime-bas-niveau.md — confirmé par
/// reproduction sur un appel de méthode AVANT ce correctif (`b.show(3.5)`
/// avec `show(v:mixed)` corrompait déjà silencieusement `v`).
pub fn box_arg_for_mixed_param(builder: &mut LowerBuilder, param_ty: Option<IrType>, arg_ty: &IrType, val: Value) -> Value {
    if param_ty != Some(IrType::Ptr) {
        return val;
    }
    match arg_ty {
        IrType::F64 => {
            let d = builder.new_value();
            builder.emit(Inst::Call { dest: Some(d.clone()), func: "__box_float".into(), args: vec![val], ret_ty: IrType::Ptr });
            d
        }
        IrType::Bool => {
            let d = builder.new_value();
            builder.emit(Inst::Call { dest: Some(d.clone()), func: "__box_bool".into(), args: vec![val], ret_ty: IrType::Ptr });
            d
        }
        IrType::I64 => {
            let d = builder.new_value();
            builder.emit(Inst::Call { dest: Some(d.clone()), func: "__box_int_for_mixed".into(), args: vec![val], ret_ty: IrType::Ptr });
            d
        }
        _ => val,
    }
}

/// Résout le nom de classe (utilisateur, ou famille builtin
/// `"String"`/`"Array"`/`"Map"`, ou nom monomorphisé d'un générique)
/// PRODUITE par une expression arbitraire utilisée comme RÉCEPTEUR d'un
/// accès de champ ou d'un appel de méthode plus loin dans une chaîne
/// (`expr.champ`, `expr.methode(...)`) — récursif, profondeur ARBITRAIRE
/// (`a.b().c.d().e()...`), pas un nombre de niveaux câblé en dur.
///
/// SEULE source de vérité pour cette résolution — avant ce correctif,
/// `lower.rs`/`typeinfer.rs`/ce fichier contenaient chacun leur PROPRE bloc
/// `match` quasi-identique pour la même question, plafonné à un ou deux
/// niveaux fixes : le cas `Expr::Call { callee: Expr::Field { object, .. }
/// }` (appel de méthode chaîné, `w.getCircle().shapeName()`) ne résolvait
/// `object` que s'il était lui-même `Expr::Ident` — jamais s'il était
/// LUI-MÊME un `Expr::Call`/`Expr::Field` imbriqué — et supposait en plus,
/// à tort, qu'une méthode retournant un pointeur (`IrType::Ptr`) retournait
/// forcément la MÊME classe que son récepteur (confondait par exemple
/// `Wrapper::getCircle(): Circle` avec `Wrapper` lui-même). Voir
/// docs/roadmap.d/langage-chained-call-depth-limit.md — la vraie correction
/// nécessitait `IrModule::method_ret_class` (absent jusqu'ici : une méthode
/// de classe ORDINAIRE, contrairement à une méthode d'INTERFACE ou une
/// fonction libre, n'avait AUCUNE entrée retraçant son type de retour réel
/// en tant que CLASSE, seulement son `IrType` réduit — `Ptr` pour n'importe
/// quel type référence, classes toutes indistinguables).
pub fn resolve_receiver_class(builder: &LowerBuilder, expr: &Expr) -> Option<String> {
    match expr {
        Expr::Ident(name, _)    => builder.var_class.get(name.as_str()).cloned(),
        Expr::SelfExpr(_)       => builder.current_class.clone(),
        Expr::ParentExpr(_)     => builder.parent_class.clone(),
        Expr::Literal(Literal::String(_), _) => Some("String".to_string()),
        // `use Classe(...)` directement en position de récepteur, jamais
        // passé par une variable nommée.
        Expr::New { class, .. } => Some(class.clone()),
        // Accès de champ chaîné (`w.inner`, `w.inner.autre`...) — délègue à
        // `resolve_chained_field_class`, qui résout maintenant SA PROPRE
        // base via cette même fonction (récursion mutuelle, profondeur
        // arbitraire des deux côtés).
        Expr::Field { object, field, .. } => resolve_chained_field_class(builder, object, field),
        // Appel chaîné : `expr.methode(...)` ou `maFonction(...)` utilisé
        // comme récepteur.
        Expr::Call { callee, .. } => match callee.as_ref() {
            // Fonction LIBRE chaînée (`maFonction(...).methode()`) — voir
            // `IrModule::func_ret_class` et
            // docs/roadmap.d/langage-chained-call-on-free-function-result.md.
            Expr::Ident(fn_name, _) => builder.module.func_ret_class.get(fn_name.as_str()).cloned(),
            // Méthode chaînée (`objet.methode1().methode2()`, à n'importe
            // quelle profondeur) : résoudre la classe du récepteur INTERNE
            // récursivement (`object` peut lui-même être n'importe quelle
            // forme reconnue par cette fonction), puis chercher le type de
            // retour RÉEL de cette méthode précise via `method_ret_class` —
            // jamais supposer qu'elle retourne la même classe que son
            // récepteur (l'ancienne heuristique, incorrecte).
            Expr::Field { object, field, .. } => {
                let base_class = resolve_receiver_class(builder, object)?;
                let mangled = format!("{}_{}", base_class, field);
                builder.module.method_ret_class.get(&mangled).cloned()
            }
            _ => None,
        },
        // `HTTPRequest::get/post/put/delete/patch(...).méthode()` chaîné
        // directement, sans jamais passer par une variable `scoped` —
        // `Expr::StaticCall` est un nœud AST complet en lui-même (ses `args`
        // sont intégrés au nœud, jamais enveloppés dans un `Expr::Call`
        // séparé) : ces raccourcis statiques retournent un `HTTPResponse`.
        Expr::StaticCall { class, method, .. }
            if class == "HTTPRequest"
                && matches!(method.as_str(), "get" | "post" | "put" | "delete" | "patch") =>
        {
            Some("HTTPResponse".to_string())
        }
        // Méthode statique utilisateur chaînée (`Classe::fabrique(...).methode()`,
        // `self::`/`parent::` compris) — même table que la méthode d'instance
        // chaînée ci-dessus.
        Expr::StaticCall { class, method, .. } => {
            let owner = match class.as_str() {
                "<self>"   => builder.current_class.clone()?,
                "<parent>" => builder.parent_class.clone()?,
                _          => class.clone(),
            };
            builder.module.method_ret_class.get(&format!("{}_{}", owner, method)).cloned()
        }
        _ => None,
    }
}

/// Résout le nom de classe d'un accès de champ CHAÎNÉ (`w.inner` où `inner`
/// est elle-même une instance de classe/`string`/`array`/`map`) — délègue à
/// `resolve_receiver_class` pour la classe de `object` (récursion mutuelle,
/// profondeur arbitraire), ne garde en propre que la résolution du CHAMP
/// lui-même sur cette classe.
///
/// Nécessite `IrModule.class_field_types` (vrai `Type` AST par champ — pas
/// `IrType`, qui réduit classe/string/array/map à `Ptr`, tous
/// indistinguables) : ne résout donc que les champs de classes
/// UTILISATEUR (voir sa doc) — un champ d'une classe builtin/opaque
/// retourne `None` ici, comme avant ce correctif.
pub fn resolve_chained_field_class(builder: &LowerBuilder, object: &Expr, field: &str) -> Option<String> {
    let base_class = resolve_receiver_class(builder, object)?;
    let field_ty = builder.module.class_field_types.get(&base_class)?
        .iter().find(|(f, _)| f == field)
        .map(|(_, ty)| ty.clone())?;
    match field_ty {
        Type::Named(n)   => Some(n),
        Type::String     => Some("String".to_string()),
        Type::Array(_)   => Some("Array".to_string()),
        Type::Map(_, _)  => Some("Map".to_string()),
        // Champ de type générique (`property box:Box<int>`) — même
        // résolution que pour une variable locale/un paramètre (voir
        // `lower_var`/le paramètre `Type::Generic` dans `functions.rs`) :
        // sans ça, un appel de méthode sur un champ générique
        // (`self.box.get()`, `c.box.get()`) ne trouvait aucune classe et
        // retombait sur un symbole `_method_<nom>` inexistant (confirmé par
        // reproduction — voir docs/roadmap.d/langage-generiques.md).
        Type::Generic { name, args } => Some(crate::core::monomorph::monomorphized_name(&name, &args)),
        _ => None,
    }
}

/// Type "élément" d'un conteneur — le type de `x[i]` sachant que `x` est de
/// type `ty`. Déplie un union (`T[]|null`, `map<K,V>|null`) vers la première
/// variante `Array`/`Map` trouvée. Sœur de `variables::map_value_type`
/// (Map uniquement — utilisée là où seule une vraie map doit être
/// enregistrée, ex. `builder.map_vars`), généralisée ici aux deux formes de
/// conteneur pour servir de brique à `elem_type_after_index` ci-dessous.
fn container_elem_type(ty: &Type) -> Option<&Type> {
    match ty {
        Type::Array(inner)   => Some(inner),
        Type::Map(_, val_ty) => Some(val_ty),
        Type::Union(variants) => variants.iter().find_map(container_elem_type),
        _ => None,
    }
}

fn is_map_shaped(ty: &Type) -> bool {
    match ty {
        Type::Map(..) => true,
        Type::Union(variants) => variants.iter().any(is_map_shaped),
        _ => false,
    }
}

/// Type de `expr[quelque_chose]` — récursif, donc supporte une indexation
/// chaînée de profondeur ARBITRAIRE (`a[0][1]["x"]["y"]`, etc.), pas
/// seulement un niveau. Cas de base (`Ident`/`Field`) : le type élément déjà
/// connu pour une variable/un champ array/map — pas besoin d'une nouvelle
/// table de types, `elem_ast_types`/`class_field_types` existent déjà pour
/// ça. Cas récursif (`Index`) : le type de `(object[index])[?]` s'obtient en
/// épluchant UNE couche de conteneur (`container_elem_type`) sur le type de
/// `object[index]` lui-même déjà résolu par le même appel récursif — c'est
/// cette composition qui rend la profondeur illimitée, sans cas particulier
/// par niveau.
///
/// Cas de base supplémentaire (`Call`/`StaticCall`) : résultat d'appel
/// indexé directement (`getRows()[0]["x"]`, `make()[1][1]`), via le type de
/// retour AST déclaré (`IrModule::call_ret_types`).
pub fn elem_type_after_index(builder: &LowerBuilder, expr: &Expr) -> Option<Type> {
    match expr {
        Expr::Ident(name, _) => builder.elem_ast_types.get(name.as_str()).cloned(),
        Expr::Index { object, .. } => {
            let one_level = elem_type_after_index(builder, object)?;
            container_elem_type(&one_level).cloned()
        }
        Expr::Field { object, field, .. } => {
            let class_name = resolve_receiver_class(builder, object)?;
            let field_ty = builder.module.class_field_types.get(&class_name)?
                .iter().find(|(f, _)| f == field)
                .map(|(_, ty)| ty.clone())?;
            container_elem_type(&field_ty).cloned()
        }
        Expr::Call { .. } | Expr::StaticCall { .. } => {
            container_elem_type(&call_ret_type(builder, expr)?).cloned()
        }
        _ => None,
    }
}

/// Type de retour déclaré d'un appel (fonction libre, méthode chaînée,
/// méthode statique — builtin compris), voir `IrModule::call_ret_types`.
fn call_ret_type(builder: &LowerBuilder, expr: &Expr) -> Option<Type> {
    let key = match expr {
        Expr::Call { callee, .. } => match callee.as_ref() {
            Expr::Ident(name, _) => name.clone(),
            Expr::Field { object, field, .. } => format!("{}_{}", resolve_receiver_class(builder, object)?, field),
            _ => return None,
        },
        Expr::StaticCall { class, method, .. } => {
            let owner = match class.as_str() {
                "<self>"   => builder.current_class.clone()?,
                "<parent>" => builder.parent_class.clone()?,
                _          => class.clone(),
            };
            format!("{}_{}", owner, method)
        }
        _ => return None,
    };
    builder.module.call_ret_types.get(&key).cloned()
}

/// Détermine si `object` (le récepteur d'un `Expr::Index`, `object[index]`)
/// est une `map` plutôt qu'un `array` — nécessaire pour choisir entre
/// `__map_get`/`__map_set` et `__array_get`/`__array_set` (un `map` interprété
/// comme `array` corrompt sa structure interne, crash silencieux au premier
/// accès suivant). Factorisé ici : cette même logique était dupliquée entre
/// la lecture (`Expr::Index` dans `lower.rs`) et l'écriture (`lower_assign`
/// dans `assignments.rs`) — un seul point désormais, pour éviter le même
/// risque de récidive "corrigé sur un chemin, pas l'autre" déjà rencontré
/// pour `expr_ir_type` (voir docs/roadmap.d/qualite-parite-sucre-statique.md).
pub fn is_map_target(builder: &LowerBuilder, object: &Expr) -> bool {
    match object {
        Expr::Ident(name, _) => builder.map_vars.contains(name.as_str()),
        // Résultat d'appel indexé directement (`Convert::strToMap(s, ";", "=")["k"]`,
        // `getConfig()["k"]`) : classe de retour déclarée, builtin compris
        // (voir `method_ret_class`/`func_ret_class`).
        Expr::Call { .. } | Expr::StaticCall { .. } => {
            resolve_receiver_class(builder, object).as_deref() == Some("Map")
        }
        Expr::Field { object: inner, field, .. } => {
            resolve_receiver_class(builder, inner)
                .and_then(|cls| builder.module.class_map_fields.get(&cls).cloned())
                .map(|fields| fields.contains(field.as_str()))
                .unwrap_or(false)
        }
        // `object` lui-même est un `Expr::Index` (`arr[0]` comme récepteur
        // de `arr[0]["name"]`, ou plus profond) — jusqu'ici toujours retombé
        // sur `_ => false` (donc `__array_get`, silencieusement faux sur une
        // map), voir docs/roadmap.d/langage-index-chaine-sur-map.md.
        Expr::Index { object: inner, .. } => {
            elem_type_after_index(builder, inner)
                .as_ref()
                .map(is_map_shaped)
                .unwrap_or(false)
        }
        _ => false,
    }
}

/// Complète les arguments avec les valeurs par défaut si nécessaire
pub fn complete_args_with_defaults(
    builder: &LowerBuilder,
    func_name: &str,
    args: &[Expr],
) -> Vec<Expr> {
    // Récupérer les valeurs par défaut de la fonction
    let default_args = match builder.func_default_args.get(func_name) {
        Some(defaults) => defaults,
        None => return args.to_vec(), // Pas de valeurs par défaut
    };
    
    // Si tous les arguments sont fournis, retourner tel quel
    if args.len() >= default_args.len() {
        return args.to_vec();
    }
    
    // Compléter avec les valeurs par défaut manquantes
    let mut completed = args.to_vec();
    for i in args.len()..default_args.len() {
        if let Some(ref default_expr) = default_args[i] {
            completed.push(default_expr.clone());
        }
    }
    
    completed
}

/// Calcule l'offset en bytes d'un champ dans une classe (8 bytes par champ).
pub fn field_offset(layouts: &HashMap<String, Vec<(String, IrType)>>, class: &str, field: &str) -> i32 {
    if let Some(fields) = layouts.get(class) {
        if let Some(idx) = fields.iter().position(|(f, _)| f == field) {
            return (idx as i32) * 8;
        }
    }
    0
}

/// Retourne le type IR d'un champ depuis le class_layout.
pub fn field_ir_type(layouts: &HashMap<String, Vec<(String, IrType)>>, class: &str, field: &str) -> IrType {
    if let Some(fields) = layouts.get(class) {
        if let Some((_, ty)) = fields.iter().find(|(f, _)| f == field) {
            return ty.clone();
        }
    }
    IrType::Ptr
}

/// Retourne true si l'expression produit un tableau (OcaraArray*).
/// Utilisé dans les templates pour appeler __array_to_str au lieu de ptr_to_str.
pub fn is_array_expr(builder: &LowerBuilder, expr: &Expr) -> bool {
    match expr {
        Expr::Ident(name, _) => builder.elem_types.contains_key(name.as_str()),
        Expr::StaticCall { class, method, .. } => {
            // Résoudre "<parent>" et "<self>" vers les classes appropriées
            let resolved_class = if class == "<parent>" {
                builder.parent_class.as_deref().unwrap_or(class.as_str())
            } else if class == "<self>" {
                builder.current_class.as_deref().unwrap_or(class.as_str())
            } else {
                class.as_str()
            };
            matches!(
                format!("{}_{}", resolved_class, method).as_str(),
                "System_args"
                | "Array_sort"
                | "Array_reverse"
                | "Array_slice"
                | "Map_keys_to_array"
                | "Map_values_to_array"
            )
        }
        _ => false,
    }
}

/// Retourne true si une fonction builtin retourne void (returns: None dans builtins()).
pub fn is_void_builtin(func_name: &str) -> bool {
    builtins().iter().any(|b| b.name == func_name && b.returns.is_none())
}

/// Retourne le nom de la variante typée de `write` selon le type de l'argument.
/// "write"       → string/mixed (pas de conversion, write(ptr) direct)
/// "write_int"   → entiers
/// "write_float" → flottants (prend f64)
/// "write_bool"  → booléens
/// Calcule, à partir du type AST **connu statiquement** de `expr` (un
/// `array<T>`/`map<K,T>`, y compris imbriqué : `array<array<T>>`...), le
/// "type de feuille" concret à transmettre à `JSON_encode`/`YAML_encode` —
/// voir `value_to_json`/`value_to_yaml` (runtime) : `OcaraArray`/`OcaraMap`
/// ne conservent aucune information de type par élément, donc un conteneur
/// **concret** (jamais boxé, contrairement à `mixed`) ne peut pas être
/// interprété sans ambiguïté par ces fonctions (un entier brut `0`/`1` y est
/// indiscernable de `null`/`false` — confirmé par reproduction, voir
/// docs/roadmap.d/langage-mixed-literal-stringification.md) à moins de leur
/// dire, une fois pour toutes à la compilation, quel est ce type de feuille.
///
/// Retourne `0` (inconnu — comportement heuristique historique, inchangé)
/// dès que le type n'est pas résolu statiquement : `expr` n'est pas un
/// identifiant simple référant à un `array`/`map` connu (`elem_ast_types`,
/// alimenté pour `var`/`const`/`scoped`/`consumed`/paramètre — voir
/// `src/lower/stmt.d/statements.d/variables.rs`/`src/lower/builder.d/
/// functions.rs`), ou la structure contient `mixed` à un niveau quelconque.
/// Jamais de faux positif possible : au pire, la précision perdue est
/// exactement celle d'avant ce correctif.
pub fn static_json_leaf_kind(builder: &LowerBuilder, expr: &Expr) -> i64 {
    fn leaf_kind_of(ty: &Type) -> i64 {
        match ty {
            Type::Array(inner) => leaf_kind_of(inner),
            Type::Map(_, inner) => leaf_kind_of(inner),
            Type::Int   => 1,
            Type::Float => 2,
            Type::Bool  => 3,
            _ => 0,
        }
    }
    if let Expr::Ident(name, _) = expr {
        if let Some(ty) = builder.elem_ast_types.get(name.as_str()) {
            return leaf_kind_of(ty);
        }
    }
    0
}

pub fn write_variant(base: &str, ty: &IrType) -> String {
    let suffix = match ty {
        IrType::F64  => "Float",
        IrType::Bool => "Bool",
        IrType::I64  => "Int",
        _            => "",   // Ptr / Mixed → write directement
    };
    format!("{}{}", base, suffix)
}

/// Élément lu (`__array_get`, valeur brute) dans le tableau d'un paramètre
/// variadic, déballé vers son type d'élément déclaré — le site d'appel boxe
/// chaque argument variadic (`float`/`bool` toujours, `int` s'il est ambigu
/// avec un pointeur, voir le packing variadic de `lower.rs`), contrairement à
/// un tableau ordinaire dont les éléments scalaires restent bruts. Sans ce
/// déballage, `not f`/`if f` testaient le pointeur de la cellule boxée. Voir
/// docs/roadmap.d/langage-variadic-bool-not.md.
pub fn unbox_variadic_elem(builder: &mut LowerBuilder, elem_ty: &IrType, raw: Value) -> Value {
    let func = match elem_ty {
        IrType::F64  => "__mixed_to_float",
        IrType::Bool => "__unbox_bool",
        IrType::I64  => "__mixed_to_int",
        _ => return raw,
    };
    let dest = builder.new_value();
    builder.emit(Inst::Call { dest: Some(dest.clone()), func: func.into(), args: vec![raw], ret_ty: elem_ty.clone() });
    dest
}
