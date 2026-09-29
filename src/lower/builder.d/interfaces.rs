/// Dispatch dynamique réel pour les interfaces (voir
/// docs/roadmap.d/langage-interfaces.md).
///
/// Un appel `d.method(...)` sur une variable typée par une INTERFACE
/// (`var d:Drawable`) est mangled comme n'importe quel appel de méthode
/// (`Drawable_method`, voir `src/lower/expr.d/lower.rs`) — jusqu'ici, cette
/// fonction n'existait tout simplement jamais (seules les classes concrètes
/// émettent du code), le dispatch était donc entièrement cassé. Ce module
/// génère RÉELLEMENT `Interface_method` : une fonction qui lit l'identité de
/// classe de `self` (`IrModule::class_ids`, écrite dans le header de chaque
/// instance par `__alloc_class_obj`, voir runtime/src/lib.rs) et appelle la
/// bonne implémentation concrète (`Classe_method`) selon un branchement sur
/// cette identité — un dispatch manuel mais réel, réutilisant telle quelle
/// toute l'infrastructure d'appel mangled déjà en place.
use crate::ir::func::{IrFunction, IrParam};
use crate::ir::inst::Inst;
use crate::ir::module::IrModule;
use crate::ir::types::IrType;
use crate::parsing::ast::{ClassDecl, InterfaceMethod, Program};

/// Génère, pour chaque interface implémentée par au moins une classe, un
/// dispatcher réel pour chacune de ses méthodes.
///
/// Portée volontairement alignée sur celle du diagnostic E09 (`src/main.rs`) :
/// seules les classes qui déclarent `implements Interface` DIRECTEMENT dans
/// leur propre liste sont candidates au dispatch — une sous-classe qui n'a
/// pas elle-même redéclaré `implements` n'est PAS candidate ici, même si
/// `SymbolTable::class_matches` (utilisée par `types_compat` pour
/// l'affectation) la considère transitivement compatible via `extends` :
/// cette asymétrie est voulue, E09 n'a validé la signature QUE pour les
/// classes qui l'implémentent explicitement, on ne peut pas générer un appel
/// vers une méthode dont la signature n'a jamais été vérifiée contre
/// l'interface (voir `SymbolTable::class_matches` pour le détail).
pub fn generate_interface_dispatchers(module: &mut IrModule, program: &Program) {
    for iface in &program.interfaces {
        let implementers: Vec<&ClassDecl> = program.classes.iter()
            .filter(|c| c.implements.iter().any(|i| i == &iface.name))
            .collect();
        // Interface jamais implémentée : rien à générer (jamais atteignable
        // par un appel qui aurait passé la sema de toute façon).
        if implementers.is_empty() {
            continue;
        }
        for method in &iface.methods {
            // Une méthode STATIQUE d'interface (`is_static`, voir
            // `InterfaceMethod::is_static` — nécessaire depuis `wiring`,
            // docs/roadmap.d/langage-interface-wiring.md, qui a rendu cette
            // grammaire atteignable pour la première fois : `static` était
            // jusqu'ici rejeté par le parser dans un corps d'interface) n'a
            // AUCUN `self` sur lequel dispatcher à l'exécution — un appel
            // statique (`Classe::method()`) résout DÉJÀ sa cible à la
            // compilation (nom explicite, ou substitution `wiring`/alias,
            // voir `core::interface_wiring`), jamais via l'identité de
            // classe d'une instance. Générer quand même un dispatcher ici
            // (comme pour une méthode d'instance) produisait une fonction
            // `Interface_method(self)` qui appelait `Classe_method(self)`
            // avec un argument `self` que la VRAIE méthode statique
            // (déclarée sans paramètre `self`) n'attend jamais : Cranelift
            // rejetait ce module entier à la vérification («mismatched
            // argument count», confirmé par reproduction) — alors même que
            // ce dispatcher mort n'est jamais appelé par aucun code
            // utilisateur, Cranelift vérifie TOUTES les fonctions du module.
            if method.is_static {
                continue;
            }
            generate_one_dispatcher(module, &iface.name, method, &implementers);
        }
    }
}

