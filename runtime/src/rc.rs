//! Comptage de références atomique et détecteur de cycles synchrone
//! (Bacon–Rajan) — voir docs/roadmap.d/memoire-refcount.md.
//!
//! Toute valeur tas porte l'en-tête `[rc @ -24][aux @ -16][tag @ -8]` ; une
//! cellule boxée porte `[rc][bits]` avec `val = (cellule + 8) | tag`.

use std::alloc::Layout;

use crate::alloc_cache::{alloc, alloc_zeroed, dealloc};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::typecheck::{read_tag, TAG_ARRAY, TAG_CELL, TAG_ENV, TAG_EXCEPTION, TAG_FUNCTION, TAG_GEN, TAG_MAP, TAG_OBJECT, TAG_STRING_OWNED, TAG_TASK};

pub(crate) const HEADER: usize = 24;
pub(crate) const FLAG_RAW: i64 = 1;
/// Type des éléments bruts d'un conteneur `FLAG_RAW` (bits 1-2 de `aux`) :
/// 1 = int, 2 = float, 3 = bool, 0 = non précisé (traité comme int). Lu par
/// JSON/YAML et `Map::forEach` quand le compilateur ne connaît pas la forme
/// (tableau concret placé dans un `mixed`).
const RAW_KIND_SHIFT: u32 = 1;
const RAW_KIND_MASK: i64 = 0b11 << RAW_KIND_SHIFT;

pub(crate) const COUNT_MASK: i64 = 0xFFFF_FFFF;
const COLOR_SHIFT: u32 = 32;
const COLOR_MASK: i64 = 0xFF << COLOR_SHIFT;
const BUFFERED: i64 = 1 << 40;
/// Époque de la dernière collecte concurrente pendant laquelle le compte a
/// changé (bits 48–55) : la valeur est vivante pour cette collecte (voir
/// `rc_concurrent`). Une époque, pas un drapeau : rien à effacer, et une
/// valeur touchée pendant une collecte ne passe pas pour touchée à la
/// suivante.
const EPOCH_SHIFT: u32 = 48;
const EPOCH_MASK: i64 = 0xFF << EPOCH_SHIFT;
/// Détruite (enfants rendus) alors qu'elle était dans le tampon des racines :
/// la collecte libérera son bloc. Posé et lu avec `BUFFERED` par des
/// opérations atomiques uniques — un seul des deux threads la libère.
pub(crate) const DEAD: i64 = 1 << 42;
const ROOTS_THRESHOLD: usize = 10_000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Color { Black = 0, Gray = 1, White = 2, Purple = 3 }

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind { String, Array, Map, Object, Function, Exception, Boxed, Cell, Env, Generator, Task }

static ROOTS: Mutex<Vec<i64>> = Mutex::new(Vec::new());
/// Une collecte est en cours (une seule à la fois).
static COLLECTING: AtomicBool = AtomicBool::new(false);
/// Collecte synchrone en cours : les autres threads Ocara sont garés et ne
/// reprennent qu'après.
static EXCLUSIVE: AtomicBool = AtomicBool::new(false);
/// Époque de la collecte concurrente en cours (0 : aucune) : `retain`/
/// `release` y marquent les valeurs touchées, les libérations sont différées.
static EPOCH: AtomicI64 = AtomicI64::new(0);
static LAST_EPOCH: AtomicI64 = AtomicI64::new(0);
static DEFERRED: Mutex<Vec<i64>> = Mutex::new(Vec::new());
/// Taille du tampon des racines qui déclenche une collecte : relevée quand
/// beaucoup de racines restent vivantes, pour ne pas rebalayer sans cesse.
static ROOTS_LIMIT: AtomicUsize = AtomicUsize::new(ROOTS_THRESHOLD);

/// Threads Ocara en train d'exécuter du code, le thread principal compris.
/// Un thread bloqué dans un appel du runtime (`recv`, `join`, `sleep`,
/// attente d'un verrou…) est « garé » (`park`) : il ne touche à aucun
/// compte. La collecte des cycles a lieu quand le thread qui la lance est le
/// seul en cours.
static RUNNING: AtomicUsize = AtomicUsize::new(1);

/// Compte un thread Ocara secondaire pendant toute sa durée de vie.
pub(crate) struct ThreadGuard;

