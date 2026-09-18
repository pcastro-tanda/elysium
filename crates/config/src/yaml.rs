//! Generic YAML values, an insertion-ordered mapping, and a Psych-compatible
//! emitter.
//!
//! RuboCop treats configuration as untyped nested hashes, so this crate keeps
//! the same shape instead of deserializing into structs: unknown cop
//! parameters must survive a load/merge/`--show-cops` round trip untouched.
//! Insertion order is preserved because `rubocop --show-cops` prints each cop's
//! parameters in the order `config/default.yml` declares them.

use std::collections::HashMap;
use std::fmt::Write as _;

use saphyr::{LoadableYamlNode, Scalar, Yaml};

/// A YAML value as RuboCop's configuration hashes see it.
///
/// `Regexp` holds the source text of a `!ruby/regexp` scalar; RuboCop permits
/// that class in configuration (`Psych.safe_load(permitted_classes: [Regexp,
/// Symbol])`). Symbols are folded into [`YamlValue::String`], matching how
/// RuboCop compares them (`to_sym`/`to_s` at the use site).
#[derive(Debug, Clone, PartialEq)]
pub enum YamlValue {
    /// `null` / `~` / an empty value.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// An integer scalar.
    Int(i64),
    /// A floating point scalar.
    Float(f64),
    /// A string scalar.
    String(String),
    /// A sequence.
    Array(Vec<YamlValue>),
    /// A mapping, in document order.
    Mapping(Mapping),
    /// A `!ruby/regexp` scalar, kept as its source text (e.g. `/foo/`).
    Regexp(String),
}

/// An insertion-ordered string-keyed mapping with O(1) lookup.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mapping {
    entries: Vec<(String, YamlValue)>,
    index: HashMap<String, usize>,
}

impl Mapping {
    /// An empty mapping.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when there are no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Looks up a key.
    pub fn get(&self, key: &str) -> Option<&YamlValue> {
        self.index.get(key).map(|&i| &self.entries[i].1)
    }

    /// Looks up a key for mutation.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut YamlValue> {
        let i = *self.index.get(key)?;
        Some(&mut self.entries[i].1)
    }

    /// True when the key is present.
    pub fn contains_key(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    /// Inserts or replaces a value, keeping the original position on replace.
    pub fn insert(&mut self, key: impl Into<String>, value: YamlValue) {
        let key = key.into();
        if let Some(&i) = self.index.get(&key) {
            self.entries[i].1 = value;
        } else {
            self.index.insert(key.clone(), self.entries.len());
            self.entries.push((key, value));
        }
    }

    /// Removes a key, returning its value.
    pub fn remove(&mut self, key: &str) -> Option<YamlValue> {
        let i = self.index.remove(key)?;
        let (_, value) = self.entries.remove(i);
        for slot in self.index.values_mut() {
            if *slot > i {
                *slot -= 1;
            }
        }
        Some(value)
    }

    /// Entries in document order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &YamlValue)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Keys in document order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(k, _)| k.as_str())
    }

    /// Nested mapping at `key`, if the value is a mapping.
    pub fn get_mapping(&self, key: &str) -> Option<&Mapping> {
        match self.get(key) {
            Some(YamlValue::Mapping(m)) => Some(m),
            _ => None,
        }
    }

    /// Nested mapping at `key` for mutation.
    pub fn get_mapping_mut(&mut self, key: &str) -> Option<&mut Mapping> {
        match self.get_mut(key) {
            Some(YamlValue::Mapping(m)) => Some(m),
            _ => None,
        }
    }

    /// Boolean at `key`, tolerating YAML 1.1 spellings (`yes`, `off`, ...).
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(YamlValue::as_bool)
    }

    /// String at `key`.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(YamlValue::as_str)
    }

    /// The strings of the sequence at `key`; a bare string yields one element.
    pub fn get_string_list(&self, key: &str) -> Vec<String> {
        self.get(key).map(YamlValue::to_string_list).unwrap_or_default()
    }

    #[must_use]
    /// Ruby's `Hash#merge`: `other`'s values win, `self`'s order is kept and
    /// keys new to `other` are appended.
    pub fn merged_with(&self, other: &Mapping) -> Mapping {
        let mut out = self.clone();
        for (k, v) in other.iter() {
            out.insert(k, v.clone());
        }
        out
    }
}

