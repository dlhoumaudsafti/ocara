// ─────────────────────────────────────────────────────────────────────────────
// Styles de nommage (R07/R08/R09/R12/R14) : vérification et conversion
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Style {
    Snake,
    Camel,
    Pascal,
    UpperSnake,
    Upper,
}

impl Style {
    pub fn parse(value: &str) -> Option<Style> {
        match value {
            "snake_case"       => Some(Style::Snake),
            "camelCase"        => Some(Style::Camel),
            "PascalCase"       => Some(Style::Pascal),
            "UPPER_SNAKE_CASE" => Some(Style::UpperSnake),
            "UPPERCASE"        => Some(Style::Upper),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Style::Snake      => "snake_case",
            Style::Camel      => "camelCase",
            Style::Pascal     => "PascalCase",
            Style::UpperSnake => "UPPER_SNAKE_CASE",
            Style::Upper      => "UPPERCASE",
        }
    }

    pub fn matches(self, name: &str) -> bool {
        let mut chars = name.chars();
        let Some(first) = chars.next() else { return false };
        let rest_ok = |pred: fn(char) -> bool| name.chars().all(pred);
        match self {
            Style::Snake => (first.is_ascii_lowercase() || first == '_')
                && rest_ok(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
            Style::Camel  => first.is_ascii_lowercase() && rest_ok(|c| c.is_ascii_alphanumeric()),
            Style::Pascal => first.is_ascii_uppercase() && rest_ok(|c| c.is_ascii_alphanumeric()),
            Style::UpperSnake => first != '_'
                && rest_ok(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'),
            Style::Upper => first.is_ascii_uppercase()
                && rest_ok(|c| c.is_ascii_uppercase() || c.is_ascii_digit()),
        }
    }

    /// `name` réécrit dans ce style (suggestion affichée avec l'avertissement).
    pub fn convert(self, name: &str) -> String {
        let words = words(name);
        match self {
            Style::Snake      => words.join("_"),
            Style::UpperSnake => words.join("_").to_uppercase(),
            Style::Upper      => words.concat().to_uppercase(),
            Style::Pascal     => words.iter().map(|w| capitalize(w)).collect(),
            Style::Camel      => words.iter().enumerate()
                .map(|(i, w)| if i == 0 { w.clone() } else { capitalize(w) })
                .collect(),
        }
    }
}

/// Mots d'un identifiant, en minuscules : `HTTPServer_max2Retry` → http, server, max2, retry.
fn words(name: &str) -> Vec<String> {
    let chars: Vec<char> = name.chars().collect();
    let mut words = Vec::new();
    let mut current = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c == '_' {
            if !current.is_empty() { words.push(std::mem::take(&mut current)); }
            continue;
        }
        let prev = i.checked_sub(1).map(|p| chars[p]);
        let next = chars.get(i + 1).copied();
        let after_lower = prev.is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit());
        let acronym_end = prev.is_some_and(|p| p.is_ascii_uppercase()) && next.is_some_and(|n| n.is_ascii_lowercase());
        if c.is_ascii_uppercase() && !current.is_empty() && (after_lower || acronym_end) {
            words.push(std::mem::take(&mut current));
        }
        current.push(c.to_ascii_lowercase());
    }
    if !current.is_empty() { words.push(current); }
    words
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map(|c| c.to_ascii_uppercase().to_string() + chars.as_str()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::Style;

    #[test]
    fn matches_each_style() {
        assert!(Style::Snake.matches("user_count") && !Style::Snake.matches("userCount"));
        assert!(Style::Camel.matches("userCount") && !Style::Camel.matches("user_count"));
        assert!(Style::Pascal.matches("UserCount") && !Style::Pascal.matches("User_Count"));
        assert!(Style::UpperSnake.matches("MAX_RETRY") && !Style::UpperSnake.matches("maxRetry"));
        assert!(Style::Upper.matches("MAXRETRY") && !Style::Upper.matches("MAX_RETRY"));
    }

    #[test]
    fn converts_between_styles() {
        assert_eq!(Style::Camel.convert("is_adult"), "isAdult");
        assert_eq!(Style::Snake.convert("HTTPServerSession"), "http_server_session");
        assert_eq!(Style::Pascal.convert("max_retry"), "MaxRetry");
        assert_eq!(Style::UpperSnake.convert("maxRetry"), "MAX_RETRY");
        assert_eq!(Style::Upper.convert("max_retry"), "MAXRETRY");
    }
}
