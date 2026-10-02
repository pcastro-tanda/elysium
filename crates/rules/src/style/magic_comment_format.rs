//! `Style/MagicCommentFormat`, ported from RuboCop's
//! `lib/rubocop/cop/style/magic_comment_format.rb` plus `RuboCop::MagicComment`.
//!
//! Upstream gates which comments it inspects through
//! `MagicComment.parse(comment.text).valid?`: a comment qualifies either as
//! an `EmacsComment` (contains a `-*- ... -*-` region, its semicolon-joined
//! tokens each checked against an *anchored* `keyword\s*:\s*token` pattern)
//! or a `SimpleComment` (the *whole* comment, trimmed, must be exactly one
//! `# keyword: token`). The actual format checking
//! (`CommentRange#directives`/`#values`) is independent of that
//! classification -- a single unanchored regex scan for any of the six
//! known keywords, each immediately followed by `:` and a value up to `;`
//! or end of line. Since every fixture's keyword:value pairing is
//! unambiguous, this collapses the two into one scan: a comment "valid" iff
//! that scan finds at least one keyword-with-value, documented as a
//! [`META`] blind spot for the (untested) case upstream's stricter
//! `SimpleComment` anchoring would reject but a looser mid-string match
//! would accept.
//!
//! `leading_comment_lines` (`processed_source.tokens.find(&:comment?)`'s
//! complement) has no token stream here; [`leading_cutoff`] rederives it as
//! the first top-level statement's start line, since a comment/blank-only
//! prefix produces no nodes at all.
//!
//! Multiple offenses on one line sort by `(line, message, column)` in this
//! harness (`tests/fixtures.rs`'s `render`), not registration order, so
//! `fix_directives` then `fix_values` (upstream's literal sequence) is kept
//! as-is without needing to match a particular visual order.

use std::sync::LazyLock;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::NodeExt as _;
use ruby_source::Span;

/// RuboCop's `MagicComment::KEYWORDS`, unioned and wrapped in a capturing
/// group so a plain `find_iter` yields each directive's own span.
fn directive_regex() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)(?:(?:en)?coding|frozen[_-]string[_-]literal|rbs_inline|warn[_-]indent|shareable[_-]constant[_-]value|typed)",
        )
        .expect("static regex is valid")
    });
    &RE
}

/// RuboCop's `MagicComment::VALUE_REGEXP`, with the `(?=;|$)` lookahead
/// (unsupported by this crate's regex engine) replaced by a consuming
/// `(?:;|$)` terminator -- harmless here since only the captured value
/// (group 1) is ever read.
fn value_regex() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)(?:(?:en)?coding|frozen[_-]string[_-]literal|rbs_inline|warn[_-]indent|shareable[_-]constant[_-]value|typed):\s*(.*?)(?:;|$)",
        )
        .expect("static regex is valid")
    });
    &RE
}

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Snake,
    Kebab,
}

/// A `DirectiveCapitalization`/`ValueCapitalization` setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Capitalization {
    Lower,
    Upper,
}

/// Use a consistent style for magic comments.
#[derive(Debug, Clone)]
pub struct MagicCommentFormat {
    style: Style,
    directive_capitalization: Option<Capitalization>,
    value_capitalization: Option<Capitalization>,
}

