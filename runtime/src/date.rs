// ─────────────────────────────────────────────────────────────────────────────
// ocara.Date — runtime des fonctions de date
//
// Fonctions exportées (convention C) :
//
//   Date_today()                      → i64      date actuelle (YYYY-MM-DD)
//   Date_from_timestamp(ts)           → i64      convertit timestamp → YYYY-MM-DD
//   Date_year(date)                   → i64      extrait l'année
//   Date_month(date)                  → i64      extrait le mois (1-12)
//   Date_day(date)                    → i64      extrait le jour (1-31)
//   Date_day_of_week(date)            → i64      jour de la semaine (0=lundi, 6=dimanche)
//   Date_is_leap_year(year)           → i64      année bissextile ? (1=oui, 0=non)
//   Date_days_in_month(year, month)   → i64      nombre de jours dans le mois
//   Date_add_days(date, days)         → i64      ajoute N jours
//   Date_diff_days(date1, date2)      → i64      différence en jours
// ─────────────────────────────────────────────────────────────────────────────

use std::ffi::CStr;
use std::time::{SystemTime, UNIX_EPOCH};

/// Retourne la date actuelle au format YYYY-MM-DD
#[no_mangle]
pub extern "C" fn Date_today() -> i64 {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    Date_from_timestamp(ts)
}

/// Convertit un timestamp en string YYYY-MM-DD
#[no_mangle]
pub extern "C" fn Date_from_timestamp(ts: i64) -> i64 {
    let (year, month, day) = timestamp_to_date(ts);
    let result = format!("{:04}-{:02}-{:02}", year, month, day);
    unsafe { crate::alloc_str(&result) }
}

/// Extrait l'année d'une date YYYY-MM-DD
#[no_mangle]
pub extern "C" fn Date_year(date: i64) -> i64 {
    let date_str = unsafe { CStr::from_ptr(date as *const i8) }.to_str().unwrap_or("");
    let parts: Vec<&str> = date_str.split('-').collect();
    if parts.len() >= 1 {
        parts[0].parse::<i64>().unwrap_or(0)
    } else {
        0
    }
}

/// Extrait le mois d'une date YYYY-MM-DD (1-12)
#[no_mangle]
pub extern "C" fn Date_month(date: i64) -> i64 {
    let date_str = unsafe { CStr::from_ptr(date as *const i8) }.to_str().unwrap_or("");
    let parts: Vec<&str> = date_str.split('-').collect();
    if parts.len() >= 2 {
        parts[1].parse::<i64>().unwrap_or(0)
    } else {
        0
    }
}

/// Extrait le jour d'une date YYYY-MM-DD (1-31)
#[no_mangle]
pub extern "C" fn Date_day(date: i64) -> i64 {
    let date_str = unsafe { CStr::from_ptr(date as *const i8) }.to_str().unwrap_or("");
    let parts: Vec<&str> = date_str.split('-').collect();
    if parts.len() >= 3 {
        parts[2].parse::<i64>().unwrap_or(0)
    } else {
        0
    }
}

/// Retourne le jour de la semaine (0=lundi, 6=dimanche)
#[no_mangle]
pub extern "C" fn Date_day_of_week(date: i64) -> i64 {
    let year = Date_year(date) as i32;
    let month = Date_month(date) as i32;
    let day = Date_day(date) as i32;
    
    // Calcul du nombre de jours depuis epoch
    let mut days = 0i64;
    for y in 1970..year {
        days += if is_leap_year(y) { 366 } else { 365 };
    }
    let months_days = days_in_months(year);
    for m in 0..(month - 1) as usize {
        days += months_days[m] as i64;
    }
    days += (day - 1) as i64;
    
    // 1970-01-01 était un jeudi (epoch + 3 jours = lundi)
    ((days + 3) % 7) as i64
}

/// Retourne 1 si l'année est bissextile, 0 sinon
#[no_mangle]
pub extern "C" fn Date_is_leap_year(year: i64) -> i64 {
    if is_leap_year(year as i32) { 1 } else { 0 }
}

/// Retourne le nombre de jours dans un mois donné
#[no_mangle]
pub extern "C" fn Date_days_in_month(year: i64, month: i64) -> i64 {
    let months = days_in_months(year as i32);
    if month >= 1 && month <= 12 {
        months[(month - 1) as usize] as i64
    } else {
        0
    }
}

/// Ajoute N jours à une date
#[no_mangle]
pub extern "C" fn Date_add_days(date: i64, days: i64) -> i64 {
    let year = Date_year(date) as i32;
    let month = Date_month(date) as i32;
    let day = Date_day(date) as i32;
    
    // Convertir en timestamp
    let ts = date_to_timestamp(year, month, day);
    // Ajouter les jours (86400 secondes par jour)
    let new_ts = ts + (days * 86400);
    
    Date_from_timestamp(new_ts)
}

/// Calcule la différence en jours entre deux dates
#[no_mangle]
pub extern "C" fn Date_diff_days(date1: i64, date2: i64) -> i64 {
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
    let mut days = (ts / 86400) as i32;
    
    let mut year = 1970;
    loop {
        let year_days = if is_leap_year(year) { 366 } else { 365 };
        if days < year_days {
            break;
        }
        days -= year_days;
        year += 1;
    }
    
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
    (year, month, day)
}

fn date_to_timestamp(year: i32, month: i32, day: i32) -> i64 {
    let mut days = 0i64;
    
    for y in 1970..year {
        days += if is_leap_year(y) { 366 } else { 365 };
    }
    
    let months_days = days_in_months(year);
    for m in 0..(month - 1) as usize {
        days += months_days[m] as i64;
    }
    
    days += (day - 1) as i64;
    days * 86400
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn days_in_months(year: i32) -> [i32; 12] {
    let feb_days = if is_leap_year(year) { 29 } else { 28 };
    [31, feb_days, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
}
