//! Shared logic behind RuboCop's `ConfigurableNaming`/`ConfigurableFormatting`
//! mixins (both `Naming::MethodName` and `Naming::VariableName` include
//! `ConfigurableNaming`, which itself includes `ConfigurableFormatting`) plus
//! the small `AllowedIdentifiers`/`AllowedPattern`/`ForbiddenIdentifiers`/
//! `ForbiddenPattern` mixins every naming cop mixes in on top. Not a `Rule`
//! itself -- `pub(crate)` helpers `naming::method_name`/`naming::variable_name`
//! call directly.
//!
//! # `FORMATS` and Unicode
//!
//! Upstream's two regexes (`snake_case`/`camelCase`) use Ruby's
//! `[[:lower:]]`/`[[:upper:]]` POSIX bracket classes, which -- unlike Rust's
//! `regex` crate, where they are ASCII-only -- are Unicode-aware in Ruby's
//! Onigmo engine (matching any codepoint with the Unicode `Lowercase`/
//! `Uppercase` property, e.g. `ú`). [`SNAKE_CASE`]/[`CAMEL_CASE`] use
//! `\p{Lowercase}`/`\p{Uppercase}` instead of `[[:lower:]]`/`[[:upper:]]` to
//! match that, verified against `MethodName`'s own "accepts for non-ascii
//! characters" fixture (`última_vista`).

use std::sync::LazyLock;

use regex::Regex;

/// RuboCop's `ConfigurableNaming::FORMATS[:snake_case]`, ported to Rust's
/// regex syntax (see the module doc for the `[[:lower:]]` substitution).
static SNAKE_CASE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^@{0,2}[\d\p{Lowercase}_]+[!?=]?$").expect("valid regex"));

/// RuboCop's `ConfigurableNaming::FORMATS[:camelCase]`.
static CAMEL_CASE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^@{0,2}(?:_|_?\p{Lowercase}[\d\p{Lowercase}\p{Uppercase}]*)[!?=]?$")
        .expect("valid regex")
});

/// RuboCop's `EnforcedStyle` for the two naming cops sharing
/// `ConfigurableNaming` (`snake_case`/`camelCase` -- the only two values
/// either cop's `config/default.yml` entry lists).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Style {
    SnakeCase,
    CamelCase,
}

impl Style {
    /// Parses an already-validated (via [`linter::RuleOptions::style`])
    /// `EnforcedStyle` value.
    pub(crate) fn parse(raw: &str) -> Self {
        if raw == "camelCase" {
            Self::CamelCase
        } else {
            Self::SnakeCase
        }
    }

    /// The raw `EnforcedStyle` config string this value came from --
    /// `MSG`'s `%<style>s` placeholder in both cops.
    pub(crate) fn config_name(self) -> &'static str {
        match self {
            Self::SnakeCase => "snake_case",
            Self::CamelCase => "camelCase",
        }
    }

    fn format(self) -> &'static Regex {
        match self {
            Self::SnakeCase => &SNAKE_CASE,
            Self::CamelCase => &CAMEL_CASE,
        }
    }
}

/// RuboCop's `ConfigurableFormatting#valid_name?`, minus the
/// `class_emitter_method?` escape hatch: `MethodName` applies that itself
/// (only singleton `def`s can ever match it), `VariableName` never needs it.
pub(crate) fn matches_style(style: Style, name: &[u8]) -> bool {
    match std::str::from_utf8(name) {
        Ok(s) => style.format().is_match(s),
        Err(_) => false,
    }
}

/// `AllowedIdentifiers`/`ForbiddenIdentifiers`' shared `SIGILS` constant:
/// `@`/`@@`/`$` are stripped before an identifier is compared, so
/// `AllowedIdentifiers: [fooBar]` also allows `@fooBar`/`@@fooBar`/`$fooBar`.
/// A sigil only ever leads an identifier, so a byte slice suffices -- no
/// per-comparison `Vec` allocation.
fn strip_sigils(name: &[u8]) -> &[u8] {
    match name {
        [b'@', b'@', rest @ ..] | [b'@' | b'$', rest @ ..] => rest,
        _ => name,
    }
}

/// RuboCop's `AllowedIdentifiers#allowed_identifier?`/
/// `ForbiddenIdentifiers#forbidden_identifier?`: `name`, sigils stripped,
/// exactly matches one entry of `list`. `list` empty always means "no
/// match" (mirrors both mixins' `list.any? && list.include?(name)` guard,
/// though it is redundant with `Vec::contains`'s own behaviour on an empty
/// slice -- kept for clarity with the upstream source).
pub(crate) fn identifier_matches(list: &[String], name: &[u8]) -> bool {
    if list.is_empty() {
        return false;
    }
    let stripped = strip_sigils(name);
    list.iter().any(|candidate| candidate.as_bytes() == stripped)
}

/// RuboCop's `AllowedPattern#matches_allowed_pattern?`/
/// `ForbiddenPattern#forbidden_pattern?`: `name` (sigils intact -- neither
/// mixin strips them) matches any of `patterns`.
pub(crate) fn pattern_matches(patterns: &[Regex], name: &[u8]) -> bool {
    if patterns.is_empty() {
        return false;
    }
    match std::str::from_utf8(name) {
        Ok(s) => patterns.iter().any(|pattern| pattern.is_match(s)),
        Err(_) => false,
    }
}

/// Compiles a rule's `AllowedPattern`/`ForbiddenPattern` config list into
/// regexes, silently dropping any entry that fails to compile (shared by
/// `Naming::MethodName` and `Naming::VariableName`, which each read the
/// same two config keys from their own `RuleOptions`).
pub(crate) fn compile_patterns(patterns: &[String]) -> Vec<Regex> {
    patterns.iter().filter_map(|p| Regex::new(p).ok()).collect()
}
