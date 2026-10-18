/// `wiring <chemin>` dans une classe de test unitaire : remplace, pour toute
/// la compilation du test, la cible des interfaces que la classe visée
/// implémente (voir docs/roadmap.d/langage-wiring-substitution-tests.md).
///
/// ```ocara
/// class CarDetailsServiceTest extends UnitTest {
///     wiring tests.support.FakeCarRepository
/// }
/// ```
///
/// La classe visée devient le PREMIER `wiring` de chacune de ses
/// interfaces : un nom nu (`CarContract::find(…)`, `use CarContract()`),
/// partout dans le programme, la désigne (`core::interface_wiring`). Un
/// alias d'import qui choisit explicitement une implémentation reste
/// inchangé. Comme dans une interface, le `wiring` vaut import implicite.
///
/// Réservé aux tests : la classe doit `extends UnitTest`, dans le fichier
/// compilé (`*Test.oc`, celui que génère `ocaraunit` compris) ; un fichier
/// importé ne peut pas en déclarer. Le code de production ne peut donc
/// jamais s'en servir.
use std::collections::HashMap;
use std::path::Path;
use crate::core::diagnostics::Diagnostic;
use crate::parsing::ast::*;

/// Fichier d'entrée : chaque classe qui déclare un `wiring` est une classe
/// de test d'un fichier `*Test.oc`.
pub fn check_entry(program: &Program, input: &Path) -> Result<(), Diagnostic> {
    for class in program.classes.iter().filter(|c| !c.wirings.is_empty()) {
        let span = &class.wirings[0].span;
        if class.extends.as_deref() != Some("UnitTest") {
            return Err(Diagnostic::at(input, span, format!(
                "class '{}': 'wiring' is only allowed in a unit test class (extends UnitTest)", class.name)));
        }
        if !is_test_file(input) {
            return Err(Diagnostic::at(input, span, format!(
                "class '{}': 'wiring' is only allowed in a unit test file (*Test.oc)", class.name)));
        }
    }
    Ok(())
}

/// Fichier importé : un `wiring` de classe n'y est jamais permis (seul le
/// fichier de test compilé peut remplacer une implémentation).
pub fn check_imported(program: &Program, file: &Path) -> Result<(), Diagnostic> {
    let Some(class) = program.classes.iter().find(|c| !c.wirings.is_empty()) else { return Ok(()) };
    Err(Diagnostic::at(file, &class.wirings[0].span, format!(
        "class '{}': 'wiring' in a class is only allowed in the unit test file being compiled, not in an imported file", class.name)))
}

/// Classes visées par les `wiring` de classe, à importer implicitement.
pub fn targets(program: &Program) -> Vec<WiringDecl> {
    program.classes.iter().flat_map(|c| c.wirings.iter().cloned()).collect()
}

/// Après le chargement des imports : chaque classe visée devient le premier
/// `wiring` des interfaces qu'elle implémente.
pub fn apply(program: &mut Program, all_interfaces: &mut HashMap<String, InterfaceDecl>, input: &Path) -> Result<(), Diagnostic> {
    let mut replaced: HashMap<String, String> = HashMap::new();
    for wiring in targets(program) {
        let target = wiring.simple_name().to_string();
        let Some(class) = program.classes.iter().find(|c| c.name == target) else {
            return Err(Diagnostic::at(input, &wiring.span, format!("'wiring {}': target class not found", target)));
        };
        if class.implements.is_empty() {
            return Err(Diagnostic::at(input, &wiring.span, format!(
                "'wiring {}': class '{}' implements no interface — nothing to replace", target, target)));
        }
        for iface in class.implements.clone() {
            if let Some(previous) = replaced.insert(iface.clone(), target.clone()) {
                if previous != target {
                    return Err(Diagnostic::at(input, &wiring.span, format!(
                        "interface '{}' is replaced twice ('wiring {}' and 'wiring {}')", iface, previous, target)));
                }
            }
            if let Some(decl) = program.interfaces.iter_mut().find(|i| i.name == iface) {
                put_first(&mut decl.wirings, &wiring);
            }
            if let Some(decl) = all_interfaces.get_mut(&iface) {
                put_first(&mut decl.wirings, &wiring);
            }
        }
    }
    Ok(())
}

fn put_first(wirings: &mut Vec<WiringDecl>, wiring: &WiringDecl) {
    wirings.retain(|w| w.simple_name() != wiring.simple_name());
    wirings.insert(0, wiring.clone());
}

fn is_test_file(input: &Path) -> bool {
    input.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with("Test.oc"))
}
