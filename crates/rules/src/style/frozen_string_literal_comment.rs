//! `Style/FrozenStringLiteralComment`, ported from RuboCop's
//! `lib/rubocop/cop/style/frozen_string_literal_comment.rb`, together with
//! its `RuboCop::MagicComment`/`RuboCop::Cop::FrozenStringLiteral` helpers
//! (RuboCop has no per-file token stream available to us, so the "leading
//! comment lines" and "magic comment" logic below is reimplemented directly
//! against physical source lines and comment spans).

use linter::{
    Applicability, Context, Department, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use linter::{ConfigDefault, ConfigOption, Edit};
use ruby_ast::NodeExt;
use ruby_source::Span;

/// RuboCop's `MSG_MISSING_TRUE`.
const MSG_MISSING_TRUE: &str = "Missing magic comment `# frozen_string_literal: true`.";
/// RuboCop's `MSG_MISSING`.
const MSG_MISSING: &str = "Missing frozen string literal comment.";
/// RuboCop's `MSG_UNNECESSARY`.
const MSG_UNNECESSARY: &str = "Unnecessary frozen string literal comment.";
/// RuboCop's `MSG_DISABLED`.
const MSG_DISABLED: &str = "Frozen string literal comment must be set to `true`.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    /// A frozen string literal comment (`true` or `false`) must be present.
    Always,
    /// The comment must be present and set to `true`.
    AlwaysTrue,
    /// No frozen string literal comment may be present.
    Never,
}

/// The comment syntax a leading line's `frozen_string_literal` setting was
/// found in, needed to format the replacement RuboCop's `new_frozen_string_literal` writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommentSyntax {
    /// `# frozen_string_literal: true`.
    Simple,
    /// `# -*- frozen_string_literal: true -*-`.
    Emacs,
}

/// One leading line that specifies `frozen_string_literal` (RuboCop's
/// `MagicComment#frozen_string_literal_specified?`), regardless of whether
/// the value is a valid `true`/`false` literal.
struct Specified {
    line: u32,
    span: Span,
    syntax: CommentSyntax,
    /// The extracted setting text, case as written (RuboCop downcases it
    /// only when comparing against `true`/`false`).
    raw_value: String,
}

impl Specified {
    /// RuboCop's `MagicComment#frozen_string_literal?`: the setting is
    /// exactly (case-insensitively) `true`.
    fn is_true(&self) -> bool {
        self.raw_value.eq_ignore_ascii_case("true")
    }

    /// RuboCop's `MagicComment#valid_literal_value?`: the setting is `true`
    /// or `false`.
    fn is_valid(&self) -> bool {
        self.is_true() || self.raw_value.eq_ignore_ascii_case("false")
    }
}

/// Looks for the `# frozen_string_literal: true` (or `false`) magic comment.
#[derive(Debug, Clone)]
pub struct FrozenStringLiteralComment {
    style: Style,
}

/// RuboCop's `ensure_comment` (`always` style, the default): a valid
/// `true`/`false` comment must exist somewhere in the leading section.
fn ensure_comment(ctx: &mut Context<'_>, any_valid: bool) {
    if !any_valid {
        missing_offense(ctx, MSG_MISSING);
    }
}

/// RuboCop's `ensure_no_comment` (`never` style): no valid comment may exist.
fn ensure_no_comment(ctx: &mut Context<'_>, any_valid: bool, found: Option<&Specified>) {
    if any_valid {
        let target = found.expect("a valid literal implies a specified comment exists");
        unnecessary_comment_offense(ctx, target);
    }
}

/// RuboCop's `ensure_enabled_comment` (`always_true` style): the comment
/// must exist and be set to `true`.
fn ensure_enabled_comment(ctx: &mut Context<'_>, found: Option<&Specified>) {
    match found {
        None => missing_offense(ctx, MSG_MISSING_TRUE),
        Some(target) if target.is_true() => {}
        Some(target) => disabled_offense(ctx, target),
    }
}

/// RuboCop's `missing_offense`/`missing_true_offense`: a zero-width (by
/// range; one byte by RuboCop's `source_range` default length) offense
/// at the very start of the file, fixed by inserting the comment.
fn missing_offense(ctx: &mut Context<'_>, message: &'static str) {
    let span = Span::new(0, 1.min(u32::try_from(ctx.source().bytes().len()).unwrap_or(1)));
    let fix = insert_comment_fix(ctx);
    ctx.report_with_fix(&FrozenStringLiteralComment::META, span, message, fix);
}

/// RuboCop's `unnecessary_comment_offense`: the comment is removed
/// (along with its trailing whitespace and the newline(s) that follow).
fn unnecessary_comment_offense(ctx: &mut Context<'_>, target: &Specified) {
    let fix = remove_comment_fix(ctx, target.span);
    ctx.report_with_fix(&FrozenStringLiteralComment::META, target.span, MSG_UNNECESSARY, fix);
}