impl YamlValue {
    /// The value as a string, if it is a string scalar.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            YamlValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// The value as a boolean.
    ///
    /// YAML 1.1 boolean spellings (`yes`, `no`, `on`, `off`) are accepted the
    /// way Psych accepts them; saphyr is a YAML 1.2 parser and leaves them as
    /// strings.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            YamlValue::Bool(b) => Some(*b),
            YamlValue::String(s) => match s.as_str() {
                "yes" | "Yes" | "YES" | "on" | "On" | "ON" | "true" | "True" | "TRUE" => Some(true),
                "no" | "No" | "NO" | "off" | "Off" | "OFF" | "false" | "False" | "FALSE" => {
                    Some(false)
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// The value as a float, accepting integers and numeric strings the way
    /// `String#to_f` does for `TargetRubyVersion: '3.1'`.
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            #[allow(clippy::cast_possible_truncation)]
            YamlValue::Float(f) => Some(*f as f32),
            #[allow(clippy::cast_precision_loss)]
            YamlValue::Int(i) => Some(*i as f32),
            YamlValue::String(s) => s.parse().ok(),
            _ => None,
        }
    }

    /// The sequence elements, if this is a sequence.
    pub fn as_array(&self) -> Option<&[YamlValue]> {
        match self {
            YamlValue::Array(a) => Some(a),
            _ => None,
        }
    }

    /// The mapping, if this is a mapping.
    pub fn as_mapping(&self) -> Option<&Mapping> {
        match self {
            YamlValue::Mapping(m) => Some(m),
            _ => None,
        }
    }

    /// True for `null`.
    pub fn is_null(&self) -> bool {
        matches!(self, YamlValue::Null)
    }

    /// Ruby truthiness: everything but `nil` and `false`.
    pub fn is_truthy(&self) -> bool {
        !matches!(self, YamlValue::Null | YamlValue::Bool(false))
    }

    /// Ruby's `Array(value)` followed by string coercion: sequences yield their
    /// string elements, a bare scalar yields a single element.
    pub fn to_string_list(&self) -> Vec<String> {
        match self {
            YamlValue::Array(items) => items.iter().filter_map(Self::scalar_text).collect(),
            YamlValue::Null => Vec::new(),
            other => other.scalar_text().into_iter().collect(),
        }
    }

    fn scalar_text(&self) -> Option<String> {
        match self {
            YamlValue::String(s) | YamlValue::Regexp(s) => Some(s.clone()),
            YamlValue::Int(i) => Some(i.to_string()),
            YamlValue::Float(f) => Some(f.to_string()),
            YamlValue::Bool(b) => Some(b.to_string()),
            _ => None,
        }
    }

    /// Ruby's `Array(value)` used by `inherit_mode` unions.
    pub fn to_array(&self) -> Vec<YamlValue> {
        match self {
            YamlValue::Array(items) => items.clone(),
            YamlValue::Null => Vec::new(),
            other => vec![other.clone()],
        }
    }
}

/// Parses the first document of a YAML string.
///
/// Returns `Ok(None)` for an empty document (RuboCop treats that as `{}`), and
/// `Err(message)` for a scan error or a disallowed Ruby class tag.
pub(crate) fn parse_document(source: &str) -> Result<Option<YamlValue>, String> {
    let docs = Yaml::load_from_str(source).map_err(|e| e.to_string())?;
    match docs.into_iter().next() {
        None | Some(Yaml::BadValue) => Ok(None),
        Some(doc) => convert(&doc).map(Some),
    }
}

fn convert(node: &Yaml<'_>) -> Result<YamlValue, String> {
    match node {
        Yaml::Value(scalar) => Ok(convert_scalar(scalar)),
        Yaml::Representation(text, _, _) => Ok(YamlValue::String(text.to_string())),
        Yaml::Sequence(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(convert(item)?);
            }
            Ok(YamlValue::Array(out))
        }
        Yaml::Mapping(map) => {
            let mut out = Mapping::new();
            for (key, value) in map {
                let key = key_text(key);
                if key == "<<" {
                    merge_key(&mut out, value)?;
                } else {
                    out.insert(key, convert(value)?);
                }
            }
            Ok(YamlValue::Mapping(out))
        }
        Yaml::Tagged(tag, inner) => {
            let suffix = tag.suffix.as_str();
            match suffix {
                "ruby/regexp" => Ok(YamlValue::Regexp(match convert(inner)? {
                    YamlValue::String(s) => s,
                    other => format!("{other:?}"),
                })),
                "ruby/symbol" => convert(inner),
                _ => Err(format!("disallowed class {suffix}")),
            }
        }
        Yaml::Alias(_) | Yaml::BadValue => Ok(YamlValue::Null),
    }
}

fn merge_key(out: &mut Mapping, value: &Yaml<'_>) -> Result<(), String> {
    let sources: Vec<&Yaml<'_>> = match value {
        Yaml::Sequence(items) => items.iter().collect(),
        other => vec![other],
    };
    for source in sources {
        let YamlValue::Mapping(map) = convert(source)? else {
            continue;
        };
        for (k, v) in map.iter() {
            if !out.contains_key(k) {
                out.insert(k, v.clone());
            }
        }
    }
    Ok(())
}

