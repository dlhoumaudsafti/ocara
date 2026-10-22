//! Allocateur global du runtime. Les petits blocs (≤ 256 octets, une
//! chaîne, une cellule boxée, un petit objet Ocara) sont découpés dans des
//! tranches de 64 Kio et resservis par des listes par thread et par taille,
//! sans repasser par `malloc`/`free` (≈ 20 ns chacun) — comme pymalloc. Le
//! surplus d'une liste passe par lots dans une réserve commune par taille,
//! où puisent les threads dont la liste est vide ; une liste de thread y est
//! versée à sa fin. Un petit bloc n'est jamais rendu au système : la mémoire
//! des petits blocs reste au pic atteint. Les gros blocs vont et viennent de
//! l'allocateur système.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::UnsafeCell;
use std::ptr::null_mut;
use std::sync::Mutex;

const CLASS_SIZE: usize = 16;
const CLASSES: usize = 16;
/// Au-delà, une liste de thread verse `BATCH` blocs dans la réserve commune.
const LOCAL_MAX: usize = 256;
/// Blocs déplacés d'un coup entre une liste de thread et la réserve.
const BATCH: usize = 128;
const SLAB_BYTES: usize = 64 * 1024;

struct Lists {
    heads: [*mut u8; CLASSES],
    counts: [usize; CLASSES],
    /// La garde qui verse les listes à la réserve à la fin du thread est enregistrée.
    guarded: bool,
}

/// Liste chaînée de blocs libres (premier mot : bloc suivant) : la réserve
/// ne doit rien allouer, elle est manipulée sous verrou depuis l'allocateur.
struct Chain {
    head: usize,
    count: usize,
}

static POOL: [Mutex<Chain>; CLASSES] = [const { Mutex::new(Chain { head: 0, count: 0 }) }; CLASSES];

/// Listes du thread, sans destructeur : leur accès ne vérifie aucun état.
struct Cache(UnsafeCell<Lists>);

/// Verse les listes du thread dans la réserve à sa fin.
struct Guard;

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = CACHE.try_with(|cache| unsafe {
            let lists = &mut *cache.0.get();
            for class in 0..CLASSES {
                let count = lists.counts[class];
                give(class, lists, count);
            }
        });
    }
}

thread_local! {
    static CACHE: Cache = const { Cache(UnsafeCell::new(Lists { heads: [null_mut(); CLASSES], counts: [0; CLASSES], guarded: false })) };
    static GUARD: Guard = const { Guard };
}

fn class_of(layout: &Layout) -> Option<usize> {
    (layout.size() <= CLASS_SIZE * CLASSES && layout.align() <= CLASS_SIZE && layout.size() > 0)
        .then(|| (layout.size() - 1) / CLASS_SIZE)
}

fn class_layout(class: usize) -> Layout {
    unsafe { Layout::from_size_align_unchecked((class + 1) * CLASS_SIZE, CLASS_SIZE) }
}

unsafe fn next(block: *mut u8) -> *mut u8 {
    unsafe { *(block as *mut *mut u8) }
}

unsafe fn link(block: *mut u8, next: *mut u8) {
    unsafe { *(block as *mut *mut u8) = next }
}

/// Verse `count` blocs de la liste du thread dans la réserve.
unsafe fn give(class: usize, lists: &mut Lists, count: usize) {
    if count == 0 {
        return;
    }
    let first = lists.heads[class];
    let mut last = first;
    for _ in 1..count {
        last = unsafe { next(last) };
    }
    lists.heads[class] = unsafe { next(last) };
    lists.counts[class] -= count;
    let mut pool = POOL[class].lock().unwrap_or_else(|e| e.into_inner());
    unsafe { link(last, pool.head as *mut u8) };
    pool.head = first as usize;
    pool.count += count;
}

/// Un bloc libre seul (thread en destruction) : directement dans la réserve.
unsafe fn give_one(class: usize, block: *mut u8) {
    let mut pool = POOL[class].lock().unwrap_or_else(|e| e.into_inner());
    unsafe { link(block, pool.head as *mut u8) };
    pool.head = block as usize;
    pool.count += 1;
}