impl ThreadGuard {
    pub(crate) fn new() -> Self {
        go_multi_threaded();
        RUNNING.fetch_add(1, Ordering::SeqCst);
        ThreadGuard
    }
}

/// Un autre thread a exécuté (ou va exécuter) du code Ocara : les comptes
/// sont désormais modifiés par opérations atomiques. Tant qu'un seul thread
/// en exécute, une simple lecture-écriture suffit (≈ 10 fois moins chère
/// qu'un `lock xadd`) — sans retour possible.
static MULTI_THREADED: AtomicBool = AtomicBool::new(false);

pub(crate) fn is_multi_threaded() -> bool {
    MULTI_THREADED.load(Ordering::Relaxed)
}

/// À appeler par le thread qui en crée un autre, AVANT sa création (ou par
/// un thread étranger qui entre, voir `enter`).
pub(crate) fn go_multi_threaded() {
    MULTI_THREADED.store(true, Ordering::SeqCst);
}

/// Ajoute `delta` au mot de compte ; rend sa valeur précédente.
fn add_count(w: &AtomicI64, delta: i64) -> i64 {
    if MULTI_THREADED.load(Ordering::Relaxed) {
        let previous = w.fetch_add(delta, Ordering::SeqCst);
        touch(w);
        previous
    } else {
        let previous = w.load(Ordering::Relaxed);
        w.store(previous + delta, Ordering::Relaxed);
        previous
    }
}

/// Pose `bits` dans le mot de compte ; rend sa valeur précédente.
fn or_flags(w: &AtomicI64, bits: i64) -> i64 {
    if MULTI_THREADED.load(Ordering::Relaxed) {
        w.fetch_or(bits, Ordering::SeqCst)
    } else {
        let previous = w.load(Ordering::Relaxed);
        w.store(previous | bits, Ordering::Relaxed);
        previous
    }
}