/// RuboCop's `disabled_offense`: the comment's line is replaced with a
/// correctly-enabled one.
fn disabled_offense(ctx: &mut Context<'_>, target: &Specified) {
    let fix = enable_comment_fix(ctx, target);
    ctx.report_with_fix(&FrozenStringLiteralComment::META, target.span, MSG_DISABLED, fix);
}

impl Rule for FrozenStringLiteralComment {
    const META: RuleMeta = RuleMeta {
        name: "Style/FrozenStringLiteralComment",
        department: Department::Style,
        summary: "Add the frozen_string_literal comment to the top of files to help transition \
            to frozen string literals by default.",
        explanation: "\
Helps you transition from mutable string literals to frozen string
literals. It will add the `# frozen_string_literal: true` magic comment to
the top of files to enable frozen string literals. Frozen string literals
may be default in future Ruby. The comment will be added below a shebang
and encoding comment. The frozen string literal comment is only valid in
Ruby 2.3+.

Note that the cop will accept files where the comment exists but is set to
`false` instead of `true` -- unless `EnforcedStyle: always_true` is used.

To require a blank line after this comment, see
`Layout/EmptyLineAfterMagicComment`.

```ruby
# EnforcedStyle: always (default)

# bad
module Bar
  # ...
end

# good
# frozen_string_literal: true

module Bar
  # ...
end

# good
# frozen_string_literal: false

module Bar
  # ...
end
```

```ruby
# EnforcedStyle: never

# bad
# frozen_string_literal: true

module Baz
  # ...
end

# good
module Baz
  # ...
end
```

```ruby
# EnforcedStyle: always_true

# bad
# frozen_string_literal: false

module Baz
  # ...
end

# bad
module Baz
  # ...
end

# good
# frozen_string_literal: true

module Bar
  # ...
end
```

Autocorrection is unsafe: any string mutation will change from being
accepted to raising `FrozenError`, since all strings become frozen by
default, and will need to be manually refactored.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("always"),
            allowed: &["always", "always_true", "never"],
            doc: "Whether a `frozen_string_literal` comment is required, required and must be \
                `true`, or forbidden.",
        }],
        blind_spots: "\
Comments are read straight from physical source lines (RuboCop's own
`processed_source.tokens` is unavailable to us), so a file's leading
section is bounded by the first top-level statement's line rather than by
an actual non-comment token; this matches every realistic file but not,
say, one starting with a `BEGIN {}` block. Vim-style magic comments
(`# vim: ...`) are recognised but -- matching RuboCop's own
`VimComment#frozen_string_literal` -- never treated as specifying
`frozen_string_literal`. RuboCop's Ruby-version-gated \"frozen by default\"
fallback (relevant only to hypothetical future Ruby versions) is not
modelled; we match RuboCop's own current behaviour of treating it as
`false`. A `__END__` data section is not excluded when locating the first
top-level statement, unlike `Layout/TrailingWhitespace`.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "always_true" => Style::AlwaysTrue,
            "never" => Style::Never,
            _ => Style::Always,
        };
        Ok(Self { style })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        // RuboCop's `return if processed_source.tokens.empty?`: a file with
        // no actual content (blank, or literally empty) has no tokens at
        // all -- not even a comment -- so the cop does nothing.
        if ctx.source().bytes().iter().all(u8::is_ascii_whitespace) {
            return;
        }

        let leading_end = leading_end_line(ctx);
        let mut any_valid = false;
        let mut first_specified: Option<Specified> = None;
        for line in 1..=leading_end {
            let Some(specified) = parse_leading_line(ctx, line) else { continue };
            any_valid |= specified.is_valid();
            if first_specified.is_none() {
                first_specified = Some(specified);
            }
        }

        match self.style {
            Style::Always => ensure_comment(ctx, any_valid),
            Style::Never => ensure_no_comment(ctx, any_valid, first_specified.as_ref()),
            Style::AlwaysTrue => ensure_enabled_comment(ctx, first_specified.as_ref()),
        }
    }
}

/// RuboCop's `FrozenStringLiteral#leading_comment_lines`: every physical
/// line up to (but not including) the line of the first top-level
/// statement, or every line in the file when there is none.
fn leading_end_line(ctx: &Context<'_>) -> u32 {
    let program = ctx.parsed().root().as_program_node().expect("Prism root is a ProgramNode");
    let body = program.statements().body();
    match body.iter().next() {
        Some(first) => ctx.line_col(first.span().start).line.saturating_sub(1),
        None => ctx.line_count(),
    }
}

