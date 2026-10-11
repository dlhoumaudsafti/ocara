//! Profilage par fonction (`ocara --profile`) : le compilateur encadre chaque
//! fonction et méthode d'appels `__prof_enter(nom)` / `__prof_exit(nom)`
//! (nom = chaîne littérale statique). Par fonction : nombre d'appels, temps
//! total (appelées comprises) et temps propre. Rapport écrit à la sortie du
//! programme dans le fichier `OCARA_PROFILE_OUT` (sinon sur stderr), une
//! ligne par fonction : `appels<TAB>total_ns<TAB>propre_ns<TAB>nom`, temps
//! propre décroissant (total d'une fonction récursive : appel externe
//! seulement). Une sortie sautée (exception) est refermée par la
//! sortie de la fonction appelante.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Mutex, Once};
use std::time::{Duration, Instant};

struct Frame {
    name: i64,
    start: Instant,
    children: Duration,
}

#[derive(Default)]
struct Stat {
    calls: u64,
    total: Duration,
    own: Duration,
}

thread_local! {
    static STACK: RefCell<Vec<Frame>> = const { RefCell::new(Vec::new()) };
}

static STATS: Mutex<Option<HashMap<i64, Stat>>> = Mutex::new(None);
static REPORT_AT_EXIT: Once = Once::new();

#[unsafe(no_mangle)]
pub extern "C" fn __prof_enter(name: i64) {
    REPORT_AT_EXIT.call_once(|| unsafe {
        libc::atexit(report);
    });
    let frame = Frame { name, start: Instant::now(), children: Duration::ZERO };
    let _ = STACK.try_with(|s| s.borrow_mut().push(frame));
}

#[unsafe(no_mangle)]
pub extern "C" fn __prof_exit(name: i64) {
    let now = Instant::now();
    let _ = STACK.try_with(|s| {
        let mut stack = s.borrow_mut();
        if !stack.iter().any(|f| f.name == name) {
            return;
        }
        while let Some(frame) = stack.pop() {
            let total = now - frame.start;
            // Appel récursif : son total est déjà dans celui de l'appel externe.
            let nested = stack.iter().any(|f| f.name == frame.name);
            record(frame.name, if nested { Duration::ZERO } else { total }, total.saturating_sub(frame.children));
            if let Some(parent) = stack.last_mut() {
                parent.children += total;
            }
            if frame.name == name {
                break;
            }
        }
    });
}

fn record(name: i64, total: Duration, own: Duration) {
    let mut stats = STATS.lock().unwrap_or_else(|e| e.into_inner());
    let stat = stats.get_or_insert_with(HashMap::new).entry(name).or_default();
    stat.calls += 1;
    stat.total += total;
    stat.own += own;
}

extern "C" fn report() {
    let stats = STATS.lock().unwrap_or_else(|e| e.into_inner()).take().unwrap_or_default();
    let mut rows: Vec<(i64, Stat)> = stats.into_iter().collect();
    rows.sort_by(|a, b| b.1.own.cmp(&a.1.own));
    let text: String = rows.iter()
        .map(|(name, s)| format!("{}\t{}\t{}\t{}\n", s.calls, s.total.as_nanos(), s.own.as_nanos(), unsafe { crate::ptr_to_str(*name) }))
        .collect();
    match std::env::var("OCARA_PROFILE_OUT") {
        Ok(path) if !path.is_empty() => {
            let _ = std::fs::write(path, text);
        }
        _ => eprint!("── profil (appels, total ns, propre ns, fonction) ──\n{}", text),
    }
}
