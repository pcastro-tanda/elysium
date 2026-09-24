//! `# rubocop:disable`/`enable`/`todo` directive comments.
//!
//! Ports RuboCop's `RuboCop::DirectiveComment` (the per-comment directive
//! parser) and `RuboCop::CommentConfig` (the per-line "is this cop disabled
//! here" index) from `lib/rubocop/directive_comment.rb` and
//! `lib/rubocop/comment_config.rb` (RuboCop 1.82.1).
//!
//! This crate is parser-independent: [`Directives::parse`] takes plain
//! `(Span, &[u8])` comment pairs, so any front end can feed it. Callers that
//! already hold a [`ruby_ast::Parsed`] tree can use [`Directives::from_parsed`]
//! instead.
//!
//! ## Deliberate deviations from upstream RuboCop
//!
//! - **`push`/`pop` directives are not recognized.** RuboCop 1.82 added a
//!   `# rubocop:push` / `# rubocop:pop` stack mechanism. A comment using
//!   either mode is simply not treated as a directive at all (as if the
//!   comment did not start with `# rubocop:`). Nothing in this phase's
//!   contract calls for push/pop support and [`DirectiveKind`] has no variant
//!   for them.
//! - **No cop registry, so `Lint/Syntax` and `Lint/RedundantCopDisableDirective`
//!   are not special-cased.** Upstream silently drops `Lint/Syntax` from
//!   *any* directive (even one that names it directly) and drops
//!   `Lint/RedundantCopDisableDirective` specifically from `all`/`Lint`
//!   department expansions, because those two cops are wired directly into
//!   RuboCop's cop registry and team-runner. This crate has no registry and
//!   no knowledge of specific cop names, so both cops behave like any other
//!   cop name here. The cops that care about this (implemented later in
//!   `rules`) are expected to special-case it themselves.
//! - **`DirectiveComment#malformed?`/`#missing_cop_name?` are not ported.**
//!   They exist upstream only to power `Lint/RedundantCopDisableDirective`
//!   diagnostics, which is out of scope for this phase; [`Directives`] simply
//!   ignores any trailing text after a recognized cop list (including a
//!   `-- reason` suffix), matching the *parsing* behaviour but not upstream's
//!   "is this malformed" diagnostic.
//! - **Inline detection is byte-positional, not comment-text-positional.**
//!   Upstream's `DirectiveComment#single_line?` checks whether the
//!   *comment's own text* starts with the directive marker (so
//!   `#=SomeDslDirective # rubocop:disable Foo` is inline even though it is
//!   the only thing on its physical line, because the directive marker is
//!   not at the very start of the comment token). Per this phase's contract,
//!   [`Directive::inline`] instead checks whether the *physical source line*
//!   has any non-whitespace byte before the comment starts. The two agree
//!   for the overwhelming majority of real directive comments (a directive
//!   is essentially always either the entire comment or nothing in it is
//!   before the marker); they differ only for the contrived case above.
//! - **Department membership is a generic prefix match, not a registry
//!   lookup.** Upstream resolves `department?(name)` and
//!   `names_for_department(name)` against the live cop registry. Without a
//!   registry, any cop reference (`Department` or a slash-qualified `Cop`)
//!   is treated as matching a query cop name that equals it, or that has it
//!   as a `name/`-prefixed ancestor. This reproduces the single-level case
//!   the contract calls out (`Style` disables `Style/Foo`) and, as a
//!   harmless side effect, also handles multi-level department prefixes
//!   (e.g. `rubocop-rspec`'s `RSpec/Rails` disabling `RSpec/Rails/HttpStatus`)
//!   without needing to special-case them.
//! - **`disable-next`/`todo-next`/`enable-next` scope to one physical line,
//!   not a full AST statement.** Upstream (`RuboCop::CommentConfig::DisableNext`,
//!   added in RuboCop 1.91) walks the parsed statement following a `-next`
//!   directive comment so a multi-line statement, a `when` clause, or a
//!   heredoc is disabled in its entirety, honors the directive only on a
//!   comment-only line, and skips past blank lines and other comment-only
//!   lines while tracking directives that end up attached to nothing. This
//!   crate has no such statement-scope notion (and no "detached directive"
//!   bookkeeping, which upstream only collects for diagnostics this phase
//!   doesn't implement): every `-next` directive instead scopes to exactly
//!   the one physical source line right after the comment's own line,
//!   whether the comment sits alone on its line or trails code on it. This
//!   matches upstream whenever the following statement happens to be a
//!   single physical line (the common case) and otherwise under- or
//!   over-disables relative to the true statement extent.
//! - **No `push`/`pop`/bare `next` support.** RuboCop 1.91 also added a
//!   `# rubocop:push`/`# rubocop:pop` stack and a signed bare
//!   `# rubocop:next +Cop -Cop` directive built on the same push/pop
//!   machinery. Neither is recognized here for the same reason `push`/`pop`
//!   themselves are not (see above): a comment using any of these modes is
//!   simply not treated as a directive at all.

