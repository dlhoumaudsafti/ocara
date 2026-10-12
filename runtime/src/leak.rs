//! Bilan des fuites (`ocara --leak-check`) : le compilateur appelle
//! `__leak_check_init(noms)` en entrée de programme (`noms` : chaîne
//! statique `id=Classe;…`). Dès lors, chaque allocation et chaque libération
//! de valeur (chaînes, tableaux, maps, objets par classe, closures…) est
//! comptée ; à la sortie, ce qui n'a pas été libéré est rapporté, dans le
//! fichier `OCARA_LEAK_OUT` (sinon sur stderr), une ligne par genre :
//! `restants<TAB>alloués<TAB>libérés<TAB>genre`, restants décroissants.
//! Désactivé (cas normal), le coût est un test de booléen par allocation.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Once};

use crate::typecheck::{TAG_ARRAY, TAG_CELL, TAG_ENV, TAG_EXCEPTION, TAG_FUNCTION, TAG_GEN, TAG_HANDLE, TAG_MAP, TAG_STRING_OWNED};

static ENABLED: AtomicBool = AtomicBool::new(false);
static INIT: Once = Once::new();
static COUNTS: Mutex<Option<HashMap<Kind, (u64, u64)>>> = Mutex::new(None);
static NAMES: Mutex<Option<HashMap<i64, String>>> = Mutex::new(None);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Kind {
    Tag(i64),
    Class(i64),
    Boxed,
}

#[unsafe(no_mangle)]
pub extern "C" fn __leak_check_init(names: i64) {
    INIT.call_once(|| {
        let table = if names == 0 { "" } else { unsafe { crate::ptr_to_str(names) } };
        let names = table.split(';')
            .filter_map(|e| e.split_once('='))
            .filter_map(|(id, name)| Some((id.parse().ok()?, name.to_string())))
            .collect();
        *NAMES.lock().unwrap_or_else(|e| e.into_inner()) = Some(names);
        ENABLED.store(true, Ordering::Release);
        unsafe { libc::atexit(report) };
    });
}

#[inline]
pub(crate) fn on_alloc(kind: Kind) {
    if ENABLED.load(Ordering::Relaxed) {
        count(kind, true);
    }
}

#[inline]
pub(crate) fn on_free(kind: Kind) {
    if ENABLED.load(Ordering::Relaxed) {
        count(kind, false);
    }
}

fn count(kind: Kind, alloc: bool) {
    let mut counts = COUNTS.lock().unwrap_or_else(|e| e.into_inner());
    let entry = counts.get_or_insert_with(HashMap::new).entry(kind).or_default();
    if alloc { entry.0 += 1 } else { entry.1 += 1 }
}

fn label(kind: Kind, names: &HashMap<i64, String>) -> String {
    match kind {
        Kind::Class(id) => names.get(&id).cloned().unwrap_or_else(|| format!("objet (classe #{})", id)),
        Kind::Boxed => "valeur boxée (mixed)".into(),
        Kind::Tag(t) if t == TAG_STRING_OWNED => "string".into(),
        Kind::Tag(t) if t == TAG_ARRAY => "array".into(),
        Kind::Tag(t) if t == TAG_MAP => "map".into(),
        Kind::Tag(t) if t == TAG_FUNCTION => "fonction (closure)".into(),
        Kind::Tag(t) if t == TAG_ENV => "environnement de closure".into(),
        Kind::Tag(t) if t == TAG_CELL => "variable capturée".into(),
        Kind::Tag(t) if t == TAG_GEN => "générateur".into(),
        Kind::Tag(t) if t == TAG_EXCEPTION => "exception".into(),
        Kind::Tag(t) if t == TAG_HANDLE => "ressource native".into(),
        Kind::Tag(t) => format!("bloc (tag {})", t),
    }
}

extern "C" fn report() {
    // Valeurs en attente dans le tampon du collecteur de cycles : libérées
    // par sa prochaine passe, ce ne sont pas des fuites.
    crate::rc::collect_cycles();
    ENABLED.store(false, Ordering::Release);
    let counts = COUNTS.lock().unwrap_or_else(|e| e.into_inner()).take().unwrap_or_default();
    let names = NAMES.lock().unwrap_or_else(|e| e.into_inner()).take().unwrap_or_default();
    let mut rows: Vec<(u64, u64, u64, String)> = counts.into_iter()
        .map(|(kind, (a, f))| (a.saturating_sub(f), a, f, label(kind, &names)))
        .filter(|r| r.0 > 0)
        .collect();
    rows.sort_by(|a, b| b.0.cmp(&a.0).then(a.3.cmp(&b.3)));
    let text: String = rows.iter().map(|(live, a, f, l)| format!("{}\t{}\t{}\t{}\n", live, a, f, l)).collect();
    match std::env::var("OCARA_LEAK_OUT") {
        Ok(path) if !path.is_empty() => {
            let _ = std::fs::write(path, text);
        }
        _ if rows.is_empty() => eprintln!("── fuites : aucune — toute valeur allouée a été libérée ──"),
        _ => {
            let total: u64 = rows.iter().map(|r| r.0).sum();
            eprint!("── fuites : {} valeur(s) non libérée(s) à la sortie ──\n", total);
            for (live, a, f, l) in &rows {
                eprintln!("  {:>6} × {:<28} (allouées {}, libérées {})", live, l, a, f);
            }
        }
    }
}
