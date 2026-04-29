// ─────────────────────────────────────────────────────────────────────────────
// ocara.Time — runtime des fonctions de temps
//
// Fonctions exportées (convention C) :
//
//   Time_now()                   → i64      heure actuelle (HH:MM:SS)
//   Time_from_timestamp(ts)      → i64      extrait l'heure d'un timestamp (HH:MM:SS)
//   Time_hour(time)              → i64      extrait l'heure (0-23)
//   Time_minute(time)            → i64      extrait les minutes (0-59)
//   Time_second(time)            → i64      extrait les secondes (0-59)
//   Time_from_seconds(seconds)   → i64      convertit secondes → HH:MM:SS
//   Time_to_seconds(time)        → i64      convertit HH:MM:SS → secondes
//   Time_add_seconds(time, s)    → i64      ajoute N secondes
//   Time_diff_seconds(t1, t2)    → i64      différence en secondes
// ─────────────────────────────────────────────────────────────────────────────

use std::ffi::CStr;
use std::time::{SystemTime, UNIX_EPOCH};

/// Retourne l'heure actuelle au format HH:MM:SS
#[no_mangle]
pub extern "C" fn Time_now() -> i64 {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    Time_from_timestamp(ts)
}

/// Extrait l'heure d'un timestamp au format HH:MM:SS
#[no_mangle]
pub extern "C" fn Time_from_timestamp(ts: i64) -> i64 {
    let seconds_in_day = ts % 86400;
    let hour = (seconds_in_day / 3600) as i32;
    let minute = ((seconds_in_day % 3600) / 60) as i32;
    let second = (seconds_in_day % 60) as i32;
    
    let result = format!("{:02}:{:02}:{:02}", hour, minute, second);
    unsafe { crate::alloc_str(&result) }
}

/// Extrait l'heure d'un time HH:MM:SS (0-23)
#[no_mangle]
pub extern "C" fn Time_hour(time: i64) -> i64 {
    let time_str = unsafe { CStr::from_ptr(time as *const i8) }.to_str().unwrap_or("");
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() < 3 {
        unsafe {
            crate::exception::throw_time_exception(
                &format!("Invalid time format: '{}' (expected HH:MM:SS)", time_str),
                101
            );
        }
    }
    match parts[0].parse::<i64>() {
        Ok(h) if h >= 0 && h <= 23 => h,
        _ => unsafe {
            crate::exception::throw_time_exception(
                &format!("Invalid hour in time: '{}' (must be 0-23)", time_str),
                101
            );
        }
    }
}

/// Extrait les minutes d'un time HH:MM:SS (0-59)
#[no_mangle]
pub extern "C" fn Time_minute(time: i64) -> i64 {
    let time_str = unsafe { CStr::from_ptr(time as *const i8) }.to_str().unwrap_or("");
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() < 3 {
        unsafe {
            crate::exception::throw_time_exception(
                &format!("Invalid time format: '{}' (expected HH:MM:SS)", time_str),
                101
            );
        }
    }
    match parts[1].parse::<i64>() {
        Ok(m) if m >= 0 && m <= 59 => m,
        _ => unsafe {
            crate::exception::throw_time_exception(
                &format!("Invalid minute in time: '{}' (must be 0-59)", time_str),
                101
            );
        }
    }
}

/// Extrait les secondes d'un time HH:MM:SS (0-59)
#[no_mangle]
pub extern "C" fn Time_second(time: i64) -> i64 {
    let time_str = unsafe { CStr::from_ptr(time as *const i8) }.to_str().unwrap_or("");
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() < 3 {
        unsafe {
            crate::exception::throw_time_exception(
                &format!("Invalid time format: '{}' (expected HH:MM:SS)", time_str),
                101
            );
        }
    }
    match parts[2].parse::<i64>() {
        Ok(s) if s >= 0 && s <= 59 => s,
        _ => unsafe {
            crate::exception::throw_time_exception(
                &format!("Invalid second in time: '{}' (must be 0-59)", time_str),
                101
            );
        }
    }
}

/// Convertit un nombre de secondes en HH:MM:SS
#[no_mangle]
pub extern "C" fn Time_from_seconds(seconds: i64) -> i64 {
    let s = seconds % 86400; // Garder dans la journée
    let hour = (s / 3600) as i32;
    let minute = ((s % 3600) / 60) as i32;
    let second = (s % 60) as i32;
    
    let result = format!("{:02}:{:02}:{:02}", hour, minute, second);
    unsafe { crate::alloc_str(&result) }
}

/// Convertit un time HH:MM:SS en nombre de secondes depuis minuit
#[no_mangle]
pub extern "C" fn Time_to_seconds(time: i64) -> i64 {
    let hour = Time_hour(time);
    let minute = Time_minute(time);
    let second = Time_second(time);
    
    hour * 3600 + minute * 60 + second
}

/// Ajoute N secondes à un time
#[no_mangle]
pub extern "C" fn Time_add_seconds(time: i64, s: i64) -> i64 {
    let current_seconds = Time_to_seconds(time);
    let new_seconds = (current_seconds + s) % 86400;
    Time_from_seconds(if new_seconds < 0 { new_seconds + 86400 } else { new_seconds })
}

/// Calcule la différence en secondes entre deux times
#[no_mangle]
pub extern "C" fn Time_diff_seconds(t1: i64, t2: i64) -> i64 {
    let s1 = Time_to_seconds(t1);
    let s2 = Time_to_seconds(t2);
    s1 - s2
}
