// Couleurs d'un `.oc.ui` : `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`,
// `rgb(r, g, b)` et `rgba(r, g, b, a)` (composantes 0 à 255, alpha 0 à 1).
// Normalisées à la compilation en `#rrggbb` (opaque) ou `#rrggbbaa` : les
// moteurs de rendu ne lisent que ces deux formes.

/// Forme normalisée de `s`, ou `None` si ce n'est pas une couleur.
pub fn normalize(s: &str) -> Option<String> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let long = match hex.len() {
            3 | 4 => hex.chars().flat_map(|c| [c, c]).collect(),
            6 | 8 => hex.to_string(),
            _ => return None,
        };
        return Some(format!("#{}", long.to_lowercase()));
    }
    let lower = s.to_ascii_lowercase();
    let (args, alpha) = if let Some(rest) = lower.strip_prefix("rgba(") {
        (rest.strip_suffix(')')?, true)
    } else {
        (lower.strip_prefix("rgb(")?.strip_suffix(')')?, false)
    };
    let parts: Vec<&str> = args.split(',').map(str::trim).collect();
    if parts.len() != if alpha { 4 } else { 3 } {
        return None;
    }
    let mut out = String::from("#");
    for part in &parts[..3] {
        let channel: u8 = part.parse().ok()?;
        out.push_str(&format!("{channel:02x}"));
    }
    if alpha {
        let a: f64 = parts[3].parse().ok()?;
        if !(0.0..=1.0).contains(&a) {
            return None;
        }
        if a < 1.0 {
            out.push_str(&format!("{:02x}", (a * 255.0).round() as u8));
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn notations_are_normalized() {
        assert_eq!(normalize("#1565C0").as_deref(), Some("#1565c0"));
        assert_eq!(normalize("#1565c080").as_deref(), Some("#1565c080"));
        assert_eq!(normalize("#fa0").as_deref(), Some("#ffaa00"));
        assert_eq!(normalize("#fa08").as_deref(), Some("#ffaa0088"));
        assert_eq!(normalize("rgb(21, 101, 192)").as_deref(), Some("#1565c0"));
        assert_eq!(normalize("rgba(21,101,192,0.5)").as_deref(), Some("#1565c080"));
        assert_eq!(normalize("rgba(21, 101, 192, 1)").as_deref(), Some("#1565c0"));
        for bad in ["#12345", "#ggg", "rgb(256, 0, 0)", "rgb(1, 2)", "rgba(1, 2, 3, 2)", "red", ""] {
            assert_eq!(normalize(bad), None, "{bad}");
        }
    }
}