/// Liste du thread vide : un lot de la réserve, sinon une nouvelle tranche.
unsafe fn refill(class: usize, lists: &mut Lists) {
    {
        let mut pool = POOL[class].lock().unwrap_or_else(|e| e.into_inner());
        if pool.count > 0 {
            let take = pool.count.min(BATCH);
            let first = pool.head as *mut u8;
            let mut last = first;
            for _ in 1..take {
                last = unsafe { next(last) };
            }
            pool.head = unsafe { next(last) } as usize;
            pool.count -= take;
            unsafe { link(last, lists.heads[class]) };
            lists.heads[class] = first;
            lists.counts[class] += take;
            return;
        }
    }
    let size = class_layout(class).size();
    let slab = unsafe { System.alloc(Layout::from_size_align_unchecked(SLAB_BYTES, CLASS_SIZE)) };
    if slab.is_null() {
        return;
    }
    for i in (0..SLAB_BYTES / size).rev() {
        let block = unsafe { slab.add(i * size) };
        unsafe { link(block, lists.heads[class]) };
        lists.heads[class] = block;
        lists.counts[class] += 1;
    }
}

pub struct CachingAlloc;

unsafe impl GlobalAlloc for CachingAlloc {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let Some(class) = class_of(&layout) else { return unsafe { System.alloc(layout) } };
        let taken = CACHE.try_with(|cache| unsafe {
            let lists = &mut *cache.0.get();
            if lists.heads[class].is_null() {
                refill(class, lists);
            }
            let block = lists.heads[class];
            if !block.is_null() {
                lists.heads[class] = next(block);
                lists.counts[class] -= 1;
            }
            block
        });
        match taken {
            Ok(block) if !block.is_null() => block,
            // Thread en destruction : bloc système, versé à la réserve à sa libération.
            _ => unsafe { System.alloc(class_layout(class)) },
        }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let Some(class) = class_of(&layout) else { return unsafe { System.dealloc(ptr, layout) } };
        let kept = CACHE.try_with(|cache| unsafe {
            let lists = &mut *cache.0.get();
            if !lists.guarded {
                // Une seule fois par thread ; refusé pendant sa destruction.
                lists.guarded = GUARD.try_with(|_| ()).is_ok();
                if !lists.guarded {
                    return false;
                }
            }
            link(ptr, lists.heads[class]);
            lists.heads[class] = ptr;
            lists.counts[class] += 1;
            if lists.counts[class] > LOCAL_MAX {
                give(class, lists, BATCH);
            }
            true
        });
        if kept != Ok(true) {
            unsafe { give_one(class, ptr) }
        }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if class_of(&layout).is_none() {
            return unsafe { System.alloc_zeroed(layout) };
        }
        let ptr = unsafe { self.alloc(layout) };
        if !ptr.is_null() {
            unsafe { std::ptr::write_bytes(ptr, 0, layout.size()) };
        }
        ptr
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        if class_of(&layout).is_none() && class_of(&new_layout).is_none() {
            return unsafe { System.realloc(ptr, layout, new_size) };
        }
        let fresh = unsafe { self.alloc(new_layout) };
        if !fresh.is_null() {
            unsafe {
                std::ptr::copy_nonoverlapping(ptr, fresh, layout.size().min(new_size));
                self.dealloc(ptr, layout);
            }
        }
        fresh
    }
}

#[global_allocator]
static GLOBAL: CachingAlloc = CachingAlloc;

// Accès direct pour les blocs des valeurs Ocara (`rc`), sans passer par
// l'interface générique de l'allocateur global (non intégrable en ligne).

#[inline]
pub(crate) unsafe fn alloc(layout: Layout) -> *mut u8 {
    unsafe { GLOBAL.alloc(layout) }
}

#[inline]
pub(crate) unsafe fn alloc_zeroed(layout: Layout) -> *mut u8 {
    unsafe { GLOBAL.alloc_zeroed(layout) }
}

#[inline]
pub(crate) unsafe fn dealloc(ptr: *mut u8, layout: Layout) {
    unsafe { GLOBAL.dealloc(ptr, layout) }
}

#[inline]
pub(crate) unsafe fn realloc(ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    unsafe { GLOBAL.realloc(ptr, layout, new_size) }
}
