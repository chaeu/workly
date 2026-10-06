//! Task ids (`WR-12`), project key suggestions and file slugs.

/// Reserved for inbox tasks.
pub const INBOX_KEY: &str = "IN";

/// `"WR-12"` or `"WR-12-navigation.md"` -> `("WR", 12)`.
pub fn split_id(s: &str) -> Option<(&str, u32)> {
    let (key, rest) = s.split_once('-')?;
    let valid_key = key.starts_with(|c: char| c.is_ascii_uppercase()) && key.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    let digits = rest.find(|c: char| !c.is_ascii_digit()).map_or(rest, |n| &rest[..n]);
    if !valid_key || digits.is_empty() {
        return None;
    }
    Some((key, digits.parse().ok()?))
}

/// Highest number for `key` among ids or file names, plus one.
pub fn next_number<'a>(key: &str, names: impl IntoIterator<Item = &'a str>) -> u32 {
    names.into_iter().filter_map(split_id).filter(|(k, _)| *k == key).map(|(_, n)| n).max().unwrap_or(0) + 1
}

/// Key from a title: an uppercase first word is kept, else the initials of up
/// to three words, else the first three letters. Collisions get a digit.
pub fn suggest_key(title: &str, taken: &[&str]) -> String {
    let folded = fold(title);
    let words: Vec<&str> = folded.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let base: String = match words.as_slice() {
        [first, ..] if first.len() >= 2 && first.chars().all(|c| c.is_ascii_uppercase()) => first.chars().take(5).collect(),
        [_, _, ..] => words.iter().take(3).filter_map(|w| w.chars().find(char::is_ascii_alphabetic)).collect(),
        [one] => one.chars().filter(char::is_ascii_alphabetic).take(3).collect(),
        [] => String::new(),
    };
    let base = match base.to_ascii_uppercase() {
        b if b.is_empty() => "P".to_string(),
        b => b,
    };
    let free = |k: &str| k != INBOX_KEY && !taken.contains(&k);
    if free(&base) {
        return base;
    }
    (2..).map(|n| format!("{base}{n}")).find(|k| free(k)).unwrap()
}

/// File slug: lowercase ASCII, words joined by `-`, at most 40 chars.
pub fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in fold(title).chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.truncate(40);
    let out = out.trim_end_matches('-');
    if out.is_empty() { "task".into() } else { out.into() }
}

/// German umlauts to ASCII; other non-ASCII characters pass through.
fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'ä' => out.push_str("ae"),
            'ö' => out.push_str("oe"),
            'ü' => out.push_str("ue"),
            'Ä' => out.push_str("Ae"),
            'Ö' => out.push_str("Oe"),
            'Ü' => out.push_str("Ue"),
            'ß' => out.push_str("ss"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids() {
        assert_eq!(split_id("WR-12"), Some(("WR", 12)));
        assert_eq!(split_id("WR-9-old-banner.md"), Some(("WR", 9)));
        assert_eq!(split_id("WR2-3-x.md"), Some(("WR2", 3)));
        assert_eq!(split_id("_project.md"), None);
        assert_eq!(split_id("wr-1"), None);
        assert_eq!(split_id("WR-x"), None);
        assert_eq!(next_number("WR", ["WR-1", "WR-8-x.md", "IE-20", "WR-9-old.md"]), 10);
        assert_eq!(next_number("ST", ["WR-1"]), 1);
    }

    #[test]
    fn keys() {
        assert_eq!(suggest_key("Website Relaunch", &[]), "WR");
        assert_eq!(suggest_key("CRM cleanup", &[]), "CRM");
        assert_eq!(suggest_key("Support Ticket Triage Bot", &[]), "STT");
        assert_eq!(suggest_key("Newsletter", &[]), "NEW");
        assert_eq!(suggest_key("Über Projekt", &[]), "UP");
        assert_eq!(suggest_key("Website Relaunch", &["WR"]), "WR2");
        assert_eq!(suggest_key("Website Relaunch", &["WR", "WR2"]), "WR3");
        assert_eq!(suggest_key("Internal Notes", &[]), "IN2");
        assert_eq!(suggest_key("!!", &[]), "P");
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("Navigation überarbeiten"), "navigation-ueberarbeiten");
        assert_eq!(slug("Write: content migration plan!"), "write-content-migration-plan");
        assert_eq!(slug("  "), "task");
        assert_eq!(slug(&"a".repeat(50)).len(), 40);
    }
}