fn convert_scalar(scalar: &Scalar<'_>) -> YamlValue {
    match scalar {
        Scalar::Null => YamlValue::Null,
        Scalar::Boolean(b) => YamlValue::Bool(*b),
        Scalar::Integer(i) => YamlValue::Int(*i),
        Scalar::FloatingPoint(f) => YamlValue::Float(f.into_inner()),
        Scalar::String(s) => YamlValue::String(s.to_string()),
    }
}

fn key_text(key: &Yaml<'_>) -> String {
    match key {
        Yaml::Value(Scalar::String(s)) => s.to_string(),
        Yaml::Value(Scalar::Boolean(b)) => b.to_string(),
        Yaml::Value(Scalar::Integer(i)) => i.to_string(),
        Yaml::Value(Scalar::FloatingPoint(f)) => f.into_inner().to_string(),
        Yaml::Representation(text, _, _) => text.to_string(),
        Yaml::Value(Scalar::Null) => String::new(),
        other => format!("{other:?}"),
    }
}

/// Emits `mapping` the way Psych's `to_yaml` does for RuboCop's
/// `--show-cops` output: block style, sequences at the indentation of their
/// key, plain scalars unless quoting is required.
pub(crate) fn emit_mapping(mapping: &Mapping, indent: usize, out: &mut String) {
    for (key, value) in mapping.iter() {
        emit_entry(key, value, indent, out);
    }
}

fn emit_entry(key: &str, value: &YamlValue, indent: usize, out: &mut String) {
    let pad = " ".repeat(indent);
    match value {
        YamlValue::Mapping(m) if !m.is_empty() => {
            let _ = writeln!(out, "{pad}{}:", quote_key(key));
            emit_mapping(m, indent + 2, out);
        }
        YamlValue::Mapping(_) => {
            let _ = writeln!(out, "{pad}{}: {{}}", quote_key(key));
        }
        YamlValue::Array(items) if items.is_empty() => {
            let _ = writeln!(out, "{pad}{}: []", quote_key(key));
        }
        YamlValue::Array(items) => {
            let _ = writeln!(out, "{pad}{}:", quote_key(key));
            for item in items {
                emit_sequence_item(item, indent, out);
            }
        }
        scalar => {
            let _ = writeln!(out, "{pad}{}: {}", quote_key(key), emit_scalar(scalar));
        }
    }
}

fn emit_sequence_item(item: &YamlValue, indent: usize, out: &mut String) {
    let pad = " ".repeat(indent);
    match item {
        YamlValue::Mapping(m) if !m.is_empty() => {
            let mut nested = String::new();
            emit_mapping(m, indent + 2, &mut nested);
            let body = nested.trim_start_matches(' ');
            let _ = write!(out, "{pad}- {body}");
        }
        YamlValue::Array(items) if !items.is_empty() => {
            let _ = writeln!(out, "{pad}-");
            for nested in items {
                emit_sequence_item(nested, indent + 2, out);
            }
        }
        scalar => {
            let _ = writeln!(out, "{pad}- {}", emit_scalar(scalar));
        }
    }
}

fn emit_scalar(value: &YamlValue) -> String {
    match value {
        YamlValue::Null => "~".to_string(),
        YamlValue::Bool(b) => b.to_string(),
        YamlValue::Int(i) => i.to_string(),
        YamlValue::Float(f) if f.is_nan() => ".nan".to_string(),
        YamlValue::Float(f) if f.is_infinite() => {
            if *f > 0.0 {
                ".inf".to_string()
            } else {
                "-.inf".to_string()
            }
        }
        YamlValue::Float(f) => {
            if f.fract() == 0.0 {
                format!("{f:.1}")
            } else {
                f.to_string()
            }
        }
        YamlValue::String(s) => quote_scalar(s),
        YamlValue::Regexp(s) => format!("!ruby/regexp {s}"),
        YamlValue::Array(_) | YamlValue::Mapping(_) => String::new(),
    }
}

fn quote_key(key: &str) -> String {
    quote_scalar(key)
}

fn quote_scalar(text: &str) -> String {
    if needs_double_quotes(text) {
        let mut out = String::with_capacity(text.len() + 2);
        out.push('"');
        for ch in text.chars() {
            match ch {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\t' => out.push_str("\\t"),
                '\r' => out.push_str("\\r"),
                other => out.push(other),
            }
        }
        out.push('"');
        out
    } else if needs_single_quotes(text) {
        format!("'{}'", text.replace('\'', "''"))
    } else {
        text.to_string()
    }
}

fn needs_double_quotes(text: &str) -> bool {
    text.chars().any(|c| c == '\n' || c == '\t' || c == '\r' || c.is_control())
}

