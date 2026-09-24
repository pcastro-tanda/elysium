//! Configured option values handed to [`crate::Rule::configure`].
//!
//! Values mirror RuboCop's YAML types exactly, so a rule reads the same
//! option names and shapes RuboCop documents. Everything here is plain data
//! (no YAML crate types leak in), keeping the rule contract marshallable.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::rule::{ConfigDefault, RuleMeta};

/// One configuration value, in RuboCop's YAML shapes.
#[derive(Debug, Clone, PartialEq)]
pub enum OptionValue {
    /// YAML `null`/`~`, or an explicitly empty value.
    Null,
    /// YAML boolean.
    Bool(bool),
    /// YAML integer.
    Int(i64),
    /// YAML float.
    Float(f64),
    /// YAML string (also used for RuboCop's `!ruby/regexp` values).
    Str(String),
    /// YAML sequence.
    List(Vec<OptionValue>),
    /// YAML mapping, in declaration order.
    Map(Vec<(String, OptionValue)>),
}

impl OptionValue {
    /// The value as a string, for `Str` values only.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            OptionValue::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// The value as a boolean, for `Bool` values only.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            OptionValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The value as an integer. Floats are truncated, matching RuboCop's
    /// tolerance for `Max: 80.0`.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            OptionValue::Int(i) => Some(*i),
            #[allow(clippy::cast_possible_truncation)]
            OptionValue::Float(f) => Some(*f as i64),
            _ => None,
        }
    }

    /// The value as a float. Integers convert.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            OptionValue::Float(f) => Some(*f),
            #[allow(clippy::cast_precision_loss)]
            OptionValue::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// The value as a list. A scalar counts as a one-element list, matching
    /// RuboCop's `Array(value)` handling of single-entry option lists.
    pub fn as_list(&self) -> Option<&[OptionValue]> {
        match self {
            OptionValue::List(items) => Some(items.as_slice()),
            _ => None,
        }
    }

    /// The value as a mapping.
    pub fn as_map(&self) -> Option<&[(String, OptionValue)]> {
        match self {
            OptionValue::Map(entries) => Some(entries.as_slice()),
            _ => None,
        }
    }

    /// Every string in the value: the strings of a list, or the value
    /// itself when it is a lone string.
    pub fn to_string_list(&self) -> Vec<String> {
        match self {
            OptionValue::Str(s) => vec![s.clone()],
            OptionValue::List(items) => {
                items.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()
            }
            _ => Vec::new(),
        }
    }
}

/// An unusable option value: RuboCop's "unsupported style ... for
/// `EnforcedStyle`" class of configuration error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionError {
    /// Cop the option belongs to.
    pub rule: &'static str,
    /// Option name.
    pub option: String,
    /// Human-readable explanation.
    pub message: String,
}

impl fmt::Display for OptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}: {}", self.rule, self.option, self.message)
    }
}

impl std::error::Error for OptionError {}

/// Every cop's configured options, so a rule can read another cop's
/// settings (`Layout/IndentationWidth: Width`).
pub type PeerOptions = BTreeMap<String, Vec<(String, OptionValue)>>;

/// The options one rule was configured with.
#[derive(Debug, Clone)]
pub struct RuleOptions {
    meta: &'static RuleMeta,
    own: Vec<(String, OptionValue)>,
    peers: Arc<PeerOptions>,
}

impl RuleOptions {
    /// Only the rule's `META.config` defaults.
    pub fn defaults(meta: &'static RuleMeta) -> Self {
        Self { meta, own: Vec::new(), peers: Arc::new(PeerOptions::new()) }
    }

    /// Configured values for this rule plus every other cop's options.
    pub fn new(
        meta: &'static RuleMeta,
        own: Vec<(String, OptionValue)>,
        peers: Arc<PeerOptions>,
    ) -> Self {
        Self { meta, own, peers }
    }

    /// The rule these options belong to.
    pub fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    /// The configured value of `key`, with no default fallback.
    pub fn get(&self, key: &str) -> Option<&OptionValue> {
        self.own.iter().find(|(name, _)| name == key).map(|(_, value)| value)
    }

