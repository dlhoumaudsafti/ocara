//! Événements Kotlin → Ocara (`AndroidUI::listen`). Le pont JNI
//! (`OcaraBridge.nativeEvent`) remet chaque événement au récepteur `receive`,
//! qui le dépose dans une file ; un thread Ocara dédié appelle les callbacks
//! l'un après l'autre — jamais sur le thread principal Android, qu'un
//! callback lent gèlerait.

use std::sync::{mpsc, Mutex, OnceLock};

use ocara_runtime_android_common::ffi::{str_arg, string};

unsafe extern "C" {
    /// runtime_android_jni : récepteur des événements de `nativeEvent`.
    fn ocara_android_set_event_sink(sink: extern "C" fn(*const u8, usize, *const u8, usize));
    /// Runtime Ocara (ABI C, voir runtime/src/lib.rs et rc.rs).
    fn __call_closure_catching(fat_ptr: i64, arg: i64) -> i64;
    fn __rc_retain(val: i64);
    fn __rc_release(val: i64);
    fn __rc_thread_enter();
    fn __rc_thread_leave();
}

/// Callbacks par événement (retenus pour toute la vie du programme).
static LISTENERS: Mutex<Vec<(String, i64)>> = Mutex::new(Vec::new());
static QUEUE: OnceLock<Mutex<mpsc::Sender<(String, String)>>> = OnceLock::new();

extern "C" fn receive(event: *const u8, event_len: usize, data: *const u8, data_len: usize) {
    let text = |ptr: *const u8, len: usize| String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(ptr, len) }).into_owned();
    if let Some(queue) = QUEUE.get() {
        let _ = queue.lock().unwrap().send((text(event, event_len), text(data, data_len)));
    }
}

/// Lance le thread des callbacks et branche le récepteur (une fois).
pub(crate) fn start() {
    QUEUE.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<(String, String)>();
        std::thread::spawn(move || {
            for (event, data) in rx {
                dispatch(&event, &data);
            }
        });
        unsafe { ocara_android_set_event_sink(receive) };
        Mutex::new(tx)
    });
}

fn dispatch(event: &str, data: &str) {
    // Messages du moteur d'`ocara.UI` (OcaraUIEngine.kt).
    if event == "__ui" {
        return crate::ui::receive(data);
    }
    let callbacks: Vec<i64> = LISTENERS.lock().unwrap().iter()
        .filter(|(name, _)| name == event)
        .map(|&(_, callback)| callback)
        .collect();
    for callback in callbacks {
        unsafe {
            __rc_thread_enter();
            let arg = string(data);
            let uncaught = __call_closure_catching(callback, arg);
            __rc_release(arg);
            if uncaught != 0 {
                eprintln!("AndroidUI : écouteur de '{event}' : exception non rattrapée ({})", str_arg(uncaught));
                __rc_release(uncaught);
            }
            __rc_thread_leave();
        }
    }
}

/// Enregistre `callback` (`Function<void(string)>`) pour `event`.
pub fn listen(event: &str, callback: i64) {
    start();
    unsafe { __rc_retain(callback) };
    LISTENERS.lock().unwrap().push((event.to_string(), callback));
}
