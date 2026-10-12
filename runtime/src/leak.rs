//! Bilan des fuites (`ocara --leak-check`) : le compilateur appelle
//! `__leak_check_init(noms)` en entrée de programme (`noms` : chaîne
//! statique `id=Classe;…`) et encadre chaque fonction de
//! `__leak_enter(lieu)`/`__leak_exit(lieu)` (`lieu` : `nom␟fichier␟ligne`).
//! Chaque valeur allouée (chaînes, tableaux, maps, objets par classe,
//! closures…) est suivie avec la fonction qui l'a allouée ; à la sortie, ce
//! qui n'a pas été libéré est rapporté par genre puis par lieu d'allocation,
//! dans le fichier `OCARA_LEAK_OUT` (sinon sur stderr) :
//! `K<TAB>genre<TAB>restants<TAB>alloués<TAB>libérés` puis, pour ce genre,
//! `S<TAB>genre<TAB>restants<TAB>fonction<TAB>fichier<TAB>ligne`.
//! Désactivé (cas normal), le coût est un test de booléen par allocation.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Once};

use crate::typecheck::{TAG_ARRAY, TAG_CELL, TAG_ENV, TAG_EXCEPTION, TAG_FUNCTION, TAG_GEN, TAG_HANDLE, TAG_MAP, TAG_STRING_OWNED};

static ENABLED: AtomicBool = AtomicBool::new(false);
static INIT: Once = Once::new();
static STATE: Mutex<Option<State>> = Mutex::new(None);

thread_local! {
    /// Fonctions en cours (lieux), la plus récente en dernier.
    static STACK: RefCell<Vec<i64>> = const { RefCell::new(Vec::new()) };
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Kind {
    Tag(i64),
    Class(i64),
    Boxed,
}

#[derive(Default)]
struct State {
    names: HashMap<i64, String>,
    /// Par genre : (allouées, libérées).
    counts: HashMap<Kind, (u64, u64)>,
    /// Valeur vivante → (genre, lieu d'allocation, 0 si inconnu).
    live: HashMap<i64, (Kind, i64)>,
}

fn with_state(f: impl FnOnce(&mut State)) {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(State::default));
}

#[unsafe(no_mangle)]
pub extern "C" fn __leak_check_init(names: i64) {
    INIT.call_once(|| {
        let table = if names == 0 { "" } else { unsafe { crate::ptr_to_str(names) } };
        let names = table.split(';')
            .filter_map(|e| e.split_once('='))
            .filter_map(|(id, name)| Some((id.parse().ok()?, name.to_string())))
            .collect();
        with_state(|s| s.names = names);
        ENABLED.store(true, Ordering::Release);
        unsafe { libc::atexit(report) };
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn __leak_enter(site: i64) {
    let _ = STACK.try_with(|s| s.borrow_mut().push(site));
}

#[unsafe(no_mangle)]
pub extern "C" fn __leak_exit(site: i64) {
    let _ = STACK.try_with(|s| {
        let mut stack = s.borrow_mut();
        if let Some(pos) = stack.iter().rposition(|&x| x == site) {
            stack.truncate(pos);
        }
    });
}

#[inline]
pub(crate) fn on_alloc(kind: Kind, val: i64) {
    if ENABLED.load(Ordering::Relaxed) {
        let site = STACK.try_with(|s| s.borrow().last().copied().unwrap_or(0)).unwrap_or(0);
        with_state(|s| {
            s.counts.entry(kind).or_default().0 += 1;
            s.live.insert(val, (kind, site));
        });
    }
}

#[inline]
pub(crate) fn on_free(kind: Kind, val: i64) {
    if ENABLED.load(Ordering::Relaxed) {
        with_state(|s| {
            s.counts.entry(kind).or_default().1 += 1;
            s.live.remove(&val);
        });
    }
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

/// Lieu `nom␟fichier␟ligne` → (fonction, fichier, ligne).
fn site(site: i64) -> (String, String, String) {
    if site == 0 {
        return ("(hors fonction)".into(), String::new(), "0".into());
    }
    let text = unsafe { crate::ptr_to_str(site) };
    let mut parts = text.split('\u{1f}');
    let name = parts.next().unwrap_or("").to_string();
    (name, parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("0").to_string())
}

extern "C" fn report() {
    // Valeurs en attente dans le tampon du collecteur de cycles : libérées
    // par sa prochaine passe, ce ne sont pas des fuites.
    crate::rc::collect_cycles();
    ENABLED.store(false, Ordering::Release);
    let state = STATE.lock().unwrap_or_else(|e| e.into_inner()).take().unwrap_or_default();
    // Restants par genre, puis par lieu d'allocation.
    let mut by_kind: HashMap<Kind, HashMap<i64, u64>> = HashMap::new();
    for (kind, site) in state.live.values() {
        *by_kind.entry(*kind).or_default().entry(*site).or_default() += 1;
    }
    let mut kinds: Vec<(Kind, u64)> = by_kind.iter().map(|(k, sites)| (*k, sites.values().sum())).collect();
    kinds.sort_by(|a, b| b.1.cmp(&a.1));
    let mut text = String::new();
    let mut human = String::new();
    for (kind, live) in &kinds {
        let name = label(*kind, &state.names);
        let (allocated, freed) = state.counts.get(kind).copied().unwrap_or_default();
        text.push_str(&format!("K\t{}\t{}\t{}\t{}\n", name, live, allocated, freed));
        human.push_str(&format!("  {:>6} × {:<28} (allouées {}, libérées {})\n", live, name, allocated, freed));
        let mut sites: Vec<(&i64, &u64)> = by_kind[kind].iter().collect();
        sites.sort_by(|a, b| b.1.cmp(a.1));
        for (s, count) in sites {
            let (function, file, line) = site(*s);
            text.push_str(&format!("S\t{}\t{}\t{}\t{}\t{}\n", name, count, function, file, line));
            let at = if file.is_empty() { String::new() } else { format!(" — {}:{}", file, line) };
            human.push_str(&format!("           {:>6} alloué(s) dans {}{}\n", count, function, at));
        }
    }
    match std::env::var("OCARA_LEAK_OUT") {
        Ok(path) if !path.is_empty() => {
            let _ = std::fs::write(path, text);
        }
        _ if kinds.is_empty() => eprintln!("── fuites : aucune — toute valeur allouée a été libérée ──"),
        _ => {
            let total: u64 = kinds.iter().map(|k| k.1).sum();
            eprint!("── fuites : {} valeur(s) non libérée(s) à la sortie ──\n{}", total, human);
        }
    }
}