use std::sync::LazyLock;

use regex::bytes::Regex;
use ruby_ast::{LocationExt, Parsed};
use ruby_source::{SourceFile, Span};

/// The recognized `# rubocop:<mode> ...` modes.
///
/// RuboCop's `todo` mode is functionally identical to `disable`; it exists
/// only so tooling that auto-inserts directives (`rubocop --auto-gen-config`)
/// can tell its own generated directives apart from hand-written ones. This
/// crate keeps the distinction in the type for that reason, but treats
/// [`DirectiveKind::Todo`] exactly like [`DirectiveKind::Disable`] everywhere.
/// Likewise [`DirectiveKind::TodoNext`] is treated exactly like
/// [`DirectiveKind::DisableNext`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DirectiveKind {
    /// `# rubocop:disable ...`
    Disable,
    /// `# rubocop:enable ...`
    Enable,
    /// `# rubocop:todo ...` (equivalent to `Disable`).
    Todo,
    /// `# rubocop:disable-next ...`. See the module docs for how this
    /// crate approximates upstream's AST-statement-scoped "next" directives
    /// with a single physical line.
    DisableNext,
    /// `# rubocop:todo-next ...` (equivalent to `DisableNext`).
    TodoNext,
    /// `# rubocop:enable-next ...`
    EnableNext,
}

impl DirectiveKind {
    /// True for `Disable`, `Todo`, and `DisableNext`/`TodoNext`, matching
    /// upstream `DirectiveComment#disabled?`.
    #[must_use]
    pub const fn disables(self) -> bool {
        !matches!(self, Self::Enable | Self::EnableNext)
    }

    /// True for the three `-next` modes, matching upstream
    /// `DirectiveComment#disable_next?` (for `DisableNext`/`TodoNext`) and
    /// `#enable_next?` combined.
    #[must_use]
    pub const fn is_next(self) -> bool {
        matches!(self, Self::DisableNext | Self::TodoNext | Self::EnableNext)
    }
}

/// One cop reference named by a directive comment.
///
/// Which variant a bare word becomes is a syntactic guess (no cop registry
/// is available): a token containing `/` is a [`CopRef::Cop`], a bare token
/// is a [`CopRef::Department`], and the literal keyword `all` is
/// [`CopRef::All`]. See the module docs for how matching treats these.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CopRef {
    /// Every cop (`# rubocop:disable all`).
    All,
    /// A bare department name, e.g. `Style` in `# rubocop:disable Style`.
    Department(String),
    /// A slash-qualified cop (or nested department) path, e.g. `Style/Foo`.
    Cop(String),
}

impl CopRef {
    /// True when this reference covers `cop_name`.
    ///
    /// `All` covers everything. A `Department`/`Cop` reference covers
    /// `cop_name` when it names it exactly, or when it is a `/`-qualified
    /// ancestor of it (see the module docs for why `Cop` gets prefix
    /// matching too).
    #[must_use]
    pub fn covers(&self, cop_name: &str) -> bool {
        match self {
            Self::All => true,
            Self::Department(name) | Self::Cop(name) => {
                cop_name == name.as_str() || cop_name.starts_with(&format!("{name}/"))
            }
        }
    }
}