/// Parses one leading line for a `frozen_string_literal` setting (RuboCop's
/// `MagicComment.parse(line).frozen_string_literal_specified?`), returning
/// `None` when the line does not specify one at all (blank, a comment
/// without the key, an Emacs/Vim comment without the key, ...).
fn parse_leading_line(ctx: &Context<'_>, line: u32) -> Option<Specified> {
    let text = ctx.line_text(line);
    let text = std::str::from_utf8(text).ok()?;

    let (syntax, raw_value) = match emacs_match(text) {
        EmacsMatch::NotEmacs if is_vim_comment(text) => return None,
        EmacsMatch::NotEmacs => (CommentSyntax::Simple, match_simple(text)?),
        EmacsMatch::NoKey => return None,
        EmacsMatch::Found(value) => (CommentSyntax::Emacs, value),
    };
    let span = comment_span_for_line(ctx, line)?;
    Some(Specified { line, span, syntax, raw_value })
}

/// The span of the (single, full-line) comment on `line`, if any.
fn comment_span_for_line(ctx: &Context<'_>, line: u32) -> Option<Span> {
    ctx.comments().iter().find(|c| c.line == line).map(|c| c.span)
}

/// RuboCop's `KEYWORDS[:frozen_string_literal]` (`frozen[_-]string[_-]literal`)
/// matched at the start of `s`. Returns the byte length consumed.
fn match_frozen_keyword(s: &str) -> Option<usize> {
    let mut idx = 0;
    for (i, word) in ["frozen", "string", "literal"].into_iter().enumerate() {
        if i > 0 {
            let sep = s[idx..].chars().next()?;
            if sep != '_' && sep != '-' {
                return None;
            }
            idx += sep.len_utf8();
        }
        if !s[idx..].starts_with(word) {
            return None;
        }
        idx += word.len();
    }
    Some(idx)
}

