//! The slice of `ActiveSupport::Inflector` rubocop-rails leans on:
//! `String#tableize` with the default English inflections
//! (`active_support/inflections.rb`).

use std::sync::LazyLock;

use regex::Regex;

/// `(pattern, replacement)` plural rules in application order, which is
/// the reverse of definition order (`Inflections#plural` prepends).
static PLURALS: LazyLock<Vec<(Regex, String)>> = LazyLock::new(|| {
    // Irregulars (`person`/`people`, ...) all share their first letter, so
    // each defines the two rules `(s0)srest$` and `(p0)prest$`, the second
    // checked first; later irregulars take precedence over earlier ones.
    let irregulars = [
        ("person", "people"),
        ("man", "men"),
        ("child", "children"),
        ("sex", "sexes"),
        ("move", "moves"),
    ];
    let mut rules: Vec<(String, String)> = Vec::new();
    for (singular, plural) in irregulars.iter().rev() {
        let (s0, srest) = singular.split_at(1);
        let (p0, prest) = plural.split_at(1);
        rules.push((format!("(?i)({p0}){prest}$"), format!("${{1}}{prest}")));
        rules.push((format!("(?i)({s0}){srest}$"), format!("${{1}}{prest}")));
    }
    let base: [(&str, &str); 21] = [
        ("(?i)(quiz)$", "${1}zes"),
        ("(?i)^(oxen)$", "${1}"),
        ("(?i)^(ox)$", "${1}en"),
        ("(?i)^(m|l)ice$", "${1}ice"),
        ("(?i)^(m|l)ouse$", "${1}ice"),
        ("(?i)(matr|vert|ind)(?:ix|ex)$", "${1}ices"),
        ("(?i)(x|ch|ss|sh)$", "${1}es"),
        ("(?i)([^aeiouy]|qu)y$", "${1}ies"),
        ("(?i)(hive)$", "${1}s"),
        ("(?i)(?:([^f])fe|([lr])f)$", "${1}${2}ves"),
        ("(?i)sis$", "ses"),
        ("(?i)([ti])a$", "${1}a"),
        ("(?i)([ti])um$", "${1}a"),
        ("(?i)(buffal|tomat)o$", "${1}oes"),
        ("(?i)(bu)s$", "${1}ses"),
        ("(?i)(alias|status)$", "${1}es"),
        ("(?i)(octop|vir)i$", "${1}i"),
        ("(?i)(octop|vir)us$", "${1}i"),
        ("(?i)^(ax|test)is$", "${1}es"),
        ("(?i)s$", "s"),
        ("(?i)$", "s"),
    ];
    rules.extend(base.iter().map(|(p, r)| ((*p).to_string(), (*r).to_string())));
    rules
        .into_iter()
        .map(|(pattern, replacement)| (Regex::new(&pattern).expect("inflection rule"), replacement))
        .collect()
});

static UNCOUNTABLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:equipment|information|rice|money|species|series|fish|sheep|jeans|police)\z",
    )
    .expect("uncountable pattern")
});

/// `String#underscore` (no acronyms configured).
#[must_use]
pub fn underscore(word: &str) -> String {
    let word = word.replace("::", "/");
    let chars: Vec<char> = word.chars().collect();
    let mut out = String::with_capacity(word.len() + 4);
    for (index, &c) in chars.iter().enumerate() {
        if index > 0 && c.is_ascii_uppercase() {
            let previous = chars[index - 1];
            let next_lower = chars.get(index + 1).is_some_and(char::is_ascii_lowercase);
            if (previous.is_ascii_uppercase() && next_lower)
                || previous.is_ascii_lowercase()
                || previous.is_ascii_digit()
            {
                out.push('_');
            }
        }
        out.push(if c == '-' { '_' } else { c.to_ascii_lowercase() });
    }
    out
}

/// `String#pluralize` in English.
#[must_use]
pub fn pluralize(word: &str) -> String {
    if word.is_empty() || UNCOUNTABLE.is_match(word) {
        return word.to_string();
    }
    for (rule, replacement) in PLURALS.iter() {
        if rule.is_match(word) {
            return rule.replace(word, replacement.as_str()).into_owned();
        }
    }
    word.to_string()
}

/// `String#tableize`.
#[must_use]
pub fn tableize(class_name: &str) -> String {
    pluralize(&underscore(class_name))
}

#[cfg(test)]
mod tests {
    use super::tableize;

    #[test]
    fn tableizes_like_active_support() {
        for (class, table) in [
            ("User", "users"),
            ("Admin_User", "admin_users"),
            ("WrittenArticles", "written_articles"),
            ("Person", "people"),
            ("Category", "categories"),
            ("Box", "boxes"),
            ("Status", "statuses"),
            ("Company_Fish", "company_fishes"),
            ("Equipment", "equipment"),
            ("HTMLPage", "html_pages"),
            ("Wolf", "wolves"),
            ("Day", "days"),
            ("Quiz", "quizzes"),
            ("Address", "addresses"),
            ("Analysis", "analyses"),
        ] {
            assert_eq!(tableize(class), table, "{class}");
        }
    }
}