/// One parsed `# rubocop:disable|enable|todo ...` directive comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Directive {
    /// Byte span of the recognized directive text within the comment (the
    /// `# rubocop : mode ...cops` portion only, not any trailing free text).
    pub span: Span,
    /// 1-based source line the comment starts on.
    pub line: u32,
    /// Which mode was used.
    pub kind: DirectiveKind,
    /// The cops named by this directive, in written order. Empty when the
    /// directive named no cops at all (e.g. a bare `# rubocop:disable` with
    /// no cop list), in which case the directive has no effect.
    pub cops: Vec<CopRef>,
    /// True when this comment is an end-of-line directive: something other
    /// than whitespace precedes it on its physical source line. Inline
    /// directives affect only their own line; own-line directives affect
    /// every line from themselves to a matching `enable` (or end of file).
    pub inline: bool,
}

/// The directive comments found in one file, and the disabled/enabled state
/// they imply at any given line.
///
/// Mirrors RuboCop's `CommentConfig#cop_enabled_at_line?` /
/// `#cop_disabled_line_ranges`.
#[derive(Debug, Clone, Default)]
pub struct Directives {
    directives: Vec<Directive>,
}

// Ported from RuboCop::DirectiveComment (directive_comment.rb):
//   DIRECTIVE_MARKER_PATTERN = '# rubocop : '  (every space -> \s*)
//   DIRECTIVE_HEADER_PATTERN = marker + "(disable-next|todo-next|enable-next|disable|enable|todo)\b"
//   COP_NAME_PATTERN = ([A-Za-z]\w+/)*(?:[A-Za-z]\w+)
//   COPS_PATTERN = (all|(?:COP_NAME_PATTERN , )*COP_NAME_PATTERN)
// `push`/`pop`/bare `next` modes are intentionally omitted; see module docs.
// Modes are listed longest-first in the alternation (matching upstream's
// `AVAILABLE_MODES.sort_by { |mode| -mode.length }` comment: "Longest first,
// so a `-next` mode is not matched as its prefix"): the `regex` crate picks
// the first alternative that matches at a position, not the longest, so
// `disable-next Foo` would otherwise match mode `disable` and leave a
// dangling `-next Foo` that fails the optional cops group.
// `(?-u)` restricts \w/\s/\b to ASCII, matching Ruby's default \w for cop
// identifiers (which are always ASCII) and avoiding any UTF-8-boundary
// subtleties on raw comment bytes.
static DIRECTIVE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?-u)\#\s*rubocop\s*:\s*(?P<mode>disable-next|todo-next|enable-next|disable|enable|todo)\b(?:\s+(?P<cops>all|(?:[A-Za-z]\w+/)*[A-Za-z]\w+(?:\s*,\s*(?:[A-Za-z]\w+/)*[A-Za-z]\w+)*))?",
    )
    .expect("static directive regex is valid")
});

fn classify_cop(token: &str) -> CopRef {
    if token.contains('/') {
        CopRef::Cop(token.to_string())
    } else {
        CopRef::Department(token.to_string())
    }
}

fn parse_cops(raw: &[u8]) -> Vec<CopRef> {
    let raw = std::str::from_utf8(raw).unwrap_or("");
    if raw == "all" {
        return vec![CopRef::All];
    }
    raw.split(',').map(str::trim).filter(|t| !t.is_empty()).map(classify_cop).collect()
}

fn is_inline(source: &SourceFile, comment_start: u32) -> bool {
    let line = source.line_col(comment_start).line;
    let line_start = source.lines().line_start(line);
    let prefix = &source.bytes()[line_start as usize..comment_start as usize];
    prefix.iter().any(|b| !b.is_ascii_whitespace())
}

fn parse_comment(source: &SourceFile, span: Span, text: &[u8]) -> Option<Directive> {
    let captures = DIRECTIVE_REGEX.captures(text)?;
    let kind = match captures.name("mode")?.as_bytes() {
        b"disable" => DirectiveKind::Disable,
        b"enable" => DirectiveKind::Enable,
        b"todo" => DirectiveKind::Todo,
        b"disable-next" => DirectiveKind::DisableNext,
        b"todo-next" => DirectiveKind::TodoNext,
        b"enable-next" => DirectiveKind::EnableNext,
        _ => return None,
    };
    let cops = captures.name("cops").map(|m| parse_cops(m.as_bytes())).unwrap_or_default();
    let whole = captures.get(0).expect("regex match always has a full match");
    let directive_span = Span::new(
        span.start + u32::try_from(whole.start()).unwrap_or(u32::MAX),
        span.start + u32::try_from(whole.end()).unwrap_or(u32::MAX),
    );
    let line = source.line_col(span.start).line;
    let inline = is_inline(source, span.start);
    Some(Directive { span: directive_span, line, kind, cops, inline })
}

