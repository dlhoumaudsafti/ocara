//! Comptage de références atomique et détecteur de cycles synchrone
//! (Bacon–Rajan) — voir docs/roadmap.d/memoire-refcount.md.
//!
//! Toute valeur tas porte l'en-tête `[rc @ -24][aux @ -16][tag @ -8]` ; une
//! cellule boxée porte `[rc][bits]` avec `val = (cellule + 8) | tag`.

use std::alloc::{alloc, alloc_zeroed, dealloc, Layout};
use std::sync::atomic::{fence, AtomicBool, AtomicI64, AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::typecheck::{read_tag, TAG_ARRAY, TAG_CELL, TAG_ENV, TAG_EXCEPTION, TAG_FUNCTION, TAG_MAP, TAG_OBJECT, TAG_STRING_OWNED};

pub(crate) const HEADER: usize = 24;
pub(crate) const FLAG_RAW: i64 = 1;

const COUNT_MASK: i64 = 0xFFFF_FFFF;
const COLOR_SHIFT: u32 = 32;
const COLOR_MASK: i64 = 0xFF << COLOR_SHIFT;
const BUFFERED: i64 = 1 << 40;
const ROOTS_THRESHOLD: usize = 10_000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Color { Black = 0, Gray = 1, White = 2, Purple = 3 }

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind { String, Array, Map, Object, Function, Exception, Boxed, Cell, Env }

static ROOTS: Mutex<Vec<i64>> = Mutex::new(Vec::new());
static COLLECTING: AtomicBool = AtomicBool::new(false);
/// Taille du tampon des racines qui déclenche une collecte : relevée quand
/// beaucoup de racines restent vivantes, pour ne pas rebalayer sans cesse.
static ROOTS_LIMIT: AtomicUsize = AtomicUsize::new(ROOTS_THRESHOLD);

/// Threads Ocara secondaires en cours : la collecte des cycles n'a lieu
/// que lorsqu'il n'y en a aucun.
pub(crate) static ACTIVE_THREADS: AtomicUsize = AtomicUsize::new(0);

/// Compte un thread Ocara secondaire pendant toute sa durée de vie.
pub(crate) struct ThreadGuard;

impl ThreadGuard {
    pub(crate) fn new() -> Self {
        ACTIVE_THREADS.fetch_add(1, Ordering::AcqRel);
        ThreadGuard
    }
}

impl Drop for ThreadGuard {
    fn drop(&mut self) {
        ACTIVE_THREADS.fetch_sub(1, Ordering::AcqRel);
    }
}

fn block_layout(payload: usize) -> Layout {
    Layout::from_size_align(HEADER + payload, 8).unwrap()
}

/// Alloue un bloc compté (compte initial 1) et retourne le pointeur des données.
pub(crate) unsafe fn alloc_block(payload: usize, aux: i64, tag: i64, zeroed: bool) -> i64 {
    unsafe {
        let layout = block_layout(payload);
        let raw = if zeroed { alloc_zeroed(layout) } else { alloc(layout) };
        assert!(!raw.is_null(), "ocara_runtime: OOM");
        *(raw as *mut i64) = 1;
        *(raw.add(8) as *mut i64) = aux;
        *(raw.add(16) as *mut i64) = tag;
        raw as i64 + HEADER as i64
    }
}

pub(crate) unsafe fn free_block(val: i64, payload: usize) {
    unsafe { dealloc((val - HEADER as i64) as *mut u8, block_layout(payload)) }
}

pub(crate) unsafe fn aux(val: i64) -> i64 {
    unsafe { *((val - 16) as *const i64) }
}

unsafe fn set_aux(val: i64, value: i64) {
    unsafe { *((val - 16) as *mut i64) = value }
}

/// Objet : `[desc][rc][class_id][TAG_OBJECT][champs]`. `desc` pointe sur une
/// chaîne littérale `'0'`/`'1'`, un caractère par champ (`'1'` : champ tas).
pub(crate) unsafe fn alloc_object(size: usize, class_id: i64, desc: i64) -> i64 {
    unsafe {
        let raw = alloc_zeroed(object_layout(size));
        assert!(!raw.is_null(), "ocara_runtime: OOM");
        *(raw as *mut i64) = desc;
        *(raw.add(8) as *mut i64) = 1;
        *(raw.add(16) as *mut i64) = class_id;
        *(raw.add(24) as *mut i64) = TAG_OBJECT;
        raw as i64 + 8 + HEADER as i64
    }
}

fn object_layout(size: usize) -> Layout {
    Layout::from_size_align(8 + HEADER + size, 8).unwrap()
}

unsafe fn object_desc(val: i64) -> i64 {
    unsafe { *((val - 8 - HEADER as i64) as *const i64) }
}

unsafe fn object_mask<'a>(val: i64) -> &'a [u8] {
    unsafe {
        let desc = object_desc(val);
        if desc == 0 { &[] } else { crate::ptr_to_str(desc).as_bytes() }
    }
}

/// Sans descripteur, la taille de l'instance est inconnue : elle n'est
/// jamais libérée.
pub(crate) unsafe fn free_object(val: i64) {
    unsafe {
        if object_desc(val) == 0 {
            return;
        }
        let size = object_mask(val).len() * 8;
        dealloc((val - 8 - HEADER as i64) as *mut u8, object_layout(size));
    }
}

/// Cellule d'un primitif boxé : `[rc][bits]`, `val = (cellule + 8) | tag`.
pub(crate) unsafe fn alloc_box(bits: i64, tag: i64) -> i64 {
    unsafe {
        let raw = alloc(Layout::from_size_align(16, 8).unwrap());
        assert!(!raw.is_null(), "ocara_runtime: OOM");
        *(raw as *mut i64) = 1;
        *(raw.add(8) as *mut i64) = bits;
        (raw as i64 + 8) | tag
    }
}

pub(crate) unsafe fn free_box(val: i64) {
    unsafe { dealloc(((val & !3) - 8) as *mut u8, Layout::from_size_align(16, 8).unwrap()) }
}

fn is_boxed(val: i64) -> bool {
    val >= 0x10000 && (val & 3) != 0
}

unsafe fn kind(val: i64) -> Option<Kind> {
    if is_boxed(val) {
        return Some(Kind::Boxed);
    }
    match unsafe { read_tag(val) } {
        TAG_STRING_OWNED => Some(Kind::String),
        TAG_ARRAY => Some(Kind::Array),
        TAG_MAP => Some(Kind::Map),
        TAG_OBJECT => Some(Kind::Object),
        TAG_FUNCTION => Some(Kind::Function),
        TAG_EXCEPTION => Some(Kind::Exception),
        TAG_CELL => Some(Kind::Cell),
        TAG_ENV => Some(Kind::Env),
        _ => None,
    }
}

unsafe fn word<'a>(val: i64, k: Kind) -> &'a AtomicI64 {
    let addr = if k == Kind::Boxed { (val & !3) - 8 } else { val - HEADER as i64 };
    unsafe { &*(addr as *const AtomicI64) }
}

fn can_cycle(k: Kind) -> bool {
    matches!(k, Kind::Array | Kind::Map | Kind::Object | Kind::Function | Kind::Cell | Kind::Env)
}

fn count_of(w: &AtomicI64) -> i64 {
    w.load(Ordering::Acquire) & COUNT_MASK
}

fn color_of(w: &AtomicI64) -> Color {
    match (w.load(Ordering::Acquire) & COLOR_MASK) >> COLOR_SHIFT {
        1 => Color::Gray,
        2 => Color::White,
        3 => Color::Purple,
        _ => Color::Black,
    }
}

fn set_color(w: &AtomicI64, c: Color) {
    let _ = w.fetch_update(Ordering::AcqRel, Ordering::Acquire, |x| {
        Some((x & !COLOR_MASK) | ((c as i64) << COLOR_SHIFT))
    });
}

fn is_buffered(w: &AtomicI64) -> bool {
    w.load(Ordering::Acquire) & BUFFERED != 0
}

/// Enfants comptés d'une valeur (éléments, champs tas, message d'exception).
unsafe fn for_each_child(val: i64, k: Kind, f: &mut dyn FnMut(i64)) {
    unsafe {
        match k {
            Kind::Array if aux(val) & FLAG_RAW == 0 => {
                for &el in &crate::array_data(val) { f(el) }
            }
            Kind::Map if aux(val) & FLAG_RAW == 0 => {
                for el in crate::map_values(val) { f(el) }
            }
            Kind::Object => {
                for (i, &b) in object_mask(val).iter().enumerate() {
                    if b == b'1' { f(*((val + 8 * i as i64) as *const i64)) }
                }
            }
            Kind::Exception => {
                for v in crate::exception::exception_children(val) { f(v) }
            }
            Kind::Function => f(*((val + 8) as *const i64)),
            Kind::Cell if aux(val) & 1 != 0 => f(*(val as *const i64)),
            Kind::Env => {
                for i in 0..(aux(val) & COUNT_MASK) {
                    f(*((val + 8 * i) as *const i64))
                }
            }
            _ => {}
        }
    }
}

/// Libère la mémoire d'un bloc, sans toucher à ses enfants.
unsafe fn dealloc_block(val: i64, k: Kind) {
    unsafe {
        match k {
            Kind::String => crate::free_str(val),
            Kind::Array => crate::drop_array_block(val),
            Kind::Map => crate::drop_map_block(val),
            Kind::Object => free_object(val),
            Kind::Function => free_block(val, 16),
            Kind::Exception => crate::exception::free_exception_block(val),
            Kind::Boxed => free_box(val),
            Kind::Cell => crate::free_cell(val),
            Kind::Env => free_block(val, (aux(val) >> 32) as usize * 8),
        }
    }
}

pub(crate) unsafe fn retain(val: i64) {
    unsafe {
        if let Some(k) = kind(val) {
            word(val, k).fetch_add(1, Ordering::Relaxed);
        }
    }
}

pub(crate) unsafe fn release(val: i64) {
    unsafe {
        let Some(k) = kind(val) else { return };
        let w = word(val, k);
        if w.fetch_sub(1, Ordering::Release) & COUNT_MASK == 1 {
            fence(Ordering::Acquire);
            destroy(val, k);
        } else if can_cycle(k) {
            possible_root(val, w);
        }
    }
}

/// Compte à zéro : relâche les enfants (sans récursion), puis libère le
/// bloc — sauf s'il est dans le tampon des racines, où la collecte le
/// libérera.
unsafe fn destroy(val: i64, k: Kind) {
    unsafe {
        let mut stack = vec![(val, k)];
        while let Some((x, xk)) = stack.pop() {
            for_each_child(x, xk, &mut |t| {
                let Some(tk) = kind(t) else { return };
                let tw = word(t, tk);
                if tw.fetch_sub(1, Ordering::Release) & COUNT_MASK == 1 {
                    fence(Ordering::Acquire);
                    stack.push((t, tk));
                } else if can_cycle(tk) {
                    possible_root(t, tw);
                }
            });
            let xw = word(x, xk);
            if is_buffered(xw) {
                set_color(xw, Color::Black);
            } else {
                dealloc_block(x, xk);
            }
        }
    }
}

fn possible_root(val: i64, w: &AtomicI64) {
    set_color(w, Color::Purple);
    let previous = w.fetch_or(BUFFERED, Ordering::AcqRel);
    if previous & BUFFERED != 0 {
        return;
    }
    let full = {
        let mut roots = ROOTS.lock().unwrap_or_else(|e| e.into_inner());
        roots.push(val);
        roots.len() >= ROOTS_LIMIT.load(Ordering::Acquire)
    };
    if full {
        collect_cycles();
    }
}

/// Collecte synchrone des cycles, ou seulement le balayage des racines
/// mortes si un thread Ocara secondaire tourne (marquer pendant qu'un autre
/// thread modifie des comptes serait faux). Sans effet si une collecte est
/// déjà en cours.
pub(crate) fn collect_cycles() {
    if COLLECTING.swap(true, Ordering::AcqRel) {
        return;
    }
    if ACTIVE_THREADS.load(Ordering::Acquire) != 0 {
        sweep_dead_roots();
        COLLECTING.store(false, Ordering::Release);
        return;
    }
    let roots = std::mem::take(&mut *ROOTS.lock().unwrap_or_else(|e| e.into_inner()));
    unsafe {
        let mut kept = Vec::new();
        for s in roots {
            let Some(k) = kind(s) else { continue };
            let w = word(s, k);
            if color_of(w) == Color::Purple && count_of(w) > 0 {
                mark_gray(s, k);
                kept.push((s, k));
            } else {
                w.fetch_and(!BUFFERED, Ordering::AcqRel);
                if color_of(w) == Color::Black && count_of(w) == 0 {
                    dealloc_block(s, k);
                }
            }
        }
        for &(s, k) in &kept {
            scan(s, k);
        }
        for &(s, k) in &kept {
            word(s, k).fetch_and(!BUFFERED, Ordering::AcqRel);
        }
        for (s, k) in kept {
            collect_white(s, k);
        }
    }
    ROOTS_LIMIT.store(ROOTS_THRESHOLD, Ordering::Release);
    COLLECTING.store(false, Ordering::Release);
}

/// Libère les racines mortes : compte à zéro et couleur noire, posée par
/// `destroy` une fois ses enfants relâchés — plus rien ne peut les atteindre,
/// même depuis un autre thread. Les racines vivantes restent en attente.
fn sweep_dead_roots() {
    let roots = std::mem::take(&mut *ROOTS.lock().unwrap_or_else(|e| e.into_inner()));
    let mut alive = Vec::with_capacity(roots.len());
    unsafe {
        for s in roots {
            let Some(k) = kind(s) else { continue };
            let w = word(s, k);
            if count_of(w) == 0 && color_of(w) == Color::Black {
                dealloc_block(s, k);
            } else {
                alive.push(s);
            }
        }
    }
    ROOTS_LIMIT.store((alive.len() * 2).max(ROOTS_THRESHOLD), Ordering::Release);
    ROOTS.lock().unwrap_or_else(|e| e.into_inner()).extend(alive);
}

unsafe fn cyclic_children(val: i64, k: Kind) -> Vec<(i64, Kind)> {
    let mut out = Vec::new();
    unsafe {
        for_each_child(val, k, &mut |t| {
            if let Some(tk) = kind(t).filter(|tk| can_cycle(*tk)) {
                out.push((t, tk));
            }
        });
    }
    out
}

unsafe fn mark_gray(val: i64, k: Kind) {
    unsafe {
        let mut stack = vec![(val, k)];
        while let Some((x, xk)) = stack.pop() {
            let xw = word(x, xk);
            if color_of(xw) == Color::Gray {
                continue;
            }
            set_color(xw, Color::Gray);
            for (t, tk) in cyclic_children(x, xk) {
                word(t, tk).fetch_sub(1, Ordering::AcqRel);
                stack.push((t, tk));
            }
        }
    }
}

unsafe fn scan(val: i64, k: Kind) {
    unsafe {
        let mut stack = vec![(val, k)];
        while let Some((x, xk)) = stack.pop() {
            let xw = word(x, xk);
            if color_of(xw) != Color::Gray {
                continue;
            }
            if count_of(xw) > 0 {
                scan_black(x, xk);
            } else {
                set_color(xw, Color::White);
                stack.extend(cyclic_children(x, xk));
            }
        }
    }
}

unsafe fn scan_black(val: i64, k: Kind) {
    unsafe {
        set_color(word(val, k), Color::Black);
        let mut stack = vec![(val, k)];
        while let Some((x, xk)) = stack.pop() {
            for (t, tk) in cyclic_children(x, xk) {
                let tw = word(t, tk);
                tw.fetch_add(1, Ordering::AcqRel);
                if color_of(tw) != Color::Black {
                    set_color(tw, Color::Black);
                    stack.push((t, tk));
                }
            }
        }
    }
}

unsafe fn collect_white(val: i64, k: Kind) {
    unsafe {
        let mut stack = vec![(val, k)];
        let mut garbage = Vec::new();
        while let Some((x, xk)) = stack.pop() {
            let xw = word(x, xk);
            if color_of(xw) != Color::White || is_buffered(xw) {
                continue;
            }
            set_color(xw, Color::Black);
            for_each_child(x, xk, &mut |t| match kind(t) {
                Some(tk) if can_cycle(tk) => stack.push((t, tk)),
                Some(_) => release(t),
                None => {}
            });
            garbage.push((x, xk));
        }
        for (x, xk) in garbage {
            dealloc_block(x, xk);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __rc_retain(val: i64) {
    unsafe { retain(val) }
}

#[unsafe(no_mangle)]
pub extern "C" fn __rc_release(val: i64) {
    unsafe { release(val) }
}

#[unsafe(no_mangle)]
pub extern "C" fn __rc_collect_cycles() {
    collect_cycles();
}

/// Compte courant (tests et diagnostic) ; `0` pour une valeur non comptée.
#[unsafe(no_mangle)]
pub extern "C" fn __rc_count(val: i64) -> i64 {
    unsafe { kind(val).map(|k| count_of(word(val, k))).unwrap_or(0) }
}

/// Conteneur à éléments `int`/`float`/`bool` bruts : jamais suivis comme pointeurs.
#[unsafe(no_mangle)]
pub extern "C" fn __rc_mark_raw(val: i64) -> i64 {
    unsafe {
        if matches!(kind(val), Some(Kind::Array | Kind::Map)) {
            set_aux(val, aux(val) | FLAG_RAW);
        }
    }
    val
}