impl Rule for MagicCommentFormat {
    const META: RuleMeta = RuleMeta {
        name: "Style/MagicCommentFormat",
        department: Department::Style,
        summary: "Use a consistent style for magic comments.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[
            linter::ConfigOption {
                name: "EnforcedStyle",
                default: linter::ConfigDefault::Str("snake_case"),
                allowed: &["snake_case", "kebab_case"],
                doc: "Separator style for magic comments.",
            },
            linter::ConfigOption {
                name: "DirectiveCapitalization",
                default: linter::ConfigDefault::Str("lowercase"),
                allowed: &["lowercase", "uppercase"],
                doc: "Required capitalization for magic comment directives.",
            },
            linter::ConfigOption {
                name: "ValueCapitalization",
                default: linter::ConfigDefault::Nil,
                allowed: &["lowercase", "uppercase"],
                doc: "Required capitalization for magic comment values.",
            },
        ],
        blind_spots: "Validity (whether a comment counts as a magic comment \
            at all) is approximated by the same keyword-scan used for \
            format checking, rather than upstream's separate anchored \
            `SimpleComment`/`EmacsComment` parse; a comment with extraneous \
            text surrounding an otherwise-valid `keyword: value` substring \
            would be accepted here but rejected upstream. An explicit YAML \
            `DirectiveCapitalization: ~`/`ValueCapitalization: ~` override \
            and the key being entirely absent both surface as `None` here \
            (the config loader's `unset_nil` merge deletes an explicitly \
            nulled key outright), so `DirectiveCapitalization`'s documented \
            `lowercase` schema default can never actually take effect when \
            the key is unset only because every layer omitted it.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "kebab_case" => Style::Kebab,
            _ => Style::Snake,
        };
        // The resolved configuration represents an explicit YAML `~`
        // override the same way as the key being entirely absent (the
        // `unset_nil` merge step deletes it outright), so a missing key
        // cannot be distinguished from an explicit null here; every fixture
        // for this cop always states the key explicitly (often as `~`), so
        // both collapse to "no capitalization enforcement" -- see
        // `blind_spots`.
        let capitalization = |key: &str| match options.get(key) {
            Some(linter::OptionValue::Str(s)) => match s.as_str() {
                "lowercase" => Some(Capitalization::Lower),
                "uppercase" => Some(Capitalization::Upper),
                _ => None,
            },
            _ => None,
        };
        Ok(Self {
            style,
            directive_capitalization: capitalization("DirectiveCapitalization"),
            value_capitalization: capitalization("ValueCapitalization"),
        })
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let cutoff = leading_cutoff(ctx);

        let comments: Vec<linter::CommentInfo> = ctx.comments().to_vec();
        for comment in comments {
            if comment.kind != linter::CommentKind::Inline {
                continue;
            }
            if cutoff.is_some_and(|cutoff| comment.line >= cutoff) {
                continue;
            }
            let text = ctx.text(comment.span);
            let Ok(text_str) = std::str::from_utf8(text) else { continue };

            let value_spans: Vec<Span> = value_regex()
                .captures_iter(text_str)
                .filter_map(|caps| caps.get(1))
                .map(|m| {
                    Span::new(
                        comment.span.start + u32::try_from(m.start()).expect("fits u32"),
                        comment.span.start + u32::try_from(m.end()).expect("fits u32"),
                    )
                })
                .collect();
            if value_spans.is_empty() {
                continue;
            }

            let directive_spans: Vec<Span> = directive_regex()
                .find_iter(text_str)
                .map(|m| {
                    Span::new(
                        comment.span.start + u32::try_from(m.start()).expect("fits u32"),
                        comment.span.start + u32::try_from(m.end()).expect("fits u32"),
                    )
                })
                .collect();

            let flagged_directives: Vec<Span> = directive_spans
                .into_iter()
                .filter(|&span| {
                    let directive_text = ctx.text(span);
                    incorrect_separator(directive_text, self.style)
                        || wrong_capitalization(directive_text, self.directive_capitalization)
                })
                .collect();
            let flagged_values: Vec<Span> = value_spans
                .into_iter()
                .filter(|&span| wrong_capitalization(ctx.text(span), self.value_capitalization))
                .collect();

            let expected_style = expected_style_text(self.directive_capitalization, self.style);
            for span in flagged_directives {
                let message = format!("Prefer {expected_style} case for magic comments.");
                let replacement = replace_separator(
                    &replace_capitalization(ctx.text(span), self.directive_capitalization),
                    self.style,
                );
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
            for span in flagged_values {
                let case_name = match self.value_capitalization {
                    Some(Capitalization::Lower) => "lowercase",
                    Some(Capitalization::Upper) => "uppercase",
                    None => unreachable!("only flagged when a capitalization is set"),
                };
                let message = format!("Prefer {case_name} for magic comment values.");
                let replacement = replace_capitalization(ctx.text(span), self.value_capitalization);
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
}

/// RuboCop's `leading_comment_lines`: the line of the first top-level
/// statement, or `None` (every comment qualifies) when the file holds no
/// code at all.
fn leading_cutoff(ctx: &Context<'_>) -> Option<u32> {
    let program = ctx.parsed().root().as_program_node()?;
    let first = program.statements().body().first()?;
    Some(ctx.line_col(first.span().start).line)
}

/// RuboCop's `incorrect_separator?`.
fn incorrect_separator(text: &[u8], style: Style) -> bool {
    let wrong = if style == Style::Snake { b'-' } else { b'_' };
    text.contains(&wrong)
}

/// RuboCop's `wrong_capitalization?`.
fn wrong_capitalization(text: &[u8], expected: Option<Capitalization>) -> bool {
    let Some(expected) = expected else { return false };
    let Ok(text_str) = std::str::from_utf8(text) else { return false };
    match expected {
        Capitalization::Lower => text_str != text_str.to_lowercase(),
        Capitalization::Upper => text_str != text_str.to_uppercase(),
    }
}

/// RuboCop's `replace_separator`.
fn replace_separator(text: &[u8], style: Style) -> Vec<u8> {
    let (wrong, correct) = if style == Style::Snake { (b'-', b'_') } else { (b'_', b'-') };
    text.iter().map(|&b| if b == wrong { correct } else { b }).collect()
}

/// RuboCop's `replace_capitalization`.
fn replace_capitalization(text: &[u8], style: Option<Capitalization>) -> Vec<u8> {
    let Some(style) = style else { return text.to_vec() };
    let Ok(text_str) = std::str::from_utf8(text) else { return text.to_vec() };
    match style {
        Capitalization::Lower => text_str.to_lowercase().into_bytes(),
        Capitalization::Upper => text_str.to_uppercase().into_bytes(),
    }
}

/// RuboCop's `expected_style`: `[directive_capitalization, style].compact
/// .join(' ').gsub(/_?case\b/, '')`.
fn expected_style_text(directive_capitalization: Option<Capitalization>, style: Style) -> String {
    let case_word = match directive_capitalization {
        Some(Capitalization::Lower) => Some("lower"),
        Some(Capitalization::Upper) => Some("upper"),
        None => None,
    };
    let style_word = match style {
        Style::Snake => "snake",
        Style::Kebab => "kebab",
    };
    match case_word {
        Some(case_word) => format!("{case_word} {style_word}"),
        None => style_word.to_string(),
    }
}