    /// The schema default of `key`, if the rule declares one.
    fn default_of(&self, key: &str) -> Option<&'static ConfigDefault> {
        self.meta.config.iter().find(|opt| opt.name == key).map(|opt| &opt.default)
    }

    /// Configured boolean, else the schema default, else `false`.
    pub fn bool(&self, key: &str) -> bool {
        if let Some(value) = self.get(key).and_then(OptionValue::as_bool) {
            return value;
        }
        matches!(self.default_of(key), Some(ConfigDefault::Bool(true)))
    }

    /// Configured integer, else the schema default, else `0`.
    pub fn int(&self, key: &str) -> i64 {
        if let Some(value) = self.get(key).and_then(OptionValue::as_int) {
            return value;
        }
        match self.default_of(key) {
            Some(ConfigDefault::Int(i)) => *i,
            #[allow(clippy::cast_possible_truncation)]
            Some(ConfigDefault::Float(f)) => *f as i64,
            _ => 0,
        }
    }

    /// Configured string, else the schema default, else the empty string.
    pub fn str(&self, key: &str) -> Cow<'_, str> {
        if let Some(value) = self.get(key).and_then(OptionValue::as_str) {
            return Cow::Borrowed(value);
        }
        match self.default_of(key) {
            Some(ConfigDefault::Str(s)) => Cow::Borrowed(s),
            _ => Cow::Borrowed(""),
        }
    }

    /// Configured string list, else the schema default, else empty.
    pub fn str_list(&self, key: &str) -> Vec<String> {
        if let Some(value) = self.get(key) {
            return value.to_string_list();
        }
        match self.default_of(key) {
            Some(ConfigDefault::StrList(items)) => items.iter().map(|s| (*s).to_string()).collect(),
            Some(ConfigDefault::Str(s)) => vec![(*s).to_string()],
            _ => Vec::new(),
        }
    }

    /// An `EnforcedStyle`-like option: the value must appear in the
    /// schema's `allowed` list.
    pub fn style(&self, key: &str) -> Result<&str, OptionError> {
        let allowed = self
            .meta
            .config
            .iter()
            .find(|opt| opt.name == key)
            .map_or::<&[&str], _>(&[], |opt| opt.allowed);
        let value = match self.get(key) {
            Some(OptionValue::Str(s)) => s.as_str(),
            Some(OptionValue::Null) | None => match self.default_of(key) {
                Some(ConfigDefault::Str(s)) => s,
                _ => return Err(self.error(key, "no value and no default".to_string())),
            },
            Some(other) => {
                return Err(self.error(key, format!("expected a string, got {other:?}")));
            }
        };
        if allowed.is_empty() || allowed.contains(&value) {
            Ok(value)
        } else {
            Err(self.error(
                key,
                format!(
                    "unsupported style `{value}`, supported styles are: {}",
                    allowed.join(", ")
                ),
            ))
        }
    }

    /// Another cop's configured option.
    pub fn peer(&self, cop: &str, key: &str) -> Option<&OptionValue> {
        self.peers.get(cop)?.iter().find(|(name, _)| name == key).map(|(_, value)| value)
    }

    /// Every peer cop name the loaded configuration knows about (every real
    /// cop `RuleOptions::new` was given peer options for), excluding the
    /// synthetic `"AllCops"` entry. Built from [`config::LoadedConfig`]'s
    /// full cop table (every cop in RuboCop's `config/default.yml`, not
    /// just ones a project's `.rubocop.yml` mentions), so this is the whole
    /// real cop registry, sorted. Empty when built via
    /// [`RuleOptions::defaults`] (no configuration loaded). Rules that need
    /// the whole cop universe -- `Lint/RedundantCopDisableDirective`'s
    /// `all`/department expansion and "did you mean" suggestions being the
    /// motivating case -- have no other way to enumerate it: `peer` only
    /// looks up one name at a time.
    pub fn peer_names(&self) -> impl Iterator<Item = &str> {
        self.peers.keys().map(String::as_str).filter(|name| *name != "AllCops")
    }

    /// Builds an [`OptionError`] attributed to this rule.
    pub fn error(&self, key: &str, message: impl Into<String>) -> OptionError {
        OptionError { rule: self.meta.name, option: key.to_string(), message: message.into() }
    }
}
