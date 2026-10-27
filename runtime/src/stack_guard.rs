// ─────────────────────────────────────────────────────────────────────────────
// Débordement de pile : message au lieu d'un SIGSEGV muet
//
// Le `main` généré appelle `__ocara_runtime_init` (voir
// src/codegen/emit.d/emitter.rs) : une pile alternative et un gestionnaire
// SIGSEGV/SIGBUS affichent « débordement de pile » quand l'adresse fautive
// touche la page de garde du thread courant, « accès mémoire invalide »
// sinon, puis rétablissent l'action par défaut (le processus se termine par
// le même signal qu'avant). Chaque thread lancé par le runtime appelle
// `ThreadStack::enter` pour avoir sa propre pile alternative.
//
// Hors Android (l'environnement ART gère lui-même SIGSEGV) et Windows. Sous
// Windows, `__ocara_runtime_init` note seulement le thread principal (voir
// `blocking::on_main_thread`).
// ─────────────────────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn __ocara_runtime_init() {
    #[cfg(all(unix, not(target_os = "android")))]
    imp::install();
    #[cfg(windows)]
    crate::blocking::record_main_thread();
}

/// Pile alternative du thread courant, rendue à sa fin.
pub(crate) struct ThreadStack {
    #[cfg(all(unix, not(target_os = "android")))]
    alt_stack: Option<imp::AltStack>,
}

impl ThreadStack {
    pub(crate) fn enter() -> Self {
        ThreadStack {
            #[cfg(all(unix, not(target_os = "android")))]
            alt_stack: imp::enter_thread(),
        }
    }
}

#[cfg(all(unix, not(target_os = "android")))]
impl Drop for ThreadStack {
    fn drop(&mut self) {
        if let Some(alt) = self.alt_stack.take() {
            imp::leave_thread(alt);
        }
    }
}

#[cfg(all(unix, not(target_os = "android")))]
mod imp {
    use std::cell::Cell;
    use std::sync::Once;

    const ALT_STACK_SIZE: usize = 64 * 1024;
    /// Zone sous le bas de la pile considérée comme sa garde (l'écart de
    /// garde du noyau vaut 1 Mio par défaut).
    const GUARD_SPAN: usize = 2 * 1024 * 1024;

    thread_local! {
        static STACK_LOW: Cell<usize> = const { Cell::new(0) };
    }

    pub(super) struct AltStack {
        base: *mut libc::c_void,
    }

    static INSTALL: Once = Once::new();

    pub(super) fn install() {
        INSTALL.call_once(|| unsafe {
            // Pile alternative du thread principal : vit jusqu'à la fin du processus.
            std::mem::forget(enter_thread());
            for sig in [libc::SIGSEGV, libc::SIGBUS] {
                let mut action: libc::sigaction = std::mem::zeroed();
                action.sa_sigaction = handler as *const () as usize;
                action.sa_flags = libc::SA_SIGINFO | libc::SA_ONSTACK;
                libc::sigemptyset(&mut action.sa_mask);
                libc::sigaction(sig, &action, std::ptr::null_mut());
            }
        });
    }

    pub(super) fn enter_thread() -> Option<AltStack> {
        unsafe {
            STACK_LOW.with(|low| low.set(current_stack_low()));
            let base = libc::mmap(
                std::ptr::null_mut(),
                ALT_STACK_SIZE,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            );
            if base == libc::MAP_FAILED {
                return None;
            }
            let stack = libc::stack_t { ss_sp: base, ss_flags: 0, ss_size: ALT_STACK_SIZE };
            if libc::sigaltstack(&stack, std::ptr::null_mut()) != 0 {
                libc::munmap(base, ALT_STACK_SIZE);
                return None;
            }
            Some(AltStack { base })
        }
    }

    pub(super) fn leave_thread(alt: AltStack) {
        unsafe {
            let disabled = libc::stack_t { ss_sp: std::ptr::null_mut(), ss_flags: libc::SS_DISABLE, ss_size: 0 };
            libc::sigaltstack(&disabled, std::ptr::null_mut());
            libc::munmap(alt.base, ALT_STACK_SIZE);
        }
    }

    unsafe fn current_stack_low() -> usize {
        unsafe {
            let mut attr: libc::pthread_attr_t = std::mem::zeroed();
            if libc::pthread_getattr_np(libc::pthread_self(), &mut attr) != 0 {
                return 0;
            }
            let mut addr: *mut libc::c_void = std::ptr::null_mut();
            let mut size: libc::size_t = 0;
            let ok = libc::pthread_attr_getstack(&attr, &mut addr, &mut size) == 0;
            libc::pthread_attr_destroy(&mut attr);
            if ok { addr as usize } else { 0 }
        }
    }

    extern "C" fn handler(sig: libc::c_int, info: *mut libc::siginfo_t, _ctx: *mut libc::c_void) {
        unsafe {
            let fault = (*info).si_addr() as usize;
            let low = STACK_LOW.with(|low| low.get());
            let overflow = low != 0 && fault < low + libc::sysconf(libc::_SC_PAGESIZE) as usize && fault + GUARD_SPAN >= low;
            let message = if overflow {
                "\nerreur fatale : débordement de pile (récursion trop profonde ?)\n"
            } else {
                "\nerreur fatale : accès mémoire invalide\n"
            };
            libc::write(2, message.as_ptr() as *const libc::c_void, message.len());
            // Action par défaut rétablie : l'instruction fautive se rejoue et
            // termine le processus par le même signal.
            libc::signal(sig, libc::SIG_DFL);
        }
    }
}
