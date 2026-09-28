//! `Style/Encoding`, ported from RuboCop's `lib/rubocop/cop/style/encoding.rb`
//! and its `RuboCop::MagicComment` helper (`lib/rubocop/magic_comment.rb`).
//!
//! # Magic comment parsing
//!
//! Upstream scans `processed_source.lines` from the top, skipping shebang
//! lines, and stops at the first line that is not a *valid* magic comment
//! (`MagicComment#valid?`: starts with `#` and specifies at least one of
//! `frozen_string_literal`/`encoding`/`rbs_inline`/`shareable_constant_value`/
//! `typed`). Each line is classified as an Emacs (`-*- ... -*-`), Vim
//! (`# vim: ...`) or plain comment, each with its own token syntax; this
//! mirrors that three-way dispatch with [`MagicComment::parse`] instead of
//! a class hierarchy.

use std::sync::LazyLock;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_source::{Side, Span};

/// RuboCop's `MSG`.
const MSG: &str = "Unnecessary utf-8 encoding comment.";

/// RuboCop's `SHEBANG`.
const SHEBANG: &str = "#!";

/// `RuboCop::MagicComment::TOKEN`.
const TOKEN: &str = r"[[:alnum:]\-_]+";

/// Checks ensures source files have no utf-8 encoding comments.
#[derive(Debug, Clone)]
pub struct Encoding;

impl Rule for Encoding {
    const META: RuleMeta = RuleMeta {
        name: "Style/Encoding",
        department: Department::Style,
        summary: "Checks ensures source files have no utf-8 encoding comments.",
        explanation: "\
Checks ensures source files have no utf-8 encoding comments.

```ruby
# bad
# encoding: UTF-8
# coding: UTF-8
# -*- coding: UTF-8 -*-
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        if ctx.source().bytes().is_empty() {
            return;
        }

        for (_line, span) in ctx.lines() {
            let Ok(text) = std::str::from_utf8(ctx.text(span)) else { break };

            if text.starts_with(SHEBANG) {
                continue;
            }

            let Some(comment) = MagicComment::parse(text) else { break };
            if !comment.is_valid() {
                break;
            }

            if let Some(encoding) = comment.encoding() {
                if encoding.eq_ignore_ascii_case("utf-8") {
                    register_offense(ctx, span, &comment);
                }
            }
        }
    }
}

/// RuboCop's `MagicComment.parse` dispatch: Emacs (`-*- ... -*-`) is tried
/// first, then Vim (`# vim: ...`), else a plain comment.
#[derive(Debug)]
enum MagicComment<'a> {
    /// `RuboCop::MagicComment::EmacsComment`.
    Emacs { tokens: Vec<&'a str> },
    /// `RuboCop::MagicComment::VimComment`.
    Vim { tokens: Vec<&'a str> },
    /// `RuboCop::MagicComment::SimpleComment`.
    Simple { comment: &'a str },
}

fn emacs_re() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"-\*-(.+)-\*-").expect("valid"));
    &RE
}

fn vim_re() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#\s*vim:\s*(.+)").expect("valid"));
    &RE
}

impl<'a> MagicComment<'a> {
    /// `MagicComment.parse`, returning `None` when the line does not even
    /// start with `#` (upstream's `valid?` first conjunct; folded in here
    /// since every caller needs it immediately).
    fn parse(line: &'a str) -> Option<Self> {
        if !line.starts_with('#') {
            return None;
        }

        if let Some(m) = emacs_re().captures(line).and_then(|c| c.get(1)) {
            let tokens = m.as_str().split(';').map(str::trim).collect();
            return Some(Self::Emacs { tokens });
        }

        if let Some(m) = vim_re().captures(line).and_then(|c| c.get(1)) {
            let tokens = m.as_str().split(", ").map(str::trim).collect();
            return Some(Self::Vim { tokens });
        }

        Some(Self::Simple { comment: line })
    }

    /// `MagicComment#valid?` minus the `start_with?('#')` check already
    /// applied in [`Self::parse`]: `any?`, restricted per subclass to the
    /// magic comment kinds that subclass actually supports.
    fn is_valid(&self) -> bool {
        match self {
            Self::Emacs { tokens } => {
                editor_match(tokens, "frozen[_-]string[_-]literal", ":").is_some()
                    || editor_match(tokens, "(?:en)?coding", ":").is_some()
                    || editor_match(tokens, "shareable[_-]constant[_-]value", ":").is_some()
            }
            Self::Vim { .. } => self.encoding().is_some(),
            Self::Simple { comment } => {
                simple_extract(comment, "frozen[_-]string[_-]literal").is_some()
                    || self.encoding().is_some()
                    || simple_rbs_inline_specified(comment)
                    || simple_extract(comment, "shareable[_-]constant[_-]value").is_some()
                    || simple_extract(comment, "typed").is_some()
            }
        }
    }