fn generate_one_dispatcher(
    module: &mut IrModule,
    iface_name: &str,
    method: &InterfaceMethod,
    implementers: &[&ClassDecl],
) {
    let ret_ty = IrType::from_ast(&method.ret_ty);
    let mut f = IrFunction::new(format!("{}_{}", iface_name, method.name), vec![], ret_ty.clone());

    // `self` + les paramètres de la méthode — leurs slots sont réservés ICI
    // (avant toute autre émission) pour correspondre aux block-params
    // Cranelift de la fonction (voir `emit_function`, `ir_func.params[i].slot`
    // devient directement l'index de la Variable associée au i-ème
    // paramètre entrant).
    let self_val = f.new_value();
    let mut params = vec![IrParam { name: "self".into(), ty: IrType::Ptr, slot: self_val.clone() }];
    let mut arg_vals = Vec::new();
    for p in &method.params {
        let v = f.new_value();
        params.push(IrParam { name: p.name.clone(), ty: IrType::from_ast(&p.ty), slot: v.clone() });
        arg_vals.push(v);
    }
    f.params = params;

    // Identité de classe de `self` — offset -16 : le `class_id` est stocké
    // JUSTE AVANT le tag `TAG_OBJECT` (lui à `self - 8`), voir
    // `__alloc_class_obj` dans runtime/src/lib.rs. Un simple `GetField` à
    // offset négatif suffit (Cranelift accepte un déplacement signé), pas
    // besoin d'une fonction runtime dédiée.
    let class_id_val = f.new_value();
    f.emit(Inst::GetField {
        dest:   class_id_val.clone(),
        obj:    self_val.clone(),
        field:  "__class_id".into(),
        ty:     IrType::I64,
        offset: -16,
    });

    // Chaîne de branchement : une comparaison par implémenteur, dans l'ordre
    // de déclaration (déterministe ; l'ORDRE n'a aucun impact sur le
    // résultat, une seule branche peut matcher puisque `class_id` est unique
    // par classe).
    for class in implementers {
        let class_id = module.class_ids.get(&class.name).copied().unwrap_or(0);
        let cid_const = f.new_value();
        f.emit(Inst::ConstInt { dest: cid_const.clone(), value: class_id });
        let cmp = f.new_value();
        f.emit(Inst::CmpEq { dest: cmp.clone(), lhs: class_id_val.clone(), rhs: cid_const.clone(), ty: IrType::I64 });

        let call_bb = f.new_block();
        let next_bb = f.new_block();
        f.emit(Inst::Branch { cond: cmp, then_bb: call_bb.clone(), else_bb: next_bb.clone() });

        f.switch_to(&call_bb);
        let callee = format!("{}_{}", class.name, method.name);
        let mut call_args = vec![self_val.clone()];
        call_args.extend(arg_vals.clone());
        if ret_ty == IrType::Void {
            f.emit(Inst::Call { dest: None, func: callee, args: call_args, ret_ty: IrType::Void });
            f.emit(Inst::Return { value: None });
        } else {
            let r = f.new_value();
            f.emit(Inst::Call { dest: Some(r.clone()), func: callee, args: call_args, ret_ty: ret_ty.clone() });
            f.emit(Inst::Return { value: Some(r) });
        }

        f.switch_to(&next_bb);
    }

    // Aucun implémenteur ne correspond : ne devrait jamais arriver pour un
    // programme qui a passé la sema (`d:Drawable` ne peut recevoir qu'une
    // instance d'une classe implémentant directement `Drawable`, voir
    // `types_compat`/`SymbolTable::class_matches`) — retour défensif d'une
    // valeur neutre plutôt qu'un comportement indéfini.
    if ret_ty == IrType::Void {
        f.emit(Inst::Return { value: None });
    } else {
        let zero = f.new_value();
        f.emit(Inst::ConstInt { dest: zero.clone(), value: 0 });
        f.emit(Inst::Return { value: Some(zero) });
    }

    module.add_function(f);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower::builder::program::lower_program;
    use crate::parsing::lexer::Lexer;
    use crate::parsing::parser::Parser;

    fn lower(src: &str) -> IrModule {
        let tokens = Lexer::new(src).tokenize().expect("lex");
        let program = Parser::new(tokens).parse_program().expect("parse");
        lower_program(&program, "<test>")
    }

    /// Régression — voir docs/roadmap.d/langage-interface-wiring.md et
    /// examples/tests/60_interface_wiringTest.oc : une méthode d'interface
    /// STATIQUE (`is_static`, atteignable seulement depuis que `wiring` a
    /// nécessité d'étendre la grammaire d'un corps d'interface pour accepter
    /// `static`) ne doit JAMAIS recevoir de dispatcher `self`-based —
    /// `generate_one_dispatcher` suppose TOUJOURS un `self` (voir son
    /// premier paramètre codé en dur), ce qui produisait un module Cranelift
    /// qui ne passait plus la vérification (`Interface_method(self)` appelant
    /// `Classe_method(self)` avec un argument que la vraie méthode statique,
    /// déclarée SANS paramètre `self`, n'attend jamais) — confirmé par
    /// reproduction avant correctif : « mismatched argument count: got 1,
    /// expected 0 ». Aucun dispatcher `Repo_create` ne doit donc être émis.
    #[test]
    fn no_dispatcher_is_generated_for_a_static_interface_method() {
        let module = lower(
            "interface Repo {\n\
                 method save(): void\n\
                 static method create(): Repo\n\
                 wiring ConcreteRepo\n\
             }\n\
             class ConcreteRepo implements Repo {\n\
                 public static method create(): Repo {\n\
                     return use ConcreteRepo()\n\
                 }\n\
                 public method save(): void {\n\
                 }\n\
             }\n",
        );

        assert!(
            module.functions.iter().all(|f| f.name != "Repo_create"),
            "no dispatcher should ever be generated for a static interface method"
        );
        // La VRAIE méthode statique, elle, ne doit prendre AUCUN paramètre
        // (ni `self`, ni quoi que ce soit d'autre — `create()` n'en déclare
        // aucun) : c'est justement ce désaccord d'arité (dispatcher à 1
        // param appelant cette fonction à 0 param) qui cassait la
        // vérification Cranelift avant le correctif.
        let create_fn = module.functions.iter().find(|f| f.name == "ConcreteRepo_create")
            .expect("the real static method must still be lowered normally");
        assert_eq!(create_fn.params.len(), 0, "a static method takes no implicit 'self'");
    }

    /// Non-régression : une méthode d'interface D'INSTANCE, elle, doit
    /// toujours recevoir son dispatcher `self`-based réel (mécanisme
    /// préexistant à ce ticket, voir la doc de module ci-dessus) — `wiring`
    /// ne doit rien changer à ce chemin pour les méthodes non-statiques.
    #[test]
    fn dispatcher_is_still_generated_for_an_instance_interface_method() {
        let module = lower(
            "interface Repo {\n\
                 method save(): void\n\
                 static method create(): Repo\n\
                 wiring ConcreteRepo\n\
             }\n\
             class ConcreteRepo implements Repo {\n\
                 public static method create(): Repo {\n\
                     return use ConcreteRepo()\n\
                 }\n\
                 public method save(): void {\n\
                 }\n\
             }\n",
        );

        let save_dispatcher = module.functions.iter().find(|f| f.name == "Repo_save")
            .expect("an instance interface method must still get a real dispatcher");
        assert_eq!(save_dispatcher.params.len(), 1, "the dispatcher's only param is 'self'");
    }

    /// Cherche le premier `Inst::Call` vers `func_name` dans TOUS les blocs
    /// de `func` — utilitaire pour les tests ci-dessous.
    fn find_call<'a>(func: &'a IrFunction, func_name: &str) -> &'a Inst {
        func.blocks.iter()
            .flat_map(|b| &b.insts)
            .find(|i| matches!(i, Inst::Call { func: f, .. } if f == func_name))
            .unwrap_or_else(|| panic!("no call to '{}' found in function '{}'", func_name, func.name))
    }

    /// Régression — voir docs/roadmap.d/langage-interface-instance-dispatch-segfault.md :
    /// SIGSEGV confirmé (`var s:Shape = c; s.area()`, `c:Circle implements Shape`,
    /// aucun `wiring` en jeu — polymorphisme d'interface ORDINAIRE). Root
    /// cause confirmée par backtrace gdb (crash dans `ocara_runtime::val_to_string`,
    /// PAS dans le dispatcher lui-même) : `fn_ret_types` (`src/lower/builder.d/program.rs`)
    /// n'était peuplée que depuis `program.classes` ("Classe_méthode"),
    /// jamais depuis `program.interfaces` ("Interface_méthode") — le SITE
    /// D'APPEL de `s.area()` (mangled "Shape_area") retombait donc sur le
    /// défaut `IrType::Ptr` pour le `ret_ty` de son `Inst::Call`, alors que
    /// le dispatcher RÉELLEMENT généré (`generate_one_dispatcher` ci-dessus)
    /// respecte le VRAI type déclaré (`F64` pour `area(): float`) —
    /// désaccord d'ABI Cranelift au retour d'appel (convention entière vs
    /// flottante), qui produisait une valeur garbage silencieusement
    /// réinterprétée par l'appelant. Ce test vérifie directement le point
    /// de désaccord : le `ret_ty` du SITE D'APPEL doit correspondre au
    /// `ret_ty` RÉEL du dispatcher appelé.
    #[test]
    fn call_site_ret_ty_matches_the_real_interface_dispatcher_ret_ty() {
        let module = lower(
            "interface Shape {\n\
                 method area(): float\n\
             }\n\
             class Circle implements Shape {\n\
                 public property r:float\n\
                 init(r:float) { self.r = r }\n\
                 public method area(): float { return 3.14 * self.r * self.r }\n\
             }\n\
             function main(): int {\n\
                 const c:Circle = use Circle(2.0)\n\
                 var s:Shape = c\n\
                 var a:float = s.area()\n\
                 return 0\n\
             }\n",
        );

        let dispatcher = module.functions.iter().find(|f| f.name == "Shape_area")
            .expect("the real interface dispatcher must be generated");
        assert_eq!(dispatcher.ret_ty, IrType::F64, "area(): float must lower to a real F64 dispatcher");

        let main_fn = module.functions.iter().find(|f| f.name == "main").expect("main must exist");
        let Inst::Call { ret_ty, .. } = find_call(main_fn, "Shape_area") else { unreachable!() };
        assert_eq!(*ret_ty, IrType::F64, "the call site must agree with the dispatcher's real return type — this exact mismatch (Ptr vs F64) caused the SIGSEGV");
    }

    /// Plusieurs classes implémentant la MÊME interface : le dispatcher
    /// doit contenir une branche par implémenteur, chacune appelant la
    /// bonne méthode concrète — non-régression du mécanisme de branchement
    /// par `__class_id` déjà en place (voir `generate_one_dispatcher`),
    /// jamais remis en cause par ce correctif (qui ne touche que la
    /// PROPAGATION du type de retour au site d'appel, pas le dispatch
    /// lui-même).
    #[test]
    fn dispatcher_branches_cover_every_implementer() {
        let module = lower(
            "interface Shape {\n\
                 method area(): float\n\
             }\n\
             class Circle implements Shape {\n\
                 public method area(): float { return 1.0 }\n\
             }\n\
             class Square implements Shape {\n\
                 public method area(): float { return 2.0 }\n\
             }\n\
             class Triangle implements Shape {\n\
                 public method area(): float { return 3.0 }\n\
             }\n",
        );

        let dispatcher = module.functions.iter().find(|f| f.name == "Shape_area")
            .expect("dispatcher must be generated");
        let called: Vec<&str> = dispatcher.blocks.iter()
            .flat_map(|b| &b.insts)
            .filter_map(|i| match i { Inst::Call { func, .. } => Some(func.as_str()), _ => None })
            .collect();
        for expected in ["Circle_area", "Square_area", "Triangle_area"] {
            assert!(called.contains(&expected), "dispatcher must call '{}', found calls: {:?}", expected, called);
        }
    }

    /// Une classe qui `implements` PLUSIEURS interfaces : chaque interface
    /// doit obtenir son propre dispatcher, tous deux ciblant la même classe
    /// concrète pour leurs méthodes respectives.
    #[test]
    fn class_implementing_multiple_interfaces_gets_a_dispatcher_for_each() {
        let module = lower(
            "interface Shape {\n\
                 method area(): float\n\
             }\n\
             interface Drawable {\n\
                 method draw(): string\n\
             }\n\
             class Circle implements Shape, Drawable {\n\
                 public method area(): float { return 1.0 }\n\
                 public method draw(): string { return \"O\" }\n\
             }\n",
        );

        let shape_dispatcher = module.functions.iter().find(|f| f.name == "Shape_area")
            .expect("Shape's dispatcher must be generated");
        assert_eq!(shape_dispatcher.ret_ty, IrType::F64);
        let drawable_dispatcher = module.functions.iter().find(|f| f.name == "Drawable_draw")
            .expect("Drawable's dispatcher must be generated");
        assert_eq!(drawable_dispatcher.ret_ty, IrType::Ptr, "string lowers to Ptr");

        for (dispatcher, callee) in [(shape_dispatcher, "Circle_area"), (drawable_dispatcher, "Circle_draw")] {
            let calls_circle = dispatcher.blocks.iter().flat_map(|b| &b.insts)
                .any(|i| matches!(i, Inst::Call { func, .. } if func == callee));
            assert!(calls_circle, "{}'s dispatcher must call '{}'", dispatcher.name, callee);
        }
    }

    /// Une méthode d'interface avec un paramètre `mixed` : le boxing d'un
    /// argument brut (float/int/bool) au site d'appel dépend de
    /// `module.method_param_types` (voir `param_type_for_call_arg`,
    /// `src/lower/expr.d/lower.rs`) — jamais peuplée pour une interface
    /// avant ce correctif (seulement `program.classes`), ce qui aurait
    /// laissé un argument scalaire NON boxé passer là où le dispatcher
    /// (puis la méthode concrète) attend une valeur `mixed` déjà boxée —
    /// même famille de bug que le boxing `mixed` déjà corrigé ailleurs
    /// dans ce compilateur (voir docs/roadmap.d/memoire-boxing-durcissement.md).
    #[test]
    fn interface_method_param_types_are_known_for_mixed_boxing() {
        let module = lower(
            "interface Sink {\n\
                 method accept(v:mixed): void\n\
             }\n\
             class Logger implements Sink {\n\
                 public method accept(v:mixed): void {\n\
                 }\n\
             }\n\
             function main(): int {\n\
                 const l:Logger = use Logger()\n\
                 var s:Sink = l\n\
                 s.accept(42)\n\
                 return 0\n\
             }\n",
        );

        assert_eq!(
            module.method_param_types.get("Sink_accept").map(Vec::as_slice),
            Some([IrType::Ptr].as_slice()),
            "an interface method's own instance param types must be known (mixed lowers to Ptr), \
             not just its implementers' — otherwise mixed-boxing at the call site has nothing to check against"
        );
    }
}
