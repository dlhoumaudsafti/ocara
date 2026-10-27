//! Un relevé de capteur : file d'événements sur l'`ALooper` du thread
//! appelant, activée le temps d'un événement.

use std::ffi::c_void;
use std::time::{Duration, Instant};

use ocara_runtime_android_common::{Failure, ERR_ANDROID, ERR_DEVICE, ERR_PERMISSION, ERR_TIMEOUT};

#[repr(C)]
struct ASensorEvent {
    version: i32,
    sensor: i32,
    kind: i32,
    reserved0: i32,
    timestamp: i64,
    data: [f32; 16],
    flags: u32,
    reserved1: [i32; 3],
}

const ALOOPER_PREPARE_ALLOW_NON_CALLBACKS: i32 = 1;
const STEP_COUNTER: i32 = 19;
const QUEUE_IDENT: i32 = 1;
const TIMEOUT: Duration = Duration::from_secs(2);
/// 50 Hz : au-delà de 200 Hz, Android 12+ exige `HIGH_SAMPLING_RATE_SENSORS`
/// (`enableSensor` prend la fréquence maximale du capteur et échoue alors).
const PERIOD_US: i32 = 20_000;

#[link(name = "android")]
unsafe extern "C" {
    fn ASensorManager_getInstance() -> *mut c_void;
    fn ASensorManager_getDefaultSensor(manager: *mut c_void, kind: i32) -> *const c_void;
    fn ASensorManager_createEventQueue(manager: *mut c_void, looper: *mut c_void, ident: i32, callback: *const c_void, data: *mut c_void) -> *mut c_void;
    fn ASensorManager_destroyEventQueue(manager: *mut c_void, queue: *mut c_void) -> i32;
    fn ASensorEventQueue_enableSensor(queue: *mut c_void, sensor: *const c_void) -> i32;
    fn ASensorEventQueue_setEventRate(queue: *mut c_void, sensor: *const c_void, period_us: i32) -> i32;
    fn ASensorEventQueue_disableSensor(queue: *mut c_void, sensor: *const c_void) -> i32;
    fn ASensorEventQueue_getEvents(queue: *mut c_void, events: *mut ASensorEvent, count: usize) -> isize;
    fn ALooper_prepare(opts: i32) -> *mut c_void;
    fn ALooper_pollOnce(timeout_ms: i32, fd: *mut i32, events: *mut i32, data: *mut *mut c_void) -> i32;
}

unsafe extern "C" {
    fn dlsym(handle: *mut c_void, symbol: *const std::ffi::c_char) -> *mut c_void;
}

/// `RTLD_DEFAULT` de Bionic en 64 bits.
const RTLD_DEFAULT: *mut c_void = std::ptr::null_mut();

type RegisterSensor = unsafe extern "C" fn(*mut c_void, *const c_void, i32, i64) -> i32;

/// Active `sensor` à `PERIOD_US`. `ASensorEventQueue_registerSensor` (API 26)
/// est cherché à l'exécution : lié en dur, il empêcherait de charger le `.so`
/// sur Android 7 ; avant l'API 26, `enableSensor` (fréquence maximale) puis
/// `setEventRate`.
unsafe fn register(queue: *mut c_void, sensor: *const c_void) -> bool {
    unsafe {
        let symbol = dlsym(RTLD_DEFAULT, c"ASensorEventQueue_registerSensor".as_ptr());
        if !symbol.is_null() {
            let register: RegisterSensor = std::mem::transmute(symbol);
            return register(queue, sensor, PERIOD_US, 0) >= 0;
        }
        ASensorEventQueue_enableSensor(queue, sensor) >= 0 && ASensorEventQueue_setEventRate(queue, sensor, PERIOD_US) >= 0
    }
}

fn default_sensor(kind: i32) -> (*mut c_void, *const c_void) {
    unsafe {
        let manager = ASensorManager_getInstance();
        let sensor = if manager.is_null() { std::ptr::null() } else { ASensorManager_getDefaultSensor(manager, kind) };
        (manager, sensor)
    }
}

pub fn available(kind: i32) -> bool {
    !default_sensor(kind).1.is_null()
}

pub fn read(kind: i32, count: usize) -> Result<Vec<f64>, Failure> {
    let (manager, sensor) = default_sensor(kind);
    if sensor.is_null() {
        return Err(Failure::new(ERR_DEVICE, "capteur absent de l'appareil"));
    }
    unsafe {
        let looper = ALooper_prepare(ALOOPER_PREPARE_ALLOW_NON_CALLBACKS);
        let queue = ASensorManager_createEventQueue(manager, looper, QUEUE_IDENT, std::ptr::null(), std::ptr::null_mut());
        if queue.is_null() {
            return Err(Failure::new(ERR_ANDROID, "file d'événements de capteur impossible à créer"));
        }
        let result = if !register(queue, sensor) {
            Err(refused(kind))
        } else {
            let event = next_event(queue, kind);
            ASensorEventQueue_disableSensor(queue, sensor);
            event.map(|e| e.data[..count].iter().map(|&v| v as f64).collect())
        };
        ASensorManager_destroyEventQueue(manager, queue);
        result
    }
}

unsafe fn next_event(queue: *mut c_void, kind: i32) -> Result<ASensorEvent, Failure> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(Failure::new(ERR_TIMEOUT, "aucun relevé du capteur en 2 s"));
        }
        unsafe {
            ALooper_pollOnce(left.as_millis() as i32, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut());
            let mut event: ASensorEvent = std::mem::zeroed();
            while ASensorEventQueue_getEvents(queue, &mut event, 1) > 0 {
                if event.kind == kind {
                    return Ok(event);
                }
            }
        }
    }
}

/// Activation refusée : le compteur de pas exige `ACTIVITY_RECOGNITION`
/// (Android 10 et plus).
fn refused(kind: i32) -> Failure {
    if kind == STEP_COUNTER {
        return Failure::new(ERR_PERMISSION, "compteur de pas : permission ACTIVITY_RECOGNITION requise (AndroidPermissions::request(\"activity_recognition\"))");
    }
    Failure::new(ERR_ANDROID, "capteur impossible à activer")
}