    /// `EditorComment#encoding` / `VimComment#encoding` /
    /// `SimpleComment#encoding`, downcased so callers can compare
    /// case-insensitively like upstream's `casecmp`.
    fn encoding(&self) -> Option<String> {
        match self {
            Self::Emacs { tokens } => editor_match(tokens, "(?:en)?coding", ":"),
            // `VimComment#encoding`: the `fileencoding` token only works
            // when at least one other token is present.
            Self::Vim { tokens } if tokens.len() > 1 => editor_match(tokens, "fileencoding", "="),
            Self::Vim { .. } => None,
            Self::Simple { comment } => {
                static RE: LazyLock<Regex> = LazyLock::new(|| {
                    Regex::new(&format!(
                        r"(?i)\A\s*#\s*(?:frozen_string_literal:\s*(?:true|false))?\s*(?:en)?coding: ({TOKEN})"
                    ))
                    .expect("valid")
                });
                RE.captures(comment).map(|c| c[1].to_string())
            }
        }
    }

    /// `EditorComment#without` / `SimpleComment#without`, called only for
    /// `:encoding` here.
    fn without_encoding(&self) -> String {
        match self {
            Self::Emacs { tokens } => {
                without_encoding_editor(tokens, "(?:en)?coding", ":", "# -*- %s -*-", ";")
            }
            Self::Vim { tokens } => {
                without_encoding_editor(tokens, "fileencoding", "=", "# vim: %s", ", ")
            }
            Self::Simple { comment } => {
                static RE: LazyLock<Regex> =
                    LazyLock::new(|| Regex::new(r"(?i)\A#\s*(?:en)?coding").expect("valid"));
                if RE.is_match(comment) {
                    String::new()
                } else {
                    (*comment).to_string()
                }
            }
        }
    }
}

/// `EditorComment#match(keyword)`: finds the first token matching
/// `\A{keyword}\s*{operator}\s*TOKEN\z` and returns its value, downcased.
fn editor_match(tokens: &[&str], keyword: &str, operator: &str) -> Option<String> {
    let pattern =
        Regex::new(&format!(r"\A{keyword}\s*{operator}\s*({TOKEN})\z")).expect("valid regex");
    tokens.iter().find_map(|token| pattern.captures(token).map(|c| c[1].to_lowercase()))
}

/// `EditorComment#without(:encoding)`: rewrites the comment with any token
/// matching `keyword` removed, or `""` if nothing remains.
fn without_encoding_editor(
    tokens: &[&str],
    keyword: &str,
    _operator: &str,
    format: &str,
    separator: &str,
) -> String {
    let pattern = Regex::new(&format!(r"\A(?:{keyword})")).expect("valid regex");
    let remaining: Vec<&str> = tokens.iter().copied().filter(|t| !pattern.is_match(t)).collect();
    if remaining.is_empty() {
        return String::new();
    }
    format.replacen("%s", &remaining.join(separator), 1)
}

/// `SimpleComment`'s anchored `extract_*` helpers used only to decide
/// validity (`frozen_string_literal_specified?`,
/// `shareable_constant_value_specified?`, `typed_specified?`); `keyword` is
/// interpolated as a regex fragment, matching upstream's `KEYWORDS` values.
fn simple_extract(comment: &str, keyword: &str) -> Option<()> {
    let pattern =
        Regex::new(&format!(r"(?i)\A\s*#\s*{keyword}:\s*{TOKEN}\s*\z")).expect("valid regex");
    pattern.is_match(comment).then_some(())
}

/// `SimpleComment#rbs_inline_specified?`: `valid_rbs_inline_value?`, i.e.
/// the extracted value (unanchored keyword check aside) is `enabled` or
/// `disabled`.
fn simple_rbs_inline_specified(comment: &str) -> bool {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(&format!(r"(?i)\A\s*#\s*rbs_inline:\s*({TOKEN})\s*\z")).expect("valid")
    });
    RE.captures(comment).is_some_and(|c| matches!(&c[1], "enabled" | "disabled"))
}

/// `register_offense`: reports the whole line, autocorrecting either by
/// removing the encoding token from a shared magic comment or, if nothing
/// else remains on the line, deleting the line entirely (its trailing
/// newline included).
fn register_offense(ctx: &mut Context<'_>, line_span: Span, comment: &MagicComment<'_>) {
    let text = comment.without_encoding();
    let edit = if text.is_empty() {
        Edit::delete(ctx.with_surrounding_space(line_span, Side::Right, true, false))
    } else {
        Edit::replace(line_span, text.into_bytes())
    };
    ctx.report_with_fix(
        &Encoding::META,
        line_span,
        MSG,
        Fix { applicability: Applicability::Safe, edits: vec![edit] },
    );
}