fn needs_single_quotes(text: &str) -> bool {
    if text.is_empty() {
        return true;
    }
    let first = text.chars().next().unwrap_or(' ');
    // `:sym` is a legal plain scalar and is how Psych dumps Ruby symbols, so
    // keeping it unquoted makes `--show-cops` round trip to the same symbols.
    let symbol_like = first == ':' && text.len() > 1 && !text[1..].starts_with(' ');
    if !symbol_like && "-?:,[]{}#&*!|>'\"%@`".contains(first) {
        return true;
    }
    if text.starts_with(' ') || text.ends_with(' ') {
        return true;
    }
    if text.contains(": ") || text.contains(" #") || text.ends_with(':') {
        return true;
    }
    // Plain scalars that YAML would re-read as another type must be quoted.
    if text.parse::<i64>().is_ok() || text.parse::<f64>().is_ok() {
        return true;
    }
    matches!(
        text,
        "~" | "null"
            | "Null"
            | "NULL"
            | "true"
            | "True"
            | "TRUE"
            | "false"
            | "False"
            | "FALSE"
            | "yes"
            | "Yes"
            | "YES"
            | "no"
            | "No"
            | "NO"
            | "on"
            | "On"
            | "ON"
            | "off"
            | "Off"
            | "OFF"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(source: &str) -> Mapping {
        match parse_document(source).unwrap().unwrap() {
            YamlValue::Mapping(m) => m,
            other => panic!("not a mapping: {other:?}"),
        }
    }

    #[test]
    fn preserves_document_order() {
        let m = map("b: 1\na: 2\nc: 3\n");
        assert_eq!(m.keys().collect::<Vec<_>>(), ["b", "a", "c"]);
    }

    #[test]
    fn resolves_anchors_and_merge_keys() {
        // spec/rubocop/config_loader_spec.rb:1911 "allows yaml anchors"
        let m = map("Style/Alias: &anchor\n  Enabled: false\nStyle/Encoding:\n  <<: *anchor\n");
        assert_eq!(m.get_mapping("Style/Encoding").unwrap().get_bool("Enabled"), Some(false));
    }

    #[test]
    fn merge_key_does_not_clobber_explicit_keys() {
        let m = map("a: &a\n  x: 1\n  y: 2\nb:\n  <<: *a\n  x: 9\n");
        let b = m.get_mapping("b").unwrap();
        assert_eq!(b.get("x"), Some(&YamlValue::Int(9)));
        assert_eq!(b.get("y"), Some(&YamlValue::Int(2)));
    }

    #[test]
    fn rejects_disallowed_ruby_classes() {
        // spec/rubocop/config_loader_spec.rb:1901 "rejects non-allowed yaml types"
        let err = parse_document("foo: !ruby/object:Rational\n  numerator: 1\n").unwrap_err();
        assert!(err.contains("Rational"), "{err}");
    }

    #[test]
    fn keeps_ruby_regexp_source() {
        let m = map("AllCops:\n  Exclude:\n    - !ruby/regexp /foo/\n");
        let excludes = m.get_mapping("AllCops").unwrap().get("Exclude").unwrap();
        assert_eq!(excludes.as_array().unwrap(), [YamlValue::Regexp("/foo/".to_string())]);
    }

    #[test]
    fn empty_document_is_none() {
        assert!(parse_document("").unwrap().is_none());
        assert!(parse_document("# only a comment\n").unwrap().is_none());
    }

    #[test]
    fn yaml_one_one_booleans_are_accepted() {
        let m = map("Style/Alias:\n  Enabled: no\n");
        assert_eq!(m.get_mapping("Style/Alias").unwrap().get_bool("Enabled"), Some(false));
    }

    #[test]
    fn emits_sequences_at_key_indentation() {
        let m = map("Exclude:\n  - foo.rb\n  - 'bar baz.rb'\nEnabled: true\n");
        let mut out = String::new();
        emit_mapping(&m, 2, &mut out);
        assert_eq!(out, "  Exclude:\n  - foo.rb\n  - bar baz.rb\n  Enabled: true\n");
    }

    #[test]
    fn emitted_yaml_round_trips() {
        let m = map(concat!(
            "Description: 'Use `alias` instead: it is nicer.'\n",
            "Enabled: pending\n",
            "Max: 120\n",
            "Ratio: 0.5\n",
            "Empty: []\n",
            "Nested:\n  Inner:\n    - 1\n    - two\n",
        ));
        let mut out = String::new();
        emit_mapping(&m, 0, &mut out);
        let reparsed = map(&out);
        assert_eq!(reparsed, m, "emitted:\n{out}");
    }

    #[test]
    fn remove_keeps_index_consistent() {
        let mut m = map("a: 1\nb: 2\nc: 3\n");
        m.remove("a");
        assert_eq!(m.get("c"), Some(&YamlValue::Int(3)));
        assert_eq!(m.keys().collect::<Vec<_>>(), ["b", "c"]);
    }
}