impl Directives {
    /// Parses directive comments out of `comments`.
    ///
    /// Each item is one comment's byte span (used to compute its 1-based
    /// line and inline/own-line status) paired with its raw text (including
    /// the leading `#`).
    pub fn parse<'a>(
        source: &SourceFile,
        comments: impl Iterator<Item = (Span, &'a [u8])>,
    ) -> Self {
        let directives =
            comments.filter_map(|(span, text)| parse_comment(source, span, text)).collect();
        Self { directives }
    }

    /// Convenience wrapper over [`Directives::parse`] for a Prism [`Parsed`] tree.
    #[must_use]
    pub fn from_parsed(parsed: &Parsed<'_>) -> Self {
        let source = parsed.source();
        let comments: Vec<_> = parsed.comments().collect();
        Self::parse(source, comments.iter().map(|c| (c.location().span(), c.text())))
    }

    /// Every directive found, in source order.
    #[must_use]
    pub fn directives(&self) -> &[Directive] {
        &self.directives
    }

    /// Builds the inclusive `(start_line, end_line)` ranges (RuboCop's
    /// `CopAnalysis#line_ranges`) covered by directives matching `covers`, by
    /// replaying them in source order. `end_line` is `u32::MAX` for a range
    /// left open through end of file.
    ///
    /// Mirrors `CommentConfig#analyze`'s per-cop state machine
    /// (`analyze_single_line`/`analyze_disabled`/`analyze_rest`), specialized
    /// to one already-filtered directive stream instead of a
    /// registry-expanded one.
    fn disabled_ranges(&self, covers: impl Fn(&Directive) -> bool) -> Vec<(u32, u32)> {
        let mut ranges = Vec::new();
        let mut start: Option<u32> = None;
        for directive in self.directives.iter().filter(|d| covers(d)) {
            if directive.kind.is_next() {
                // apply_disable_next/apply_enable_next, simplified to the
                // one physical line after the comment (see module docs):
                // `inline` is irrelevant here, unlike the plain modes below.
                let target = directive.line + 1;
                if directive.kind.disables() {
                    // add_next_range: an independent one-off range that
                    // doesn't disturb any already-open `start`.
                    ranges.push((target, target));
                } else if let Some(s) = start {
                    // suspend_disable: punch the target line out of the
                    // open range (a no-op unless something is open).
                    if target > s {
                        ranges.push((s, target - 1));
                    }
                    start = Some(target + 1);
                }
            } else if directive.inline {
                // analyze_single_line: an inline `enable` has no effect; an
                // inline disable/todo affects only its own line.
                if directive.kind.disables() {
                    ranges.push((directive.line, directive.line));
                }
            } else if directive.kind.disables() {
                // analyze_disabled: close any already-open range at this
                // line (the "double disable" case), then (re)open here.
                if let Some(s) = start {
                    ranges.push((s, directive.line));
                }
                start = Some(directive.line);
            } else {
                // analyze_rest: an own-line `enable` closes an open range.
                if let Some(s) = start.take() {
                    ranges.push((s, directive.line));
                }
            }
        }
        if let Some(s) = start {
            ranges.push((s, u32::MAX));
        }
        ranges
    }

    /// True when `cop_name` is disabled at `line`, matching RuboCop's
    /// `CommentConfig#cop_enabled_at_line?` (negated).
    #[must_use]
    pub fn is_disabled(&self, cop_name: &str, line: u32) -> bool {
        self.disabled_ranges(|d| d.cops.iter().any(|c| c.covers(cop_name)))
            .into_iter()
            .any(|(s, e)| line >= s && line <= e)
    }