/// The run of `[[:alnum:]\-_]` characters starting at `s`, if non-empty.
fn take_token(s: &str) -> Option<&str> {
    let end = s
        .char_indices()
        .take_while(|&(_, c)| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        .last()
        .map_or(0, |(i, c)| i + c.len_utf8());
    (end > 0).then(|| &s[..end])
}

/// RuboCop's `SimpleComment#extract_frozen_string_literal`:
/// `\A\s*#\s*frozen[_-]string[_-]literal:\s*TOKEN\s*\z`, case-insensitive on
/// the keyword.
fn match_simple(line: &str) -> Option<String> {
    let after_hash = line.trim_start().strip_prefix('#')?;
    let after_ws = after_hash.trim_start();
    let lower: String = after_ws.chars().map(|c| c.to_ascii_lowercase()).collect();
    let key_len = match_frozen_keyword(&lower)?;
    let after_colon = after_ws[key_len..].strip_prefix(':')?;
    let after_colon_ws = after_colon.trim_start();
    let token = take_token(after_colon_ws)?;
    let rest = &after_colon_ws[token.len()..];
    rest.chars().all(char::is_whitespace).then(|| token.to_string())
}

/// Result of checking a line against RuboCop's Emacs-style magic comment
/// wrapper (`-\*-...-\*-`).
enum EmacsMatch {
    /// `line` does not match the Emacs wrapper at all; Vim/Simple parsing applies.
    NotEmacs,
    /// `line` is Emacs-style but has no `frozen_string_literal` key.
    NoKey,
    /// `line` is Emacs-style and specifies `frozen_string_literal` as this
    /// (case as written) value.
    Found(String),
}

/// RuboCop's `MagicComment.parse` dispatch to `EmacsComment` plus its
/// `#frozen_string_literal`.
fn emacs_match(line: &str) -> EmacsMatch {
    let Some(first) = line.find("-*-") else { return EmacsMatch::NotEmacs };
    let after_first = first + 3;
    let Some(last_rel) = line[after_first..].rfind("-*-") else { return EmacsMatch::NotEmacs };
    if last_rel == 0 {
        // No non-empty capture between the two markers: Ruby's `.+` needs
        // at least one character, so the Emacs regexp would not match.
        return EmacsMatch::NotEmacs;
    }
    let captured = &line[after_first..after_first + last_rel];
    for token in captured.split(';') {
        let token = token.trim();
        // RuboCop's `EditorComment#match` builds its pattern from the raw
        // keyword fragment with no `/i` flag: case-sensitive.
        let Some(key_len) = match_frozen_keyword(token) else { continue };
        let Some(after_colon) = token[key_len..].strip_prefix(':') else { continue };
        let after_ws = after_colon.trim_start();
        let Some(value) = take_token(after_ws) else { continue };
        if value.len() == after_ws.len() {
            return EmacsMatch::Found(value.to_string());
        }
    }
    EmacsMatch::NoKey
}

/// RuboCop's `VimComment`: `#\s*vim:\s*.+` anywhere in the line. Vim
/// comments never specify `frozen_string_literal`.
fn is_vim_comment(line: &str) -> bool {
    let Some(hash) = line.find('#') else { return false };
    let Some(after_vim) = line[hash + 1..].trim_start().strip_prefix("vim:") else {
        return false;
    };
    !after_vim.trim_start().is_empty()
}

/// RuboCop's `Encoding::ENCODING_PATTERN`: `#.*coding\s?[:=]\s?(?:UTF|utf)-8`.
fn contains_encoding_pattern(line: &str) -> bool {
    let Some(hash) = line.find('#') else { return false };
    let rest = &line[hash..];
    let mut search_from = 0;
    while let Some(pos) = rest[search_from..].find("coding") {
        let after_coding = search_from + pos + "coding".len();
        if matches_coding_suffix(&rest[after_coding..]) {
            return true;
        }
        search_from += pos + 1;
    }
    false
}

/// `\s?[:=]\s?(?:UTF|utf)-8` starting at `s`.
fn matches_coding_suffix(s: &str) -> bool {
    let mut rest = s;
    if let Some(c) = rest.chars().next().filter(|c| c.is_whitespace()) {
        rest = &rest[c.len_utf8()..];
    }
    let Some(c) = rest.chars().next() else { return false };
    if c != ':' && c != '=' {
        return false;
    }
    rest = &rest[c.len_utf8()..];
    if let Some(c) = rest.chars().next().filter(|c| c.is_whitespace()) {
        rest = &rest[c.len_utf8()..];
    }
    rest.starts_with("UTF-8") || rest.starts_with("utf-8")
}

/// Whether `line` has any non-whitespace content.
fn has_content(ctx: &Context<'_>, line: u32) -> bool {
    !ctx.line_text(line).iter().all(u8::is_ascii_whitespace)
}

/// The next line after `after` with any content, if any.
fn next_content_line(ctx: &Context<'_>, after: u32) -> Option<u32> {
    ((after + 1)..=ctx.line_count()).find(|&line| has_content(ctx, line))
}

/// RuboCop's `last_special_comment`: the shebang line and/or the encoding
/// comment immediately following it (or leading the file, absent a
/// shebang), whichever is found last -- i.e. the encoding comment wins over
/// the shebang when both are present.
fn insert_after_line(ctx: &Context<'_>) -> Option<u32> {
    let first = (1..=ctx.line_count()).find(|&line| has_content(ctx, line))?;
    let mut shebang = None;
    let mut check_line = Some(first);
    if ctx.line_text(first).starts_with(b"#!") {
        shebang = Some(first);
        check_line = next_content_line(ctx, first);
    }
    if let Some(check_line) = check_line {
        let text = ctx.line_text(check_line);
        if let Ok(text) = std::str::from_utf8(text) {
            if contains_encoding_pattern(text) {
                return Some(check_line);
            }
        }
    }
    shebang
}

/// RuboCop's `insert_comment`: after the shebang/encoding line if either is
/// present, else before the whole file.
fn insert_comment_fix(ctx: &Context<'_>) -> Fix {
    let edit = match insert_after_line(ctx) {
        Some(line) => {
            Edit::insert(ctx.line_span(line).end, b"\n# frozen_string_literal: true".as_slice())
        }
        None => Edit::insert(0, b"# frozen_string_literal: true\n".as_slice()),
    };
    Fix { applicability: Applicability::Unsafe, edits: vec![edit] }
}

/// RuboCop's `remove_comment`: the comment plus any trailing `[ \t]`, plus
/// the newline(s) that follow (including further fully-blank lines).
fn remove_comment_fix(ctx: &Context<'_>, comment: Span) -> Fix {
    let bytes = ctx.source().bytes();
    let mut end = comment.end as usize;
    while bytes.get(end).is_some_and(|&b| b == b' ' || b == b'\t') {
        end += 1;
    }
    while bytes.get(end) == Some(&b'\n') {
        end += 1;
    }
    let range = Span::new(comment.start, u32::try_from(end).unwrap_or(comment.end));
    Fix { applicability: Applicability::Unsafe, edits: vec![Edit::delete(range)] }
}

/// RuboCop's `enable_comment`: the whole line is replaced with a freshly
/// formatted, enabled comment (`MagicComment#new_frozen_string_literal`),
/// discarding whatever else the original line's magic comment specified.
fn enable_comment_fix(ctx: &Context<'_>, target: &Specified) -> Fix {
    let replacement = match target.syntax {
        CommentSyntax::Simple => "# frozen_string_literal: true".to_string(),
        CommentSyntax::Emacs => "# -*- frozen_string_literal: true -*-".to_string(),
    };
    let span = ctx.line_span(target.line);
    Fix {
        applicability: Applicability::Unsafe,
        edits: vec![Edit::replace(span, replacement.into_bytes())],
    }
}
