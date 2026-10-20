// ─────────────────────────────────────────────────────────────────────────────
// Couverture par ligne (`ocara --coverage`, voir src/ir/coverage.rs)
//
//   __cov_init(n)                 → void  n points, rapport écrit à la sortie
//   __cov_point(i, fichier, ligne) → void  enregistre le point i
//   __cov_hit(i)                  → void  le point i a été exécuté
//
// À la sortie du programme (retour de `main`, `System::exit`, exception non
// rattrapée), si `OCARA_COVERAGE_FILE` est défini, une ligne par point y est
// écrite : `<0|1>\t<ligne>\t<fichier>`. Un arrêt par signal n'écrit rien.
// ─────────────────────────────────────────────────────────────────────────────

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::ptr_to_str;

static HITS: OnceLock<Box<[AtomicBool]>> = OnceLock::new();
static POINTS: Mutex<Vec<(String, i64)>> = Mutex::new(Vec::new());

#[unsafe(no_mangle)]
pub extern "C" fn __cov_init(count: i64) {
    let hits: Box<[AtomicBool]> = (0..count.max(0)).map(|_| AtomicBool::new(false)).collect();
    if HITS.set(hits).is_ok() {
        unsafe { libc::atexit(write_report) };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __cov_point(index: i64, file: i64, line: i64) {
    let mut points = POINTS.lock().unwrap_or_else(|p| p.into_inner());
    if points.len() as i64 == index {
        points.push((unsafe { ptr_to_str(file) }.to_string(), line));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __cov_hit(index: i64) {
    if let Some(hit) = HITS.get().and_then(|hits| hits.get(index as usize)) {
        hit.store(true, Ordering::Relaxed);
    }
}

extern "C" fn write_report() {
    let Some(path) = std::env::var_os("OCARA_COVERAGE_FILE") else { return };
    let Some(hits) = HITS.get() else { return };
    let points = POINTS.lock().unwrap_or_else(|p| p.into_inner());
    let mut report = String::new();
    for ((file, line), hit) in points.iter().zip(hits.iter()) {
        report.push_str(if hit.load(Ordering::Relaxed) { "1\t" } else { "0\t" });
        report.push_str(&line.to_string());
        report.push('\t');
        report.push_str(file);
        report.push('\n');
    }
    let _ = std::fs::write(path, report);
}