    /// Like [`Self::is_disabled`], but ignores `# rubocop:disable all` coverage: only an
    /// explicit mention of `cop_name` (or its department) counts. `Lint/RedundantCopDisableDirective`
    /// is excluded from `all`/`Lint` department expansion by RuboCop's own registry
    /// (`DirectiveComment#exclude_lint_department_cops`), so `all` never actually silences it,
    /// even though this crate's registry-less [`CopRef::All`] otherwise `covers` every name.
    #[must_use]
    pub fn is_disabled_by_name(&self, cop_name: &str, line: u32) -> bool {
        self.disabled_ranges(|d| {
            d.cops.iter().any(|c| !matches!(c, CopRef::All) && c.covers(cop_name))
        })
        .into_iter()
        .any(|(s, e)| line >= s && line <= e)
    }

    /// True when a `# rubocop:disable all` (or `todo all`) directive covers
    /// `line`.
    #[must_use]
    pub fn all_disabled_at(&self, line: u32) -> bool {
        self.disabled_ranges(|d| d.cops.iter().any(|c| matches!(c, CopRef::All)))
            .into_iter()
            .any(|(s, e)| line >= s && line <= e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directives_for(source: &str) -> Directives {
        let file = SourceFile::new("test.rb", source.as_bytes());
        let parsed = Parsed::parse(&file);
        Directives::from_parsed(&parsed)
    }

    // Ports of RuboCop::CommentConfig spec cases
    // (spec/rubocop/comment_config_spec.rb, RuboCop 1.82.1).

    #[test]
    fn own_line_disable_enable_pair_covers_inclusive_range() {
        // "supports disabling multiple lines with a pair of directive"
        let d = directives_for(
            "# rubocop:disable Metrics/MethodLength with a comment why\ndef some_method\n  puts 'foo'\nend\n# rubocop:enable Metrics/MethodLength\n",
        );
        for line in 1..=5 {
            assert!(d.is_disabled("Metrics/MethodLength", line), "line {line} should be disabled");
        }
        assert!(!d.is_disabled("Metrics/MethodLength", 6));
    }

    #[test]
    fn multiple_cops_in_a_single_directive_share_the_same_range() {
        // "supports enabling/disabling multiple cops in a single directive"
        let d = directives_for(
            "# rubocop:disable Style/For, Style/Not,Layout/IndentationStyle\nfoo\n# rubocop:enable Style/Not,Layout/IndentationStyle\n",
        );
        for cop in ["Style/Not", "Layout/IndentationStyle"] {
            assert!(d.is_disabled(cop, 1));
            assert!(d.is_disabled(cop, 3));
            assert!(!d.is_disabled(cop, 4));
        }
    }

    #[test]
    fn trailing_free_text_after_cop_list_does_not_break_parsing() {
        // "supports enabling/disabling multiple cops along with a comment"
        let d = directives_for(
            "# rubocop:disable Style/Send, Lint/RandOne some comment why\nfoo\n# rubocop:enable Style/Send, Layout/BlockAlignment but why?\nbar\n",
        );
        assert!(d.is_disabled("Style/Send", 1));
        assert!(d.is_disabled("Lint/RandOne", 2));
        // The enable line itself stays inside the disabled range (matches
        // upstream's inclusive `start_line..line` on the closing directive).
        assert!(d.is_disabled("Style/Send", 3));
        assert!(!d.is_disabled("Style/Send", 4));
        // Layout/BlockAlignment was only ever named on the enable line, so
        // it was never disabled in the first place.
        assert!(!d.is_disabled("Layout/BlockAlignment", 1));
    }

    #[test]
    fn multi_level_department_cop_name_is_matched_exactly() {
        // "supports disabling cops with multiple levels in department name"
        let d = directives_for(
            "# rubocop:disable RSpec/Rails/HttpStatus\nit { is_expected.to have_http_status 200 }\n# rubocop:enable RSpec/Rails/HttpStatus\n",
        );
        assert!(d.is_disabled("RSpec/Rails/HttpStatus", 1));
        assert!(d.is_disabled("RSpec/Rails/HttpStatus", 2));
        assert!(!d.is_disabled("RSpec/Rails/HttpStatus", 4));
    }

    #[test]
    fn own_line_pair_disables_named_cop() {
        // "supports enabling/disabling cops without a prefix"
        let d = directives_for(
            "# rubocop:disable Lint/EmptyInterpolation\n\"result is #{}\"\n# rubocop:enable Lint/EmptyInterpolation\n",
        );
        assert!(d.is_disabled("Lint/EmptyInterpolation", 1));
        assert!(d.is_disabled("Lint/EmptyInterpolation", 2));
        assert!(!d.is_disabled("Lint/EmptyInterpolation", 4));
    }

    #[test]
    fn unpaired_disable_covers_through_end_of_file() {
        // "supports disabling all lines after a directive"
        let d = directives_for("# rubocop:disable Style/For\nfoo\n");
        assert!(d.is_disabled("Style/For", 2));
        assert!(d.is_disabled("Style/For", 10_000));
    }

    #[test]
    fn unpaired_enable_directive_is_ignored() {
        // "just ignores unpaired enabling directives"
        let d = directives_for("# rubocop:enable Lint/Void\nfoo\n");
        for line in 1..=1000 {
            assert!(!d.is_disabled("Lint/Void", line));
        }
    }

    #[test]
    fn inline_directive_disables_only_its_own_line() {
        // "supports disabling single line with a directive at end of line"
        let d = directives_for(
            "code = 'x'\neval(code) # rubocop:disable Security/Eval\nputs 'not evil'\n",
        );
        assert!(d.is_disabled("Security/Eval", 2));
        assert!(!d.is_disabled("Security/Eval", 3));
        assert!(!d.is_disabled("Security/Eval", 1));
    }

    #[test]
    fn indented_inline_directive_disables_only_its_own_line() {
        // "handles indented single line"
        let d = directives_for(
            "def some_method\n  puts 'x' # rubocop:disable Layout/LineLength\nend\n",
        );
        assert!(d.is_disabled("Layout/LineLength", 2));
        assert!(!d.is_disabled("Layout/LineLength", 3));
    }

    #[test]
    fn all_keyword_disables_every_cop_in_range() {
        // "supports disabling all cops except ... with keyword all"
        let d = directives_for("# rubocop:disable all\nsome_method\n# rubocop:enable all\n");
        for cop in ["Style/For", "Lint/Void", "Whatever/Anything"] {
            assert!(d.is_disabled(cop, 1));
            assert!(d.is_disabled(cop, 2));
        }
        assert!(!d.is_disabled("Style/For", 4));
        assert!(d.all_disabled_at(2));
        assert!(!d.all_disabled_at(4));
    }

    #[test]
    fn cop_name_containing_substring_all_is_not_confused_with_all_keyword() {
        // "does not confuse a cop name including 'all' with all cops"
        let d = directives_for("foo # rubocop:disable Style/MethodCallWithoutArgsParentheses\n");
        assert!(d.is_disabled("Style/MethodCallWithoutArgsParentheses", 1));
        assert!(!d.is_disabled("Alias", 1));
        assert!(!d.all_disabled_at(1));
    }

    #[test]
    fn double_disable_without_intervening_enable_reopens_from_second_line() {
        // "can handle double disable of one cop"
        let d = directives_for(
            "# rubocop:disable Style/ClassVars\n@@a = 1\n# rubocop:disable Style/ClassVars\n@@b = 2\n",
        );
        // First disable (line 1) is implicitly closed by the second disable
        // (line 3): [1..3] gets disabled, then re-opened from line 3 onward.
        assert!(d.is_disabled("Style/ClassVars", 1));
        assert!(d.is_disabled("Style/ClassVars", 2));
        assert!(d.is_disabled("Style/ClassVars", 3));
        assert!(d.is_disabled("Style/ClassVars", 100));
    }

    #[test]
    fn cop_name_with_digits_is_recognized() {
        // "supports disabling cops with numbers in their name"
        let d = directives_for("# rubocop:disable Custom2/Number9\nfoo\n");
        assert!(d.is_disabled("Custom2/Number9", 2));
    }

    #[test]
    fn typo_marker_is_not_recognized_as_a_directive() {
        // directive_comment_spec.rb "#match_captures when typo" -> nil
        let d = directives_for("# rudocop:todo Dig/ThisMine\nfoo\n");
        assert!(d.directives().is_empty());
        assert!(!d.is_disabled("Dig/ThisMine", 1));
    }

    #[test]
    fn reason_suffix_after_double_dash_does_not_break_parsing() {
        let d = directives_for("# rubocop:disable Foo/Bar -- because reasons\nfoo\n");
        assert_eq!(d.directives().len(), 1);
        assert_eq!(d.directives()[0].cops, vec![CopRef::Cop("Foo/Bar".to_string())]);
        assert!(d.is_disabled("Foo/Bar", 2));
    }

    #[test]
    fn marker_whitespace_variants_are_all_accepted() {
        for text in [
            "#rubocop:disable Foo/Bar\n",
            "# rubocop:disable Foo/Bar\n",
            "#  rubocop  :  disable   Foo/Bar\n",
            "# rubocop : disable Foo/Bar\n",
        ] {
            let d = directives_for(text);
            assert_eq!(d.directives().len(), 1, "failed for {text:?}");
            assert_eq!(d.directives()[0].kind, DirectiveKind::Disable);
        }
    }

    #[test]
    fn directive_embedded_in_a_string_literal_is_not_a_real_comment() {
        // "does not confuse a comment directive embedded in a string
        // literal with a real comment" -- free, per the module docs: real
        // parsers (here, Prism via `from_parsed`) never yield string
        // contents as comments in the first place.
        let d = directives_for(
            "string = <<~END\nThis is a string not a real comment # rubocop:disable Style/Loop\nEND\n",
        );
        assert!(d.directives().is_empty());
        assert!(!d.is_disabled("Style/Loop", 2));
    }

    #[test]
    fn department_prefix_disables_every_member_cop() {
        let d = directives_for("# rubocop:disable Style\nfoo\n# rubocop:enable Style\n");
        assert!(d.is_disabled("Style/For", 1));
        assert!(d.is_disabled("Style/Not", 2));
        assert!(!d.is_disabled("Lint/Void", 1));
        assert!(!d.is_disabled("Style/For", 4));
    }

    #[test]
    fn todo_directive_behaves_like_an_unpaired_disable() {
        let d = directives_for("# rubocop:todo Foo/Bar\nfoo\n");
        assert_eq!(d.directives()[0].kind, DirectiveKind::Todo);
        assert!(d.is_disabled("Foo/Bar", 500));
    }

    #[test]
    fn directives_accessor_reports_all_directives_in_source_order() {
        let d = directives_for("# rubocop:disable Foo/Bar\nfoo\n# rubocop:enable Foo/Bar\n");
        let all = d.directives();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].line, 1);
        assert_eq!(all[0].kind, DirectiveKind::Disable);
        assert!(!all[0].inline);
        assert_eq!(all[1].line, 3);
        assert_eq!(all[1].kind, DirectiveKind::Enable);
    }

