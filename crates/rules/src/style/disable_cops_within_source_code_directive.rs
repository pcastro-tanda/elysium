//! `Style/DisableCopsWithinSourceCodeDirective`, ported from RuboCop's
//! `lib/rubocop/cop/style/disable_cops_within_source_code_directive.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_directives::CopRef;

const MSG: &str = "RuboCop disable/enable directives are not permitted.";

/// Forbids disabling/enabling cops within source code.
#[derive(Debug, Clone)]
pub struct DisableCopsWithinSourceCodeDirective {
    allowed_cops: Vec<String>,
}

impl DisableCopsWithinSourceCodeDirective {
    fn cop_name(cop: &CopRef) -> &str {
        match cop {
            CopRef::All => "all",
            CopRef::Department(name) | CopRef::Cop(name) => name,
        }
    }
}

impl Rule for DisableCopsWithinSourceCodeDirective {
    const META: RuleMeta = RuleMeta {
        name: "Style/DisableCopsWithinSourceCodeDirective",
        department: Department::Style,
        summary: "Forbids disabling/enabling cops within source code.",
        explanation: "Detects comments to enable/disable RuboCop.\nThis is useful if want to make sure that every RuboCop error gets fixed\nand not quickly disabled with a comment.\n\nSpecific cops can be allowed with the `AllowedCops` configuration. Note that\nif this configuration is set, `rubocop:disable all` is still disallowed.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[ConfigOption {
            name: "AllowedCops",
            default: ConfigDefault::StrList(&[]),
            allowed: &[],
            doc: "Cops that can be disabled/enabled by a directive comment.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allowed_cops: options.str_list("AllowedCops") })
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let any_cops_allowed = !self.allowed_cops.is_empty();

        let directives: Vec<_> = ctx.directives().directives().to_vec();
        for directive in &directives {
            let directive_cops: Vec<&str> = directive.cops.iter().map(Self::cop_name).collect();
            let disallowed_cops: Vec<&str> = directive_cops
                .iter()
                .copied()
                .filter(|c| !self.allowed_cops.iter().any(|a| a == c))
                .collect();

            if disallowed_cops.is_empty() {
                continue;
            }

            // The comment's own full span (`# rubocop:...` through end of
            // line), not `directive.span` which stops before any trailing
            // free text -- matching upstream's `add_offense(comment, ...)`.
            let Some(comment) =
                ctx.comments().iter().find(|c| c.span.start == directive.span.start)
            else {
                continue;
            };
            let span = comment.span;

            let message = if any_cops_allowed {
                let list =
                    disallowed_cops.iter().map(|c| format!("`{c}`")).collect::<Vec<_>>().join(", ");
                format!("RuboCop disable/enable directives for {list} are not permitted.")
            } else {
                MSG.to_string()
            };

            let replacement = if directive_cops.len() == disallowed_cops.len() {
                Vec::new()
            } else {
                remove_first_disallowed(ctx.text(span), &disallowed_cops)
            };

            ctx.report_with_fix(
                &Self::META,
                span,
                message,
                Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::replace(span, replacement)],
                },
            );
        }
    }
}

/// Upstream: `comment.text.sub(/#{Regexp.union(disallowed_cops)},?\s*/, '')`
/// `.sub(/,\s*$/, '')`. Removes only the first (leftmost) occurrence of any
/// disallowed cop name, plus a following comma and whitespace, then trims a
/// trailing `,\s*` left dangling at the very end of the text.
fn remove_first_disallowed(text: &[u8], disallowed_cops: &[&str]) -> Vec<u8> {
    let mut found: Option<(usize, usize)> = None;
    for pos in 0..text.len() {
        for cop in disallowed_cops {
            let bytes = cop.as_bytes();
            if text[pos..].starts_with(bytes) {
                found = Some((pos, bytes.len()));
                break;
            }
        }
        if found.is_some() {
            break;
        }
    }

    let Some((start, len)) = found else {
        return text.to_vec();
    };

    let mut end = start + len;
    if text.get(end) == Some(&b',') {
        end += 1;
    }
    while text.get(end).is_some_and(u8::is_ascii_whitespace) {
        end += 1;
    }

    let mut result = Vec::with_capacity(text.len() - (end - start));
    result.extend_from_slice(&text[..start]);
    result.extend_from_slice(&text[end..]);

    // Trailing `,\s*$` trim.
    let mut trim_to = result.len();
    let mut i = result.len();
    while i > 0 && result[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    if i > 0 && result[i - 1] == b',' {
        trim_to = i - 1;
    }
    if trim_to != result.len() {
        result.truncate(trim_to);
    }

    result
}
