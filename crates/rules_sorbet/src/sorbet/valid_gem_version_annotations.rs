//! `Sorbet/ValidGemVersionAnnotations`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/rbi_versioning/valid_gem_version_annotations.rb`
//! (with its `GemVersionAnnotationHelper` mixin).

use std::sync::LazyLock;

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use regex::Regex;
use ruby_ast::Node;

const VERSION_PREFIX: &str = "# @version";
const VALID_OPERATORS: [&str; 7] = ["=", "!=", ">", ">=", "<", "<=", "~>"];

/// `Gem::Version::ANCHORED_VERSION_PATTERN` (the version itself is optional).
static ANCHORED_VERSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?:[0-9]+(?:\.[0-9a-zA-Z]+)*(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?)?\s*$")
        .expect("valid regex")
});

/// Checks that gem versions in RBI annotations are properly formatted per the
/// Bundler gem specification.
#[derive(Debug, Clone)]
pub struct ValidGemVersionAnnotations;

impl Rule for ValidGemVersionAnnotations {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/ValidGemVersionAnnotations",
        department: Department::Sorbet,
        summary: "Checks that gem versions in RBI annotations are properly formatted per the \
                  Bundler gem specification.",
        explanation: "Checks that gem versions in RBI annotations are properly formatted per \
                      the Bundler gem specification.\n\n```ruby\n# bad\n# @version > not a \
                      version number\n\n# good\n# @version = 1\n\n# good\n# @version > \
                      1.2.3\n\n# good\n# @version <= 4.3-preview\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let comments: Vec<_> = ctx.comments().iter().map(|comment| comment.span).collect();
        for span in comments {
            let text = String::from_utf8_lossy(ctx.text(span)).into_owned();
            let Some(rest) = text.strip_prefix(VERSION_PREFIX) else { continue };
            let versions = gem_versions(rest);
            if versions.is_empty() {
                ctx.report(&Self::META, span, "Invalid gem version(s) detected: empty version");
                break;
            }
            let invalid: Vec<&str> = versions
                .iter()
                .filter(|version| !valid_version(version))
                .map(|v| strip(v))
                .collect();
            if !invalid.is_empty() {
                ctx.report(
                    &Self::META,
                    span,
                    format!("Invalid gem version(s) detected: {}", invalid.join(", ")),
                );
            }
        }
    }

    fn enter(&mut self, _node: &Node<'_>, _ctx: &mut Context<'_>) {}
}

/// Ruby's `String#strip`: ASCII whitespace and NUL.
fn strip(text: &str) -> &str {
    text.trim_matches(|c: char| c.is_ascii_whitespace() || c == '\0' || c == '\u{b}')
}

/// `text.split(/, ?/).map(&:strip)`: Ruby's `split` drops trailing empty
/// fields (and yields nothing for an empty string).
fn gem_versions(text: &str) -> Vec<&str> {
    let mut fields = Vec::new();
    let mut rest = text;
    while let Some(comma) = rest.find(',') {
        fields.push(&rest[..comma]);
        rest = &rest[comma + 1..];
        rest = rest.strip_prefix(' ').unwrap_or(rest);
    }
    fields.push(rest);
    while fields.last().is_some_and(|field| field.is_empty()) {
        fields.pop();
    }
    fields.into_iter().map(strip).collect()
}

fn valid_version(version: &str) -> bool {
    let mut parts = version
        .split(|c: char| c.is_ascii_whitespace() || c == '\u{b}')
        .filter(|part| !part.is_empty());
    let Some(operator) = parts.next() else { return false };
    if !VALID_OPERATORS.contains(&operator) {
        return false;
    }
    // `Gem::Version.correct?(nil)` matches the empty string.
    ANCHORED_VERSION.is_match(parts.next().unwrap_or(""))
}