    // Ports of RuboCop::CommentConfig / DirectiveComment spec cases for the
    // `-next` modes (spec/rubocop/comment_config_spec.rb and
    // spec/rubocop/directive_comment_spec.rb, RuboCop 1.91.0). See the
    // module docs for how this crate's one-physical-line approximation
    // relates to upstream's full AST-statement scope.

    #[test]
    fn disable_next_targets_only_the_single_next_physical_line() {
        // comment_config_spec.rb, "disable-next directives" > "when several
        // statements share the target line" -> "scopes to that line only"
        // ([2..2]); this is one of the cases where upstream's AST-statement
        // scope and this crate's one-physical-line approximation agree,
        // since both statements on line 2 still end on line 2.
        let d = directives_for("# rubocop:disable-next Style/Semicolon\na = 1; b = 2\nc = 3\n");
        assert!(!d.is_disabled("Style/Semicolon", 1));
        assert!(d.is_disabled("Style/Semicolon", 2));
        assert!(!d.is_disabled("Style/Semicolon", 3));
    }

    #[test]
    fn todo_next_department_disables_every_member_cop_on_the_next_line() {
        // comment_config_spec.rb, "disable-next directives" > "with a
        // department" -> "disables every cop of the department for the
        // statement".
        let d = directives_for("# rubocop:todo-next Style\n@@foo = 1\n@@bar = 2\n");
        assert!(d.is_disabled("Style/ClassVars", 2));
        assert!(d.is_disabled("Style/FrozenStringLiteralComment", 2));
        assert!(!d.is_disabled("Style/ClassVars", 3));
    }

