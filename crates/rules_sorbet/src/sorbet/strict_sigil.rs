//! `Sorbet/StrictSigil`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/sigils/strict_sigil.rb` (and its base class
//! `Sorbet::ValidSigil`, `sigils/valid_sigil.rb`).
//!
//! Approximation: the first token of a file with no leading comment is
//! located by scanning bytes (there is no token stream), so its offense
//! width is only exact for common token shapes.

use std::sync::OnceLock;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use regex::bytes::Regex;
use ruby_source::Span;

/// All files must be at least at strictness `strict`.
#[derive(Debug, Clone)]
pub struct StrictSigil {
    suggested: String,
    exact: Option<String>,
}

impl Rule for StrictSigil {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/StrictSigil",
        department: Department::Sorbet,
        summary: "All files must be at least at strictness `strict`.",
        explanation: "Makes the Sorbet `strict` sigil mandatory in all files.\n\n```ruby\n# bad\n# typed: true\n\n# bad\n# typed: false\n\n# good\n# typed: strict\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[
            ConfigOption {
                name: "SuggestedStrictness",
                default: ConfigDefault::Str("strict"),
                allowed: &[],
                doc: "Sorbet strictness level suggested in offense messages and used in autocorrect.",
            },
            ConfigOption {
                name: "ExactStrictness",
                default: ConfigDefault::Nil,
                allowed: &[],
                doc: "If set, make offense if the strictness level in the file is different than this one.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            suggested: suggested_option(options),
            exact: level_option(options, "ExactStrictness"),
        })
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        Check {
            meta: &Self::META,
            require_sigil_on_all_files: true,
            suggested: &self.suggested,
            minimum: Some("strict"),
            exact: self.exact.as_deref(),
            applicability: Applicability::Unsafe,
            reported: Vec::new(),
        }
        .run(ctx);
    }
}

const STRICTNESS_LEVELS: [&str; 5] = ["ignore", "false", "true", "strict", "strong"];

fn sigil_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?m)^[[:blank:]]*(?:#[[:blank:]]*)?#[[:blank:]]+typed:(?:[[:blank:]]+((?-u:\S)+))?",
        )
        .expect("valid regex")
    })
}

fn double_comment_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\A[[:blank:]]*#[[:blank:]]*#").expect("valid regex"))
}

fn level_index(level: &str) -> Option<usize> {
    STRICTNESS_LEVELS.iter().position(|&l| l == level)
}

/// Ruby's lexer whitespace between tokens.
fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r')
}

/// The leading `tCOMMENT` tokens of the file (`tokens.take_while`), and the
/// span of the first token overall. `None` when the file has no tokens.
fn leading_comments(ctx: &Context<'_>) -> Option<(Vec<Span>, Span)> {
    let source = ctx.source().bytes();
    let mut comments = ctx.comments().iter().peekable();
    let mut pos = 0usize;
    let mut leading = Vec::new();
    loop {
        while pos < source.len() && is_ws(source[pos]) {
            pos += 1;
        }
        let at = u32::try_from(pos).expect("offset exceeds u32");
        match comments.peek() {
            Some(comment) if comment.span.start == at => {
                leading.push(comment.span);
                pos = comment.span.end as usize;
                comments.next();
            }
            _ => break,
        }
    }
    if let Some(first) = leading.first() {
        return Some((leading.clone(), *first));
    }
    if pos >= source.len() || source[pos..].starts_with(b"__END__") {
        return None;
    }
    let start = u32::try_from(pos).expect("offset exceeds u32");
    let width = first_token_width(&source[pos..]);
    Some((leading, Span::new(start, start + width)))
}

fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80
}

/// Width of the first (non-comment) lexical token: identifiers/keywords,
/// variables, numbers, quoted strings, otherwise one byte.
fn first_token_width(bytes: &[u8]) -> u32 {
    let b0 = bytes[0];
    let mut i = 1;
    if is_ident_continue(b0) && !b0.is_ascii_digit() {
        while i < bytes.len() && is_ident_continue(bytes[i]) {
            i += 1;
        }
        if i < bytes.len() && matches!(bytes[i], b'?' | b'!') && bytes.get(i + 1) != Some(&b'=') {
            i += 1;
        }
    } else if b0 == b'@' || b0 == b'$' {
        while i < bytes.len() && (is_ident_continue(bytes[i]) || bytes[i] == b'@') {
            i += 1;
        }
    } else if b0.is_ascii_digit() {
        while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
            i += 1;
        }
    } else if b0 == b'"' || b0 == b'\'' {
        while i < bytes.len() {
            match bytes[i] {
                b'\\' => i += 2,
                b if b == b0 => {
                    i += 1;
                    break;
                }
                b'#' if b0 == b'"' && bytes.get(i + 1) == Some(&b'{') => {
                    i = 1;
                    break;
                }
                _ => i += 1,
            }
        }
        i = i.min(bytes.len());
    }
    u32::try_from(i).expect("offset exceeds u32")
}

