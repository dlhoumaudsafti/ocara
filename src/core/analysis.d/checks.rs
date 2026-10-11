//! Vérifications de structure après la fusion des imports : parents
//! `extends` (E27), interfaces implémentées (E09) par les classes et les
//! `generic`, cibles des `wiring` (E39, E40, E42).

use std::path::Path;

use crate::core::diagnostics::Diagnostic;
use crate::parsing::ast::Program;
use crate::parsing::token::Span;
use crate::sema::symbols::{FuncSig, SymbolTable};
use crate::sema::typecheck::{type_name, types_compat};

pub fn check_extends(program: &Program, symbols: &SymbolTable, input: &Path) -> Result<(), Diagnostic> {
    for class_decl in &program.classes {
        if let Some(parent) = &class_decl.extends {
            if symbols.lookup_class(parent).is_none() {
                return Err(Diagnostic::at(input, &class_decl.span,
                    format!("class '{}' extends unknown class '{}'", class_decl.name, parent)));
            }
        }
    }
    for generic_decl in &program.generics {
        if let Some(parent) = &generic_decl.extends {
            if symbols.lookup_class(parent).is_none() && symbols.lookup_generic(parent).is_none() {
                return Err(Diagnostic::at(input, &generic_decl.span,
                    format!("generic '{}' extends unknown class/generic '{}'", generic_decl.name, parent)));
            }
        }
    }
    Ok(())
}

pub fn check_implements(program: &Program, symbols: &SymbolTable, input: &Path) -> Result<(), Diagnostic> {
    for class_decl in &program.classes {
        let lookup = |method: &str| symbols.lookup_method_in_chain(&class_decl.name, method);
        check_contracts("class", &class_decl.name, &class_decl.implements, &class_decl.span, &lookup, symbols, input)?;
    }
    // Un `generic` n'est monomorphisé en classe qu'après la sema : vérifié
    // ici sur sa déclaration (docs/roadmap.d/langage-generiques.md).
    for generic_decl in &program.generics {
        let info = symbols.lookup_generic(&generic_decl.name).expect("generic enregistré");
        let lookup = |method: &str| info.methods.get(method);
        check_contracts("generic", &generic_decl.name, &generic_decl.implements, &generic_decl.span, &lookup, symbols, input)?;
    }
    Ok(())
}

/// Chaque méthode des interfaces `implements` doit exister avec la même
/// staticité, le même `async`, la même arité et des types compatibles.
fn check_contracts<'a>(
    kind: &str,
    name: &str,
    implements: &[String],
    span: &Span,
    lookup: &dyn Fn(&str) -> Option<&'a FuncSig>,
    symbols: &SymbolTable,
    input: &Path,
) -> Result<(), Diagnostic> {
    let err = |msg: String| Err(Diagnostic::at(input, span, msg));
    for iface_name in implements {
        let Some(iface_info) = symbols.lookup_interface(iface_name) else {
            return err(format!("interface '{}' not found", iface_name));
        };
        for (method_name, iface_sig) in &iface_info.methods {
            let Some(sig) = lookup(method_name) else {
                return err(format!("{} '{}' does not implement method '{}' from interface '{}'", kind, name, method_name, iface_name));
            };
            let mismatch = |detail: String| err(format!("method '{}' of {} '{}' does not match interface '{}': {}", method_name, kind, name, iface_name, detail));
            if sig.is_static != iface_sig.is_static {
                return mismatch(format!("expected a {} method, found a {} method",
                    if iface_sig.is_static { "static" } else { "instance" },
                    if sig.is_static { "static" } else { "instance" }));
            }
            if sig.is_async != iface_sig.is_async {
                return mismatch(format!("expected an '{}' method, found an '{}' method",
                    if iface_sig.is_async { "async" } else { "non-async" },
                    if sig.is_async { "async" } else { "non-async" }));
            }
            if sig.params.len() != iface_sig.params.len() {
                return mismatch(format!("expected {} parameter(s), found {}", iface_sig.params.len(), sig.params.len()));
            }
            for (i, (_, iface_param_ty)) in iface_sig.params.iter().enumerate() {
                let (_, param_ty) = &sig.params[i];
                if !types_compat(param_ty, iface_param_ty, symbols) {
                    return mismatch(format!("parameter {} expected type '{}', found '{}'", i + 1, type_name(iface_param_ty), type_name(param_ty)));
                }
            }
            if !types_compat(&sig.ret_ty, &iface_sig.ret_ty, symbols) {
                return mismatch(format!("expected return type '{}', found '{}'", type_name(&iface_sig.ret_ty), type_name(&sig.ret_ty)));
            }
        }
    }
    Ok(())
}

/// Ce que `check_implements` ne couvre pas pour un `wiring` (voir
/// docs/roadmap.d/langage-interface-wiring.md) : nom simple en double (E42),
/// cible introuvable ou générique (E39), cible qui n'`implements` pas
/// l'interface (E40).
pub fn check_wirings(program: &Program, symbols: &SymbolTable, input: &Path) -> Result<(), Diagnostic> {
    for iface_decl in &program.interfaces {
        let w = &iface_decl.wirings;
        for i in 0..w.len() {
            for j in (i + 1)..w.len() {
                if w[i].simple_name() == w[j].simple_name() {
                    return Err(Diagnostic::at(input, &iface_decl.span, format!(
                        "interface '{}' declares two 'wiring' targets with the same simple name '{}' ({}:{} and {}:{}) — alias resolution could not tell them apart",
                        iface_decl.name, w[i].simple_name(), w[i].span.line, w[i].span.col, w[j].span.line, w[j].span.col)));
                }
            }
        }

        for wiring in w {
            let target_name = wiring.simple_name();
            let Some(target_class) = symbols.lookup_class(target_name) else {
                let msg = if symbols.lookup_generic(target_name).is_some() {
                    format!("interface '{}': 'wiring {}' targets a generic, not a concrete class — wiring a bare generic is ambiguous (which instantiation?)", iface_decl.name, target_name)
                } else {
                    format!("interface '{}': 'wiring {}' target class not found", iface_decl.name, target_name)
                };
                return Err(Diagnostic::at(input, &wiring.span, msg));
            };
            if !target_class.implements.iter().any(|i| i == &iface_decl.name) {
                return Err(Diagnostic::at(input, &wiring.span, format!(
                    "interface '{}': 'wiring {}' target class '{}' does not 'implements {}'",
                    iface_decl.name, target_name, target_name, iface_decl.name)));
            }
        }
    }
    Ok(())
}
