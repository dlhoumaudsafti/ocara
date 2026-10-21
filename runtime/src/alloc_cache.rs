//! Allocateur global du runtime : les petits blocs (≤ 256 octets) libérés
//! sont gardés dans des listes par thread et par taille, et resservis sans
//! repasser par `malloc`/`free` — une chaîne, une cellule boxée ou un petit
//! objet Ocara coûtait deux appels à l'allocateur système (≈ 20 ns chacun).
//! Au-delà d'un plafond par liste, ou pour un gros bloc, le bloc va et vient
//! de l'allocateur système. Les listes d'un thread lui sont rendues à sa fin.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::UnsafeCell;
use std::ptr::null_mut;

const CLASS_SIZE: usize = 16;
const CLASSES: usize = 16;
/// Blocs gardés au plus par liste (32 Kio pour la plus grande classe).
const CAP: usize = 128;

struct Lists {
    heads: [*mut u8; CLASSES],
    counts: [usize; CLASSES],
    /// La garde qui rend les listes à la fin du thread est enregistrée.
    guarded: bool,
}

/// Listes du thread, sans destructeur : leur accès ne vérifie aucun état.
struct Cache(UnsafeCell<Lists>);

/// Rend les listes du thread à l'allocateur système à sa fin.
struct Guard;

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = CACHE.try_with(|cache| unsafe { release_lists(&mut *cache.0.get()) });
    }
}

unsafe fn release_lists(lists: &mut Lists) {
    {
        for class in 0..CLASSES {
            let layout = class_layout(class);
            let mut block = lists.heads[class];
            while !block.is_null() {
                let next = unsafe { *(block as *mut *mut u8) };
                unsafe { System.dealloc(block, layout) };
                block = next;
            }
            lists.heads[class] = null_mut();
            lists.counts[class] = 0;
        }
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

pub struct CachingAlloc;

unsafe impl GlobalAlloc for CachingAlloc {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let Some(class) = class_of(&layout) else { return unsafe { System.alloc(layout) } };
        let reused = CACHE.try_with(|cache| unsafe {
            let lists = &mut *cache.0.get();
            let block = lists.heads[class];
            if !block.is_null() {
                lists.heads[class] = *(block as *mut *mut u8);
                lists.counts[class] -= 1;
            }
            block
        });
        match reused {
            Ok(block) if !block.is_null() => block,
            _ => unsafe { System.alloc(class_layout(class)) },
        }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let Some(class) = class_of(&layout) else { return unsafe { System.dealloc(ptr, layout) } };
        let kept = CACHE.try_with(|cache| unsafe {
            let lists = &mut *cache.0.get();
            if lists.counts[class] >= CAP {
                return false;
            }
            if !lists.guarded {
                // Une seule fois par thread ; refusé pendant sa destruction.
                lists.guarded = GUARD.try_with(|_| ()).is_ok();
                if !lists.guarded {
                    return false;
                }
            }
            *(ptr as *mut *mut u8) = lists.heads[class];
            lists.heads[class] = ptr;
            lists.counts[class] += 1;
            true
        });
        if kept != Ok(true) {
            unsafe { System.dealloc(ptr, class_layout(class)) }
        }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
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