/// `Sorbet::ValidSigil#on_new_investigation`.
struct Check<'o> {
    meta: &'static RuleMeta,
    require_sigil_on_all_files: bool,
    suggested: &'o str,
    minimum: Option<&'o str>,
    exact: Option<&'o str>,
    applicability: Applicability,
    reported: Vec<Span>,
}

impl Check<'_> {
    /// `add_offense` ignores a second offense at an already-reported range.
    fn add(&mut self, ctx: &mut Context<'_>, span: Span, message: String, edits: Vec<Edit>) {
        if self.reported.contains(&span) {
            return;
        }
        self.reported.push(span);
        if edits.is_empty() {
            ctx.report(self.meta, span, message);
        } else {
            ctx.report_with_fix(
                self.meta,
                span,
                message,
                Fix { applicability: self.applicability, edits },
            );
        }
    }

    fn run(&mut self, ctx: &mut Context<'_>) {
        let Some((leading, first)) = leading_comments(ctx) else { return };
        let found = leading.iter().find_map(|&span| {
            let caps = sigil_regex().captures(ctx.text(span))?;
            let strictness =
                caps.get(1).map(|m| String::from_utf8_lossy(m.as_bytes()).into_owned());
            Some((span, strictness))
        });

        // check_sigil_present
        let Some((sigil, strictness)) = found else {
            if self.require_sigil_on_all_files {
                let level = self.suggested_strictness_level();
                let message = format!(
                    "No Sorbet sigil found in file. Try a `typed: {level}` to start (you can \
                     also use `rubocop -a` to automatically add this)."
                );
                let edits = self.autocorrect(ctx, first, true);
                self.add(ctx, first, message, edits);
            }
            return;
        };

        // check_double_commented_sigil
        let text = ctx.text(sigil);
        if double_comment_regex().is_match(text) {
            let message = format!("Invalid Sorbet sigil `{}`.", String::from_utf8_lossy(text));
            let replacement = format!("# typed: {}", strictness.as_deref().unwrap_or(""));
            self.add(ctx, sigil, message, vec![Edit::replace(sigil, replacement.into_bytes())]);
        }

        // check_strictness_not_empty
        let Some(strictness) = strictness else {
            self.add(ctx, sigil, "Sorbet sigil should not be empty.".to_owned(), Vec::new());
            return;
        };

        // check_strictness_valid
        let Some(current_level) = level_index(&strictness) else {
            self.add(ctx, sigil, format!("Invalid Sorbet sigil `{strictness}`."), Vec::new());
            return;
        };

        // check_strictness_level
        if let Some(exact) = self.exact {
            if Some(current_level) != level_index(exact) {
                let message = format!("Sorbet sigil should be `{exact}` got `{strictness}`.");
                self.add(ctx, sigil, message, Vec::new());
            }
        } else if let Some(minimum) = self.minimum {
            if level_index(minimum).is_some_and(|min| current_level < min) {
                let message =
                    format!("Sorbet sigil should be at least `{minimum}` got `{strictness}`.");
                self.add(ctx, sigil, message, Vec::new());
            }
        }
    }

    fn suggested_strictness_level(&self) -> &str {
        if let Some(exact) = self.exact {
            return exact;
        }
        let Some(minimum) = self.minimum else { return self.suggested };
        // `default.yml` always supplies `SuggestedStrictness`, so the
        // `ignore`-without-config special case never applies; take the
        // higher of the suggested and minimum levels.
        let level = level_index(self.suggested).max(level_index(minimum));
        level.map_or(self.suggested, |i| STRICTNESS_LEVELS[i])
    }

    /// `autocorrect`: only acts when the file has no sigil (`sigil_missing`).
    fn autocorrect(&self, ctx: &Context<'_>, token: Span, sigil_missing: bool) -> Vec<Edit> {
        if !self.require_sigil_on_all_files || !sigil_missing {
            return Vec::new();
        }
        let sigil = format!("# typed: {}", self.suggested_strictness_level());
        if ctx.text(token).starts_with(b"#!") {
            vec![Edit::insert(token.end, format!("\n{sigil}").into_bytes())]
        } else {
            vec![Edit::insert(token.start, format!("{sigil}\n").into_bytes())]
        }
    }
}

/// Strictness option: set and a valid level, else `None`.
fn level_option(options: &RuleOptions, key: &str) -> Option<String> {
    let value = options.str(key);
    STRICTNESS_LEVELS.contains(&&*value).then(|| value.into_owned())
}

/// `suggested_strictness`: the configured level when valid, else `false`.
fn suggested_option(options: &RuleOptions) -> String {
    level_option(options, "SuggestedStrictness").unwrap_or_else(|| "false".to_owned())
}