    #[test]
    fn inline_disable_next_targets_the_following_line_not_its_own() {
        // comment_config_spec.rb, "disable-next directives" > "when the
        // directive sits at the end of a code line" -> upstream does *not*
        // honor this (it requires a comment-only line and records the
        // directive as detached instead). This crate has no notion of a
        // "detached" directive and, per the module docs, resolves every
        // `-next` directive to the physical next line regardless of
        // inline-ness, so it deliberately diverges here and disables line 2.
        let d = directives_for("puts 1 # rubocop:disable-next Metrics/MethodLength\nputs 2\n");
        assert!(!d.is_disabled("Metrics/MethodLength", 1));
        assert!(d.is_disabled("Metrics/MethodLength", 2));
    }

    #[test]
    fn enable_next_suspends_an_open_disable_for_the_following_line_only() {
        // comment_config_spec.rb, "enable-next directives" > "inside a
        // disabled region" -> "enables the cop for the statement only".
        let d = directives_for(
            "# rubocop:disable Style/For\nfor x in [1, 2] do x end\n# rubocop:enable-next Style/For -- reviewed\nfor y in [3, 4] do y end\nfor z in [5, 6] do z end\n# rubocop:enable Style/For\n",
        );
        assert!(d.is_disabled("Style/For", 2));
        assert!(!d.is_disabled("Style/For", 4));
        assert!(d.is_disabled("Style/For", 5));
    }

