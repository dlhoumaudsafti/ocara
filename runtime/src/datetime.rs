// ─────────────────────────────────────────────────────────────────────────────
// ocara.DateTime — runtime des fonctions de date/heure
//
// Fonctions exportées (convention C) :
//
//   DateTime_now()                     → i64      timestamp Unix actuel
//   DateTime_from_timestamp(ts)        → i64      convertit timestamp en ISO 8601
//   DateTime_year(ts)                  → i64      extrait l'année
//   DateTime_month(ts)                 → i64      extrait le mois (1-12)
//   DateTime_day(ts)                   → i64      extrait le jour (1-31)
//   DateTime_hour(ts)                  → i64      extrait l'heure (0-23)
//   DateTime_minute(ts)                → i64      extrait les minutes (0-59)
//   DateTime_second(ts)                → i64      extrait les secondes (0-59)
//   DateTime_format(ts, fmt)           → i64      formate selon pattern
//   DateTime_parse(s)                  → i64      parse ISO 8601 → timestamp
// ─────────────────────────────────────────────────────────────────────────────

use std::ffi::CStr;
use std::time::{SystemTime, UNIX_EPOCH};

/// Retourne le timestamp Unix actuel (secondes depuis 1970-01-01 00:00:00 UTC)
#[no_mangle]
pub extern "C" fn DateTime_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// Convertit un timestamp en string ISO 8601 (YYYY-MM-DDTHH:MM:SS)
#[no_mangle]
pub extern "C" fn DateTime_from_timestamp(ts: i64) -> i64 {
    let (year, month, day, hour, minute, second) = timestamp_to_parts(ts);
    let result = format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
                         year, month, day, hour, minute, second);
    unsafe { crate::alloc_str(&result) }
}

/// Extrait l'année d'un timestamp
#[no_mangle]
pub extern "C" fn DateTime_year(ts: i64) -> i64 {
    timestamp_to_parts(ts).0 as i64
}

/// Extrait le mois d'un timestamp (1-12)
#[no_mangle]
pub extern "C" fn DateTime_month(ts: i64) -> i64 {
    timestamp_to_parts(ts).1 as i64
}

/// Extrait le jour d'un timestamp (1-31)
#[no_mangle]
pub extern "C" fn DateTime_day(ts: i64) -> i64 {
    timestamp_to_parts(ts).2 as i64
}

/// Extrait l'heure d'un timestamp (0-23)
#[no_mangle]
pub extern "C" fn DateTime_hour(ts: i64) -> i64 {
    timestamp_to_parts(ts).3 as i64
}

/// Extrait les minutes d'un timestamp (0-59)
#[no_mangle]
pub extern "C" fn DateTime_minute(ts: i64) -> i64 {
    timestamp_to_parts(ts).4 as i64
}

/// Extrait les secondes d'un timestamp (0-59)
#[no_mangle]
pub extern "C" fn DateTime_second(ts: i64) -> i64 {
    timestamp_to_parts(ts).5 as i64
}

/// Formate un timestamp selon un pattern
/// Patterns supportés : %Y (année), %m (mois), %d (jour), %H (heure), %M (minute), %S (seconde)
#[no_mangle]
pub extern "C" fn DateTime_format(ts: i64, fmt: i64) -> i64 {
    let fmt_str = unsafe { CStr::from_ptr(fmt as *const i8) }.to_str().unwrap_or("");
    let (year, month, day, hour, minute, second) = timestamp_to_parts(ts);
    
    let result = fmt_str
        .replace("%Y", &format!("{:04}", year))
        .replace("%m", &format!("{:02}", month))
        .replace("%d", &format!("{:02}", day))
        .replace("%H", &format!("{:02}", hour))
        .replace("%M", &format!("{:02}", minute))
        .replace("%S", &format!("{:02}", second));
    
    unsafe { crate::alloc_str(&result) }
}

