// ─────────────────────────────────────────────────────────────────────────────
// Calendrier civil (grégorien proleptique), partagé par `Date`/`DateTime`.
//
// Algorithmes de Howard Hinnant (« chrono-compatible low-level date
// algorithms ») : justes pour toute date, avant 1970 comprise. Les anciennes
// boucles d'années partaient de 1970 et tronquaient les négatifs
// (`fromTimestamp(-1)` → `1970-01-01T00:00:-1`).
// ─────────────────────────────────────────────────────────────────────────────

/// Jours depuis le 1970-01-01 du jour civil `(year, month, day)`.
pub(crate) fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Jour civil `(year, month, day)` à `days` jours du 1970-01-01.
pub(crate) fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

/// Timestamp Unix → (année, mois, jour, heure, minute, seconde).
pub(crate) fn parts_from_timestamp(ts: i64) -> (i64, i64, i64, i64, i64, i64) {
    let (year, month, day) = civil_from_days(ts.div_euclid(86_400));
    let secs = ts.rem_euclid(86_400);
    (year, month, day, secs / 3600, secs % 3600 / 60, secs % 60)
}

/// (année, mois, jour, heure, minute, seconde) → timestamp Unix.
pub(crate) fn timestamp_from_parts(year: i64, month: i64, day: i64, hour: i64, minute: i64, second: i64) -> i64 {
    days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second
}

pub(crate) fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Nombre de jours du mois (0 si le mois est hors de 1..12).
pub(crate) fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}
