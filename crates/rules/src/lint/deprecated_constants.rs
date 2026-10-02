//! `Lint/DeprecatedConstants`, ported from RuboCop's
//! `lib/rubocop/cop/lint/deprecated_constants.rb`.
//!
//! Upstream's `FIXME` workaround (`return unless node.loc`) exists only
//! because whitequark parses `__ENCODING__` as a `:const` node with no
//! location; Prism has a dedicated `SourceEncodingNode` kind for it
//! instead, so it never reaches a `ConstantReadNode`/`ConstantPathNode`
//! handler here in the first place and no equivalent guard is needed.

use std::collections::HashMap;
use std::sync::Arc;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// `config/default.yml`'s `DeprecatedConstants` default.
const DEFAULTS: &[(&str, Option<&str>, Option<&str>)] = &[
    ("NIL", Some("nil"), Some("2.4")),
    ("TRUE", Some("true"), Some("2.4")),
    ("FALSE", Some("false"), Some("2.4")),
    ("Net::HTTPServerException", Some("Net::HTTPClientException"), Some("2.6")),
    ("Random::DEFAULT", Some("Random.new"), Some("3.0")),
    ("Struct::Group", Some("Etc::Group"), Some("3.0")),
    ("Struct::Passwd", Some("Etc::Passwd"), Some("3.0")),
];

#[derive(Debug)]
struct Deprecation {
    alternative: Option<String>,
    /// The configured `DeprecatedVersion` text, verbatim (for the message),
    /// and its parsed numeric form (for the `target_ruby_version` gate).
    deprecated_version: Option<(String, f32)>,
}

fn map_str(entries: &[(String, OptionValue)], key: &str) -> Option<String> {
    entries.iter().find(|(k, _)| k == key).and_then(|(_, v)| v.as_str()).map(ToString::to_string)
}

fn deprecation_from(alternative: Option<String>, version: Option<String>) -> Deprecation {
    let deprecated_version = version.and_then(|v| v.parse::<f32>().ok().map(|parsed| (v, parsed)));
    Deprecation { alternative, deprecated_version }
}

fn build_deprecations(options: &RuleOptions) -> HashMap<String, Deprecation> {
    let mut map = HashMap::new();
    match options.get("DeprecatedConstants") {
        Some(OptionValue::Map(entries)) => {
            for (name, value) in entries {
                let OptionValue::Map(sub) = value else { continue };
                let alternative = map_str(sub, "Alternative");
                let version = map_str(sub, "DeprecatedVersion");
                map.insert(name.clone(), deprecation_from(alternative, version));
            }
        }
        _ => {
            for &(name, alternative, version) in DEFAULTS {
                map.insert(
                    name.to_string(),
                    deprecation_from(
                        alternative.map(ToString::to_string),
                        version.map(ToString::to_string),
                    ),
                );
            }
        }
    }
    map
}

fn message(deprecation: &Deprecation, bad: &str) -> String {
    let deprecated_message = match &deprecation.deprecated_version {
        Some((text, _)) => format!(", deprecated since Ruby {text}"),
        None => String::new(),
    };
    match &deprecation.alternative {
        Some(good) => format!("Use `{good}` instead of `{bad}`{deprecated_message}."),
        None => format!("Do not use `{bad}`{deprecated_message}."),
    }
}

/// Checks for deprecated constants.
#[derive(Debug, Clone)]
pub struct DeprecatedConstants {
    deprecations: Arc<HashMap<String, Deprecation>>,
    target_ruby_version: f32,
}

impl Rule for DeprecatedConstants {
    const META: RuleMeta = RuleMeta {
        name: "Lint/DeprecatedConstants",
        department: Department::Lint,
        summary: "Checks for deprecated constants.",
        explanation: "\
Checks for deprecated constants.

It has `DeprecatedConstants` config. If there is an alternative method, you \
can set alternative value as `Alternative`. And you can set the deprecated \
version as `DeprecatedVersion`. These options can be omitted if they are \
not needed.

By default, `NIL`, `TRUE`, `FALSE`, `Net::HTTPServerException`, \
`Random::DEFAULT`, `Struct::Group`, and `Struct::Passwd` are configured.

```ruby
# bad
NIL
TRUE
FALSE
Net::HTTPServerException
Random::DEFAULT # Return value of Ruby 2 is `Random` instance, Ruby 3.0 is `Random` class.
Struct::Group
Struct::Passwd

# good
nil
true
false
Net::HTTPClientException
Random.new # `::DEFAULT` has been deprecated in Ruby 3, `.new` is compatible with Ruby 2.
Etc::Group
Etc::Passwd
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ConstantReadNode, NodeKind::ConstantPathNode],
        config: &[ConfigOption {
            name: "DeprecatedConstants",
            default: ConfigDefault::Nil,
            allowed: &[],
            doc:
                "Deprecated constants mapped to an optional `Alternative` and `DeprecatedVersion`. \
Defaults: NIL, TRUE, FALSE, Net::HTTPServerException, Random::DEFAULT, Struct::Group, \
Struct::Passwd.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            deprecations: Arc::new(build_deprecations(options)),
            target_ruby_version: options.target_ruby_version(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let span = node.span();
        let source = ctx.text(span);
        let lookup_bytes = source.strip_prefix(b"::").unwrap_or(source);
        let lookup = String::from_utf8_lossy(lookup_bytes).into_owned();

        let Some(deprecation) = self.deprecations.get(&lookup) else { return };
        if let Some((_, version)) = deprecation.deprecated_version {
            if self.target_ruby_version < version {
                return;
            }
        }

        let bad = String::from_utf8_lossy(source).into_owned();
        let text = message(deprecation, &bad);

        match &deprecation.alternative {
            Some(good) => {
                let fix = Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::replace(span, good.clone().into_bytes())],
                };
                ctx.report_with_fix(&Self::META, span, text, fix);
            }
            None => ctx.report(&Self::META, span, text),
        }
    }
}