/// Parse une chaîne ISO 8601 en timestamp
/// Format attendu : YYYY-MM-DDTHH:MM:SS ou YYYY-MM-DD HH:MM:SS
#[no_mangle]
pub extern "C" fn DateTime_parse(s: i64) -> i64 {
    let s_str = unsafe { CStr::from_ptr(s as *const i8) }.to_str().unwrap_or("");
    
    // Parser YYYY-MM-DDTHH:MM:SS ou YYYY-MM-DD HH:MM:SS
    let parts: Vec<&str> = s_str.split(&['T', ' '][..]).collect();
    if parts.len() != 2 {
        unsafe {
            crate::exception::throw_datetime_exception(
                &format!("Invalid datetime format: '{}' (expected YYYY-MM-DDTHH:MM:SS or YYYY-MM-DD HH:MM:SS)", s_str),
                101
            );
        }
    }
    
    let date_parts: Vec<&str> = parts[0].split('-').collect();
    let time_parts: Vec<&str> = parts[1].split(':').collect();
    
    if date_parts.len() != 3 || time_parts.len() != 3 {
        unsafe {
            crate::exception::throw_datetime_exception(
                &format!("Invalid datetime format: '{}' (expected YYYY-MM-DDTHH:MM:SS or YYYY-MM-DD HH:MM:SS)", s_str),
                101
            );
        }
    }
    
    let year = match date_parts[0].parse::<i32>() {
        Ok(y) => y,
        Err(_) => unsafe {
            crate::exception::throw_datetime_exception(
                &format!("Invalid year in datetime: '{}'", s_str),
                101
            );
        }
    };
    let month = match date_parts[1].parse::<i32>() {
        Ok(m) if m >= 1 && m <= 12 => m,
        _ => unsafe {
            crate::exception::throw_datetime_exception(
                &format!("Invalid month in datetime: '{}' (must be 1-12)", s_str),
                101
            );
        }
    };
    let day = match date_parts[2].parse::<i32>() {
        Ok(d) if d >= 1 && d <= 31 => d,
        _ => unsafe {
            crate::exception::throw_datetime_exception(
                &format!("Invalid day in datetime: '{}' (must be 1-31)", s_str),
                101
            );
        }
    };
    let hour = match time_parts[0].parse::<i32>() {
        Ok(h) if h >= 0 && h <= 23 => h,
        _ => unsafe {
            crate::exception::throw_datetime_exception(
                &format!("Invalid hour in datetime: '{}' (must be 0-23)", s_str),
                101
            );
        }
    };
    let minute = match time_parts[1].parse::<i32>() {
        Ok(m) if m >= 0 && m <= 59 => m,
        _ => unsafe {
            crate::exception::throw_datetime_exception(
                &format!("Invalid minute in datetime: '{}' (must be 0-59)", s_str),
                101
            );
        }
    };
    let second = match time_parts[2].parse::<i32>() {
        Ok(s) if s >= 0 && s <= 59 => s,
        _ => unsafe {
            crate::exception::throw_datetime_exception(
                &format!("Invalid second in datetime: '{}' (must be 0-59)", s_str),
                101
            );
        }
    };
    
    parts_to_timestamp(year, month, day, hour, minute, second)
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers internes
// ─────────────────────────────────────────────────────────────────────────────

/// Convertit un timestamp Unix en composants (année, mois, jour, heure, minute, seconde)
fn timestamp_to_parts(ts: i64) -> (i32, i32, i32, i32, i32, i32) {
    let mut remaining = ts;
    
    // Extraire l'heure, minute, seconde
    let second = (remaining % 60) as i32;
    remaining /= 60;
    let minute = (remaining % 60) as i32;
    remaining /= 60;
    let hour = (remaining % 24) as i32;
    remaining /= 24;
    
    // remaining = nombre de jours depuis epoch
    let mut days = remaining as i32;
    
    // Calculer l'année (approximation puis ajustement)
    let mut year = 1970;
    loop {
        let year_days = if is_leap_year(year) { 366 } else { 365 };
        if days < year_days {
            break;
        }
        days -= year_days;
        year += 1;
    }
    
    // Calculer le mois et le jour
    let mut month = 1;
    let months_days = days_in_months(year);
    for (m, &m_days) in months_days.iter().enumerate() {
        if days < m_days {
            month = (m + 1) as i32;
            break;
        }
        days -= m_days;
    }
    
    let day = days + 1;
    
    (year, month, day, hour, minute, second)
}

/// Convertit des composants en timestamp Unix
fn parts_to_timestamp(year: i32, month: i32, day: i32, hour: i32, minute: i32, second: i32) -> i64 {
    // Calculer les jours depuis epoch
    let mut days = 0i64;
    
    // Ajouter les années complètes
    for y in 1970..year {
        days += if is_leap_year(y) { 366 } else { 365 };
    }
    
    // Ajouter les mois de l'année courante
    let months_days = days_in_months(year);
    for m in 0..(month - 1) as usize {
        days += months_days[m] as i64;
    }
    
    // Ajouter les jours
    days += (day - 1) as i64;
    
    // Convertir en secondes
    let mut ts = days * 86400;
    ts += hour as i64 * 3600;
    ts += minute as i64 * 60;
    ts += second as i64;
    
    ts
}

/// Retourne true si l'année est bissextile
fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Retourne le nombre de jours dans chaque mois pour une année donnée
fn days_in_months(year: i32) -> [i32; 12] {
    let feb_days = if is_leap_year(year) { 29 } else { 28 };
    [31, feb_days, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
}