impl Drop for ThreadGuard {
    fn drop(&mut self) {
        RUNNING.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Gare le thread courant le temps d'un appel bloquant (boucle d'événements
/// Tauri, attente SDL comprises) ; au réveil, il attend la fin d'une
/// collecte en cours avant de reprendre.
pub struct Parked;

pub fn park() -> Parked {
    RUNNING.fetch_sub(1, Ordering::SeqCst);
    Parked
}

impl Drop for Parked {
    fn drop(&mut self) {
        resume();
    }
}

fn resume() {
    loop {
        RUNNING.fetch_add(1, Ordering::SeqCst);
        if !EXCLUSIVE.load(Ordering::SeqCst) {
            return;
        }
        RUNNING.fetch_sub(1, Ordering::SeqCst);
        while EXCLUSIVE.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }
    }
}

/// Compte le thread courant le temps d'un appel vers du code Ocara depuis un
/// thread garé ou étranger (handler appelé par la boucle d'événements).
pub struct Entered;

pub fn enter() -> Entered {
    go_multi_threaded();
    resume();
    Entered
}

impl Drop for Entered {
    fn drop(&mut self) {
        RUNNING.fetch_sub(1, Ordering::SeqCst);
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
        let val = raw as i64 + HEADER as i64;
        crate::leak::on_alloc(crate::leak::Kind::Tag(tag), val);
        val
    }
}

pub(crate) unsafe fn free_block(val: i64, payload: usize) {
    unsafe {
        crate::leak::on_free(crate::leak::Kind::Tag(*((val - 8) as *const i64)), val);
        dealloc((val - HEADER as i64) as *mut u8, block_layout(payload))
    }
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
        let val = raw as i64 + 8 + HEADER as i64;
        crate::leak::on_alloc(crate::leak::Kind::Class(class_id), val);
        val
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
/// Finalisations d'un objet, une seule fois avant sa libération : un champ
/// marqué d'une lettre est un handle de ressource fermé avec l'objet (puis
/// remis à zéro) ; `X`/`T` ferment l'objet builtin opaque lui-même
/// (`Mutex`, `Thread` jamais attendu, détaché).
pub(crate) unsafe fn finalize_object(val: i64) {
    unsafe {
        for (i, &kind) in object_mask(val).iter().enumerate() {
            let field = (val + 8 * i as i64) as *mut i64;
            match kind {
                b'0' | b'1' => continue,
                b'X' => crate::mutex::Mutex_destroy(val),
                b'T' => crate::thread::Thread_detach(val),
                _ if *field == 0 => continue,
                b'S' => crate::sqlite::SQLite_close(*field),
                b'Y' => crate::mysql::MySQL_close(*field),
                b'B' => crate::mysql::MariaDB_close(*field),
                b'Q' => crate::httprequest::HTTPRequest_close(*field),
                b'P' => crate::httprequest::HTTPRequest_closeResponse(*field),
                _ => continue,
            }
            *field = 0;
        }
    }
}

pub(crate) unsafe fn free_object(val: i64) {
    unsafe {
        if object_desc(val) == 0 {
            return;
        }
        let size = object_mask(val).len() * 8;
        crate::leak::on_free(crate::leak::Kind::Class(*((val - 16) as *const i64)), val);
        dealloc((val - 8 - HEADER as i64) as *mut u8, object_layout(size));
    }
}

/// Handle natif (`TAG_HANDLE`, non compté) contenant `value` ; remplace
/// `Box::into_raw` pour toute structure Rust rendue comme valeur Ocara.
pub(crate) fn handle_new<T>(value: T) -> i64 {
    assert!(std::mem::align_of::<T>() <= 8, "ocara_runtime: handle trop aligné");
    let size = std::mem::size_of::<T>();
    unsafe {
        let ptr = alloc_block(size.max(8), size as i64, crate::typecheck::TAG_HANDLE, false);
        std::ptr::write(ptr as *mut T, value);
        ptr
    }
}

/// Reprend la structure d'un handle et libère son bloc (remplace `Box::from_raw`).
pub(crate) unsafe fn handle_take<T>(ptr: i64) -> T {
    unsafe {
        let value = std::ptr::read(ptr as *const T);
        free_block(ptr, std::mem::size_of::<T>().max(8));
        value
    }
}

/// Cellule d'un primitif boxé : `[rc][bits]`, `val = (cellule + 8) | tag`.
pub(crate) unsafe fn alloc_box(bits: i64, tag: i64) -> i64 {
    unsafe {
        let raw = alloc(Layout::from_size_align(16, 8).unwrap());
        assert!(!raw.is_null(), "ocara_runtime: OOM");
        *(raw as *mut i64) = 1;
        *(raw.add(8) as *mut i64) = bits;
        let val = (raw as i64 + 8) | tag;
        crate::leak::on_alloc(crate::leak::Kind::Boxed, val);
        val
    }
}

pub(crate) unsafe fn free_box(val: i64) {
    crate::leak::on_free(crate::leak::Kind::Boxed, val);
    unsafe { dealloc(((val & !3) - 8) as *mut u8, Layout::from_size_align(16, 8).unwrap()) }
}

/// Cellule d'un primitif (`ptr | 1..3`) — pas une valeur en ligne.
fn is_boxed(val: i64) -> bool {
    (0x10000..crate::POINTER_LIMIT).contains(&val) && (val & 3) != 0
}

pub(crate) unsafe fn kind(val: i64) -> Option<Kind> {
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
        TAG_GEN => Some(Kind::Generator),
        TAG_TASK => Some(Kind::Task),
        _ => None,
    }
}

pub(crate) unsafe fn word<'a>(val: i64, k: Kind) -> &'a AtomicI64 {
    let addr = if k == Kind::Boxed { (val & !3) - 8 } else { val - HEADER as i64 };
    unsafe { &*(addr as *const AtomicI64) }
}

pub(crate) fn can_cycle(k: Kind) -> bool {
    matches!(k, Kind::Array | Kind::Map | Kind::Object | Kind::Function | Kind::Cell | Kind::Env)
}

pub(crate) fn count_of(w: &AtomicI64) -> i64 {
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
pub(crate) unsafe fn for_each_child(val: i64, k: Kind, f: &mut dyn FnMut(i64)) {
    unsafe {
        match k {
            // Parcours sous le verrou du conteneur, sans copie : `f` ne
            // lance jamais de collecte (différée à la fin du `release`).
            Kind::Array if aux(val) & FLAG_RAW == 0 => {
                crate::containers::with_array(val, |data| for &el in data.iter() { f(el) })
            }
            Kind::Map if aux(val) & FLAG_RAW == 0 => {
                crate::containers::with_map(val, |data| for el in data.values() { f(el) })
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

/// Libère la mémoire d'un bloc, sans toucher à ses enfants — différée
/// pendant une collecte concurrente, qui peut encore lire ce bloc.
unsafe fn dealloc_block(val: i64, k: Kind) {
    if EPOCH.load(Ordering::SeqCst) != 0 {
        DEFERRED.lock().unwrap_or_else(|e| e.into_inner()).push(val);
        return;
    }
    unsafe { dealloc_now(val, k) }
}

/// Libère tout de suite (collecteur, ou hors collecte concurrente).
pub(crate) unsafe fn dealloc_now(val: i64, k: Kind) {
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
            Kind::Generator => {
                let drop: unsafe extern "C" fn(i64) = std::mem::transmute(aux(val) as usize);
                drop(val);
            }
            Kind::Task => crate::task::free_task(val),
        }
    }
}

/// Pendant une collecte concurrente, marque la valeur dont le compte vient
/// de changer. `SeqCst` (opération de compte comprise) : une opération qui
/// ne voit pas l'époque est visible du collecteur, qui ne lit les comptes
/// qu'après l'avoir publiée.
fn touch(w: &AtomicI64) {
    let epoch = EPOCH.load(Ordering::SeqCst);
    if epoch != 0 {
        let _ = w.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |x| {
            Some((x & !EPOCH_MASK) | (epoch << EPOCH_SHIFT))
        });
    }
}

/// Le compte de `w` a changé pendant la collecte concurrente `epoch`.
pub(crate) fn touched_in(w: &AtomicI64, epoch: i64) -> bool {
    (w.load(Ordering::SeqCst) & EPOCH_MASK) >> EPOCH_SHIFT == epoch
}

pub(crate) unsafe fn retain(val: i64) {
    unsafe {
        if let Some(k) = kind(val) {
            add_count(word(val, k), 1);
        }
    }
}

/// Ni pointeur ni cellule (entier brut, valeur en ligne, null) : rien à
/// rendre. Testé avant l'appel du chemin complet, dont le prologue coûtait
/// autant que le test sur une boucle `mixed`.
#[inline]
pub(crate) unsafe fn release(val: i64) {
    if (0x10000..crate::POINTER_LIMIT).contains(&val) {
        unsafe { release_out_of_line(val) }
    }
}

#[inline(never)]
unsafe fn release_out_of_line(val: i64) {
    unsafe { release_counted(val) }
}

/// Chemin complet de `release` : sûr pour toute valeur (`kind` écarte ce qui
/// n'est pas compté). Appelé directement par `__rc_release`, que le code
/// généré ne lance déjà que pour une valeur qui peut être comptée.
#[inline]
unsafe fn release_counted(val: i64) {
    unsafe {
        let Some(k) = kind(val) else { return };
        if decrement(val, k, word(val, k)) {
            destroy(val, k);
        }
    }
    // Collecte lancée APRÈS la décrémentation : lancée au milieu (racine
    // inscrite, référence pas encore rendue), une collecte exclusive jugeait
    // la valeur vivante et la retirait du tampon — la décrémentation qui
    // suivait laissait alors un cycle poubelle sans racine.
    if COLLECT_DUE.with(|c| c.replace(false)) {
        collect_due();
    }
}

/// Relâche une chaîne sans le détour de `destroy` (feuille : ni enfant, ni
/// cycle, ni finalisation) — clés d'une map.
pub(crate) unsafe fn release_string(val: i64) {
    unsafe {
        if kind(val) == Some(Kind::String) && add_count(word(val, Kind::String), -1) & COUNT_MASK == 1 {
            dealloc_block(val, Kind::String);
        }
    }
}

/// Rend une référence ; vrai si c'était la dernière. Une valeur susceptible
/// de former un cycle et encore partagée est inscrite parmi les racines
/// AVANT de rendre la référence : après, un autre thread peut l'avoir
/// libérée. Un compte de 1 n'est jamais partagé (personne d'autre ne peut
/// l'atteindre pour la retenir) ; si elle meurt quand même entre-temps,
/// `destroy` la voit inscrite et laisse la collecte la libérer.
unsafe fn decrement(val: i64, k: Kind, w: &AtomicI64) -> bool {
    if can_cycle(k) && count_of(w) > 1 && unsafe { may_hold_references(val, k) } {
        possible_root(val, w);
    }
    add_count(w, -1) & COUNT_MASK == 1
}

/// Compte à zéro : relâche les enfants (sans récursion), puis libère le
/// bloc — sauf s'il est dans le tampon des racines, où la collecte le
/// libérera.
unsafe fn destroy(val: i64, k: Kind) {
    unsafe {
        // La pile n'alloue qu'à la première valeur enfant à détruire (une
        // chaîne libérée allouait un `Vec`).
        let mut stack: Vec<(i64, Kind)> = Vec::new();
        let mut current = Some((val, k));
        while let Some((x, xk)) = current.take().or_else(|| stack.pop()) {
            if xk == Kind::Object {
                finalize_object(x);
            }
            for_each_child(x, xk, &mut |t| {
                let Some(tk) = kind(t) else { return };
                if decrement(t, tk, word(t, tk)) {
                    // Feuille (chaîne, valeur boxée) : sans enfant ni
                    // finalisation, libérée sur-le-champ.
                    if matches!(tk, Kind::String | Kind::Boxed) {
                        dealloc_block(t, tk);
                    } else {
                        stack.push((t, tk));
                    }
                }
            });
            // Une valeur qui ne peut pas former de cycle n'est jamais dans le
            // tampon des racines.
            if !can_cycle(xk) || or_flags(word(x, xk), DEAD) & BUFFERED == 0 {
                dealloc_block(x, xk);
            }
        }
    }
}

/// Valeur qui peut référencer d'autres valeurs comptées, donc appartenir à
/// un cycle : objet à champ référence, conteneur d'éléments non bruts,
/// closure avec environnement, cellule comptée, environnement non vide.
/// Les autres ne passent jamais par le tampon des racines.
unsafe fn may_hold_references(val: i64, k: Kind) -> bool {
    unsafe {
        match k {
            Kind::Object => object_mask(val).contains(&b'1'),
            Kind::Array | Kind::Map => aux(val) & FLAG_RAW == 0,
            Kind::Function => *((val + 8) as *const i64) != 0,
            Kind::Cell => aux(val) & 1 != 0,
            Kind::Env => aux(val) & COUNT_MASK != 0,
            _ => false,
        }
    }
}

fn possible_root(val: i64, w: &AtomicI64) {
    // Déjà inscrite et violette : rien à changer.
    let current = w.load(Ordering::Acquire);
    if current & BUFFERED != 0 && (current & COLOR_MASK) >> COLOR_SHIFT == Color::Purple as i64 {
        return;
    }
    set_color(w, Color::Purple);
    let previous = or_flags(w, BUFFERED);
    if previous & BUFFERED != 0 {
        return;
    }
    let pending = {
        let mut roots = ROOTS.lock().unwrap_or_else(|e| e.into_inner());
        roots.push(val);
        roots.len()
    };
    if pending >= ROOTS_LIMIT.load(Ordering::Acquire) {
        COLLECT_DUE.with(|c| c.set(true));
    }
}

/// Seuil du tampon des racines atteint : collecte.
fn collect_due() {
    let pending = ROOTS.lock().unwrap_or_else(|e| e.into_inner()).len();
    let limit = ROOTS_LIMIT.load(Ordering::Acquire);
    // Contre-pression : les autres threads produisent des racines pendant
    // une collecte concurrente ; au-delà de 4 fois le seuil, attendre sa fin
    // borne la mémoire au rythme du collecteur.
    if pending >= limit * 4 && !IN_COLLECTOR.with(|c| c.get()) {
        while COLLECTING.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }
    }
    collect_cycles();
}

thread_local! {
    /// Le tampon des racines a atteint son seuil pendant un `release` du
    /// thread courant : collecte à la fin de ce `release`.
    static COLLECT_DUE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Le thread courant exécute une collecte : ses propres `release` ne
    /// l'attendent jamais.
    static IN_COLLECTOR: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Collecte des cycles, une seule à la fois. Thread courant seul en cours :
/// collecte synchrone (les autres sont garés et attendent `EXCLUSIVE`) ;
/// sinon collecte concurrente (`rc_concurrent`), les autres threads
/// continuent.
pub(crate) fn collect_cycles() {
    if COLLECTING.swap(true, Ordering::SeqCst) {
        return;
    }
    IN_COLLECTOR.with(|c| c.set(true));
    EXCLUSIVE.store(true, Ordering::SeqCst);
    if RUNNING.load(Ordering::SeqCst) == 1 {
        unsafe { collect_exclusive() }
        EXCLUSIVE.store(false, Ordering::SeqCst);
    } else {
        EXCLUSIVE.store(false, Ordering::SeqCst);
        collect_concurrent();
    }
    IN_COLLECTOR.with(|c| c.set(false));
    COLLECTING.store(false, Ordering::SeqCst);
}

fn take_roots() -> Vec<i64> {
    std::mem::take(&mut *ROOTS.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Retire `val` du tampon des racines ; la libère si elle a été détruite
/// pendant qu'elle y était (voir `DEAD`).
pub(crate) unsafe fn unbuffer(val: i64, k: Kind) {
    unsafe {
        if word(val, k).fetch_and(!BUFFERED, Ordering::SeqCst) & DEAD != 0 {
            dealloc_now(val, k);
        }
    }
}
/// Suppression d'essai (Bacon–Rajan) : modifie les comptes, donc réservée au
/// thread seul en cours.
unsafe fn collect_exclusive() {
    unsafe {
        let mut kept = Vec::new();
        for s in take_roots() {
            let Some(k) = kind(s) else { continue };
            let w = word(s, k);
            if color_of(w) == Color::Purple && count_of(w) > 0 {
                mark_gray(s, k);
                kept.push((s, k));
            } else {
                unbuffer(s, k);
            }
        }
        for &(s, k) in &kept {
            scan(s, k);
        }
        // Seuil relevé par les racines restées vivantes : sans ça, un grand
        // graphe vivant était reparcouru tous les `ROOTS_THRESHOLD` candidats.
        let live = kept.iter().filter(|&&(s, k)| color_of(word(s, k)) == Color::Black).count();
        for &(s, k) in &kept {
            word(s, k).fetch_and(!BUFFERED, Ordering::AcqRel);
        }
        for (s, k) in kept {
            collect_white(s, k);
        }
        ROOTS_LIMIT.store((live * 2).max(ROOTS_THRESHOLD), Ordering::Release);
    }
}

/// Collecte pendant que d'autres threads Ocara modifient des comptes :
/// libérations différées pendant l'époque de collecte, racines touchées
/// remises dans le tampon.
fn collect_concurrent() {
    let epoch = LAST_EPOCH.load(Ordering::Relaxed) % 255 + 1;
    LAST_EPOCH.store(epoch, Ordering::Relaxed);
    EPOCH.store(epoch, Ordering::SeqCst);
    let kept = unsafe { crate::rc_concurrent::collect(take_roots(), epoch) };
    EPOCH.store(0, Ordering::SeqCst);
    let deferred = std::mem::take(&mut *DEFERRED.lock().unwrap_or_else(|e| e.into_inner()));
    for val in deferred {
        if let Some(k) = unsafe { kind(val) } {
            unsafe { dealloc_now(val, k) }
        }
    }
    // Seuil relevé par les seules racines remises (encore vivantes), pas par
    // celles arrivées pendant la collecte.
    ROOTS_LIMIT.store((kept.len() * 2).max(ROOTS_THRESHOLD), Ordering::Release);
    ROOTS.lock().unwrap_or_else(|e| e.into_inner()).extend(kept);
}

pub(crate) unsafe fn cyclic_children(val: i64, k: Kind) -> Vec<(i64, Kind)> {
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
            if xk == Kind::Object {
                finalize_object(x);
            }
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

// ── Déroulement par `raise` ───────────────────────────────────────────────────
//
// Une fonction qui a des locales comptées les range dans un tableau de mots
// sur sa pile, enregistré ici à son entrée et retiré à sa sortie. Un `raise`
// saute par `longjmp` jusqu'au `try` : avant le saut (les frames sont encore
// vivantes), `unwind_to` rend les locales des fonctions sautées.

thread_local! {
    static UNWIND: std::cell::UnsafeCell<Vec<(i64, i64)>> = const { std::cell::UnsafeCell::new(Vec::new()) };
}

#[unsafe(no_mangle)]
pub extern "C" fn __rc_unwind_push(base: i64, words: i64) {
    unsafe { std::ptr::write_bytes(base as *mut i64, 0, words.max(0) as usize) }
    UNWIND.with(|u| unsafe { (*u.get()).push((base, words)) });
}

#[unsafe(no_mangle)]
pub extern "C" fn __rc_unwind_pop() {
    UNWIND.with(|u| unsafe { (*u.get()).pop() });
}

pub(crate) fn unwind_depth() -> usize {
    UNWIND.with(|u| unsafe { (*u.get()).len() })
}

/// Rend les locales des fonctions enregistrées au-dessus de `depth`.
pub(crate) fn unwind_to(depth: usize) {
    loop {
        let Some((base, words)) = UNWIND.with(|u| unsafe {
            let records = &mut *u.get();
            if records.len() > depth { records.pop() } else { None }
        }) else { break };
        for i in 0..words.max(0) {
            let word = (base + 8 * i) as *mut i64;
            unsafe {
                let val = std::mem::replace(&mut *word, 0);
                release(val);
            }
        }
    }
}

// ── Ressources ouvertes ─────────────────────────────────────────────────────
//
// Ressource `scoped`/`consumed` ouverte : (slot de la variable, fonction de
// fermeture). Retirée à sa fermeture normale ; un `raise` ferme celles des
// frames qu'il saute (`close_resources_to`).

thread_local! {
    static RESOURCES: std::cell::UnsafeCell<Vec<(i64, i64)>> = const { std::cell::UnsafeCell::new(Vec::new()) };
}

#[unsafe(no_mangle)]
pub extern "C" fn __rc_resource_push(slot: i64, closer: i64) {
    RESOURCES.with(|r| unsafe { (*r.get()).push((slot, closer)) });
}

#[unsafe(no_mangle)]
pub extern "C" fn __rc_resource_pop(slot: i64) {
    RESOURCES.with(|r| unsafe {
        let open = &mut *r.get();
        if let Some(i) = open.iter().rposition(|(s, _)| *s == slot) {
            open.remove(i);
        }
    });
}

pub(crate) fn resource_depth() -> usize {
    RESOURCES.with(|r| unsafe { (*r.get()).len() })
}

/// Ferme les ressources ouvertes au-dessus de `depth`, de la plus récente à
/// la plus ancienne.
pub(crate) fn close_resources_to(depth: usize) {
    loop {
        let Some((slot, closer)) = RESOURCES.with(|r| unsafe {
            let open = &mut *r.get();
            if open.len() > depth { open.pop() } else { None }
        }) else { break };
        unsafe {
            let handle = *(slot as *const i64);
            if handle != 0 {
                let close: unsafe extern "C" fn(i64) = std::mem::transmute(closer as usize);
                close(handle);
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __rc_retain(val: i64) {
    unsafe { retain(val) }
}

#[unsafe(no_mangle)]
pub extern "C" fn __rc_release(val: i64) {
    unsafe { release_counted(val) }
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
pub extern "C" fn __rc_mark_raw_kind(val: i64, raw_kind: i64) -> i64 {
    unsafe {
        if matches!(kind(val), Some(Kind::Array | Kind::Map)) {
            set_aux(val, (aux(val) & !RAW_KIND_MASK) | FLAG_RAW | ((raw_kind & 0b11) << RAW_KIND_SHIFT));
        }
    }
    val
}

/// Type des éléments bruts de `val` (1 = int, 2 = float, 3 = bool), 0 si ce
/// n'est pas un conteneur à éléments bruts.
pub(crate) fn raw_kind(val: i64) -> i64 {
    unsafe {
        if !matches!(kind(val), Some(Kind::Array | Kind::Map)) || aux(val) & FLAG_RAW == 0 {
            return 0;
        }
        match (aux(val) & RAW_KIND_MASK) >> RAW_KIND_SHIFT {
            0 => 1,
            k => k,
        }
    }
}

/// Propage le drapeau « éléments bruts » (et leur type) de `src` à `dst`.
pub(crate) fn copy_raw(src: i64, dst: i64) {
    unsafe {
        if aux(src) & FLAG_RAW != 0 {
            set_aux(dst, (aux(dst) & !RAW_KIND_MASK) | FLAG_RAW | (aux(src) & RAW_KIND_MASK));
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __rc_mark_raw(val: i64) -> i64 {
    unsafe {
        if matches!(kind(val), Some(Kind::Array | Kind::Map)) {
            set_aux(val, aux(val) | FLAG_RAW);
        }
    }
    val
}