    #[test]
    fn enable_next_all_suspends_every_open_disable_for_the_following_line() {
        // comment_config_spec.rb, "enable-next directives" > "with `all`" ->
        // "suspends every open disable for the statement".
        let d = directives_for(
            "# rubocop:disable Style/For, Style/Not\nfor x in [1, 2] do not x.nil? end\n# rubocop:enable-next all\nfor y in [3, 4] do not y.nil? end\n# rubocop:enable Style/For, Style/Not\n",
        );
        assert!(d.is_disabled("Style/For", 2));
        assert!(!d.is_disabled("Style/For", 4));
        assert!(!d.is_disabled("Style/Not", 4));
    }

    #[test]
    fn disable_next_mode_is_parsed_with_its_cop_list() {
        // directive_comment_spec.rb, "#disable_next?" > "when disable-next"
        // -> "is a disabling directive with the listed cops".
        let d =
            directives_for("# rubocop:disable-next Metrics/AbcSize, Metrics/MethodLength\nfoo\n");
        assert_eq!(d.directives().len(), 1);
        let directive = &d.directives()[0];
        assert_eq!(directive.kind, DirectiveKind::DisableNext);
        assert!(directive.kind.disables());
        assert_eq!(
            directive.cops,
            vec![
                CopRef::Cop("Metrics/AbcSize".to_string()),
                CopRef::Cop("Metrics/MethodLength".to_string())
            ]
        );
    }

    #[test]
    fn todo_next_mode_disables_and_keeps_reason_out_of_cops() {
        // directive_comment_spec.rb, "#disable_next?" > "when todo-next" ->
        // "is a disabling directive and keeps the reason".
        let d = directives_for("# rubocop:todo-next Metrics/AbcSize -- a reason\nfoo\n");
        let directive = &d.directives()[0];
        assert_eq!(directive.kind, DirectiveKind::TodoNext);
        assert!(directive.kind.disables());
        assert_eq!(directive.cops, vec![CopRef::Cop("Metrics/AbcSize".to_string())]);
    }

    #[test]
    fn enable_next_mode_does_not_disable_and_keeps_reason_out_of_cops() {
        // directive_comment_spec.rb, "#enable_next?" > "when an
        // `enable-next` directive" -> "is an enabling directive and keeps
        // the reason".
        let d = directives_for("# rubocop:enable-next Metrics/AbcSize -- settled\nfoo\n");
        let directive = &d.directives()[0];
        assert_eq!(directive.kind, DirectiveKind::EnableNext);
        assert!(!directive.kind.disables());
        assert_eq!(directive.cops, vec![CopRef::Cop("Metrics/AbcSize".to_string())]);
    }
}
