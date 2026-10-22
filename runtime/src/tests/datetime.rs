// `DateTime::format` en une passe : même texte que les `replace` successifs
// qu'il remplace, formats limites compris.

use crate::datetime::DateTime_format;
use crate::*;

/// Ancienne mise en forme : un `replace` par motif.
fn by_replace(ts: i64, fmt: &str) -> String {
    let (y, mo, d, h, mi, s) = civil::parts_from_timestamp(ts);
    fmt.replace("%Y", &format!("{:04}", y))
        .replace("%m", &format!("{:02}", mo))
        .replace("%d", &format!("{:02}", d))
        .replace("%H", &format!("{:02}", h))
        .replace("%M", &format!("{:02}", mi))
        .replace("%S", &format!("{:02}", s))
}

#[test]
fn single_pass_format_matches_successive_replaces() {
    let formats = ["%d/%m/%Y %H:%M:%S", "%%Y", "%", "", "%Y%Y", "a%Zb%", "%m%", "é %Y à %H", "100%", "%%%d"];
    let stamps = [0i64, 1_700_000_000, -1, -86_400 * 800_000, 86_400 * 3_000_000, 951_782_400];
    for fmt in formats {
        for ts in stamps {
            let text = unsafe { alloc_str(fmt) };
            let result = DateTime_format(ts, text);
            assert_eq!(unsafe { ptr_to_str(result) }, by_replace(ts, fmt), "format {:?}, ts {}", fmt, ts);
            unsafe {
                rc::release(result);
                rc::release(text);
            }
        }
    }
}
