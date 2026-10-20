// ─────────────────────────────────────────────────────────────────────────────
// ocara.Date — runtime des fonctions de date
//
// Fonctions exportées (convention C) :
//
//   Date_today()                      → i64      date actuelle (YYYY-MM-DD)
//   Date_fromTimestamp(ts)           → i64      convertit timestamp → YYYY-MM-DD
//   Date_year(date)                   → i64      extrait l'année
//   Date_month(date)                  → i64      extrait le mois (1-12)
//   Date_day(date)                    → i64      extrait le jour (1-31)
//   Date_dayOfWeek(date)            → i64      jour de la semaine (0=lundi, 6=dimanche)
//   Date_isLeapYear(year)           → i64      année bissextile ? (1=oui, 0=non)
//   Date_daysInMonth(year, month)   → i64      nombre de jours dans le mois
//   Date_addDays(date, days)         → i64      ajoute N jours
//   Date_diffDays(date1, date2)      → i64      différence en jours
// ─────────────────────────────────────────────────────────────────────────────

use std::time::{SystemTime, UNIX_EPOCH};

/// Retourne la date actuelle au format YYYY-MM-DD
#[unsafe(no_mangle)]
pub extern "C" fn Date_today() -> i64 {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    Date_fromTimestamp(ts)
}

/// Convertit un timestamp en string YYYY-MM-DD
#[unsafe(no_mangle)]
pub extern "C" fn Date_fromTimestamp(ts: i64) -> i64 {
    let (year, month, day) = timestamp_to_date(ts);
    let result = format!("{:04}-{:02}-{:02}", year, month, day);
    unsafe { crate::alloc_str(&result) }
}

/// Extrait l'année d'une date YYYY-MM-DD
#[unsafe(no_mangle)]
pub extern "C" fn Date_year(date: i64) -> i64 {
    let date_str = unsafe { crate::ptr_to_str(date) };
    let parts: Vec<&str> = date_str.split('-').collect();
    if parts.len() < 3 {
        unsafe {
            crate::exception::throw_date_exception(
                &format!("Invalid date format: '{}' (expected YYYY-MM-DD)", date_str),
                101
            );
        }
    }
    match parts[0].parse::<i64>() {
        Ok(y) => y,
        Err(_) => unsafe {
            crate::exception::throw_date_exception(
                &format!("Invalid year in date: '{}'", date_str),
                101
            );
        }
    }
}

/// Extrait le mois d'une date YYYY-MM-DD (1-12)
#[unsafe(no_mangle)]
pub extern "C" fn Date_month(date: i64) -> i64 {
    let date_str = unsafe { crate::ptr_to_str(date) };
    let parts: Vec<&str> = date_str.split('-').collect();
    if parts.len() < 3 {
        unsafe {
            crate::exception::throw_date_exception(
                &format!("Invalid date format: '{}' (expected YYYY-MM-DD)", date_str),
                101
            );
        }
    }
    match parts[1].parse::<i64>() {
        Ok(m) if m >= 1 && m <= 12 => m,
        _ => unsafe {
            crate::exception::throw_date_exception(
                &format!("Invalid month in date: '{}' (must be 1-12)", date_str),
                101
            );
        }
    }
}

/// Extrait le jour d'une date YYYY-MM-DD (1-31)
#[unsafe(no_mangle)]
pub extern "C" fn Date_day(date: i64) -> i64 {
    let date_str = unsafe { crate::ptr_to_str(date) };
    let parts: Vec<&str> = date_str.split('-').collect();
    if parts.len() < 3 {
        unsafe {
            crate::exception::throw_date_exception(
                &format!("Invalid date format: '{}' (expected YYYY-MM-DD)", date_str),
                101
            );
        }
    }
    // Jour borné par la longueur réelle du mois (`2023-02-31` refusé).
    let max_day = match (parts[0].parse::<i64>(), parts[1].parse::<i64>()) {
        (Ok(y), Ok(m)) if (1..=12).contains(&m) => crate::civil::days_in_month(y, m),
        _ => 31,
    };
    match parts[2].parse::<i64>() {
        Ok(d) if d >= 1 && d <= max_day => d,
        _ => unsafe {
            crate::exception::throw_date_exception(
                &format!("Invalid day in date: '{}' (must be 1-{})", date_str, max_day),
                101
            );
        }
    }
}

/// Retourne le jour de la semaine (0=lundi, 6=dimanche)
#[unsafe(no_mangle)]
pub extern "C" fn Date_dayOfWeek(date: i64) -> i64 {
    let days = crate::civil::days_from_civil(Date_year(date), Date_month(date), Date_day(date));
    // 1970-01-01 était un jeudi (3 avec 0 = lundi) ; division euclidienne :
    // juste aussi avant 1970.
    (days + 3).rem_euclid(7)
}

/// Retourne 1 si l'année est bissextile, 0 sinon
#[unsafe(no_mangle)]
pub extern "C" fn Date_isLeapYear(year: i64) -> i64 {
    if crate::civil::is_leap_year(year) { 1 } else { 0 }
}

/// Retourne le nombre de jours dans un mois donné
#[unsafe(no_mangle)]
pub extern "C" fn Date_daysInMonth(year: i64, month: i64) -> i64 {
    crate::civil::days_in_month(year, month)
}

/// Ajoute N jours à une date
#[unsafe(no_mangle)]
pub extern "C" fn Date_addDays(date: i64, days: i64) -> i64 {
    let year = Date_year(date) as i32;
    let month = Date_month(date) as i32;
    let day = Date_day(date) as i32;
    
    // Convertir en timestamp
    let ts = date_to_timestamp(year, month, day);
    // Ajouter les jours (86400 secondes par jour)
    let new_ts = ts + (days * 86400);
    
    Date_fromTimestamp(new_ts)
}

/// Calcule la différence en jours entre deux dates
#[unsafe(no_mangle)]
pub extern "C" fn Date_diffDays(date1: i64, date2: i64) -> i64 {
    let y1 = Date_year(date1) as i32;
    let m1 = Date_month(date1) as i32;
    let d1 = Date_day(date1) as i32;
    
    let y2 = Date_year(date2) as i32;
    let m2 = Date_month(date2) as i32;
    let d2 = Date_day(date2) as i32;
    
    let ts1 = date_to_timestamp(y1, m1, d1);
    let ts2 = date_to_timestamp(y2, m2, d2);
    
    (ts1 - ts2) / 86400
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers internes
// ─────────────────────────────────────────────────────────────────────────────

fn timestamp_to_date(ts: i64) -> (i32, i32, i32) {
    let (year, month, day) = crate::civil::civil_from_days(ts.div_euclid(86_400));
    (year as i32, month as i32, day as i32)
}

fn date_to_timestamp(year: i32, month: i32, day: i32) -> i64 {
    crate::civil::days_from_civil(year as i64, month as i64, day as i64) * 86_400
}
