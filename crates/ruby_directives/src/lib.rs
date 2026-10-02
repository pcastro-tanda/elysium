//! `# rubocop:disable`/`enable`/`todo`/`push`/`pop`/`-next` directive comments.
//!
//! Ports RuboCop's `RuboCop::DirectiveComment` (the per-comment directive
//! parser) and `RuboCop::CommentConfig` (the per-line "is this cop disabled
//! here" index, together with its `PushPop` and `DisableNext` mixins) from
//! `lib/rubocop/directive_comment.rb`, `lib/rubocop/comment_config.rb` and
//! `lib/rubocop/comment_config/` (RuboCop 1.91.0).
//!
//! This crate is parser-independent: [`Directives::parse`] takes plain
//! `(Span, &[u8])` comment pairs, so any front end can feed it. Callers that
//! already hold a [`ruby_ast::Parsed`] tree can use [`Directives::from_parsed`]
//! instead, which is what resolves the statement a `-next`/`next` directive
//! scopes to.
//!
//! ## Deliberate deviations from upstream RuboCop
//!
//! - **No cop registry, so `Lint/Syntax` and `Lint/RedundantCopDisableDirective`
//!   are not special-cased.** Upstream silently drops `Lint/Syntax` from
//!   *any* directive (even one that names it directly) and drops
//!   `Lint/RedundantCopDisableDirective` specifically from `all`/`Lint`
//!   department expansions, because those two cops are wired directly into
//!   RuboCop's cop registry and team-runner. This crate has no registry and
//!   no knowledge of specific cop names, so both cops behave like any other
//!   cop name here. The cops that care about this (implemented later in
//!   `rules`) are expected to special-case it themselves.
//! - **`DirectiveComment#malformed?`/`#missing_cop_name?`/`#invalid_signed_args?`
//!   are not ported.** They exist upstream only to power
//!   `Lint/RedundantCopDisableDirective` diagnostics, which is out of scope for
//!   this crate; [`Directives`] simply ignores any trailing text after a
//!   recognized cop list (including a `-- reason` suffix) and any `push`/`next`
//!   argument that carries no `+`/`-` sign, matching the *parsing* behaviour but
//!   not upstream's "is this malformed" diagnostic.
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
//! - **`comment_only_line?` is derived from the source text, not from a token
//!   stream.** Upstream asks the lexer which lines carry a non-comment token;
//!   [`Directives::comment_only_line`] instead calls a line comment-only when
//!   it is blank or when a comment starts it with nothing but whitespace in
//!   front. The two differ only for a line *inside* a multi-line literal that
//!   happens to be blank or to start with `#`: those lines carry string-content
//!   tokens upstream, but hold no comment here.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::bytes::Regex;
use ruby_ast::ext::is_heredoc;
use ruby_ast::{walk, LocationExt, Node, NodeExt, Parsed, Visitor};
use ruby_source::{SourceFile, Span};

/// The recognized `# rubocop:<mode> ...` modes (upstream's
/// `DirectiveComment::AVAILABLE_MODES`).
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
    /// `# rubocop:disable-next ...`, scoped to the statement below.
    DisableNext,
    /// `# rubocop:todo-next ...` (equivalent to `DisableNext`).
    TodoNext,
    /// `# rubocop:enable-next ...`, scoped to the statement below.
    EnableNext,
    /// `# rubocop:push [+Cop -Cop ...]`: saves the current state, then applies
    /// its signed arguments ([`Directive::signs`]).
    Push,
    /// `# rubocop:pop`: restores the state saved by the matching `push`.
    Pop,
    /// `# rubocop:next +Cop -Cop ...`: like `push`'s signed arguments, but
    /// scoped to the statement below instead of to a `pop`.
    Next,
}

impl DirectiveKind {
    /// True for `Disable`, `Todo`, and `DisableNext`/`TodoNext`, matching
    /// upstream `DirectiveComment#disabled?`.
    #[must_use]
    pub const fn disables(self) -> bool {
        matches!(self, Self::Disable | Self::Todo | Self::DisableNext | Self::TodoNext)
    }

    /// True for `Enable`/`EnableNext`, matching upstream
    /// `DirectiveComment#enabled?`.
    #[must_use]
    pub const fn enables(self) -> bool {
        matches!(self, Self::Enable | Self::EnableNext)
    }

    /// True for every mode whose scope is the statement below the comment:
    /// the three `-next` modes plus the bare `next` mode (the modes
    /// `CommentConfig::DisableNext` handles).
    #[must_use]
    pub const fn is_next(self) -> bool {
        matches!(self, Self::DisableNext | Self::TodoNext | Self::EnableNext | Self::Next)
    }

    /// True for the modes whose arguments are `+`/`-` signed cop names
    /// ([`Directive::signs`]): upstream's `push_args`/`signed_args`.
    #[must_use]
    pub const fn is_signed(self) -> bool {
        matches!(self, Self::Push | Self::Next)
    }

    /// True for the modes that close their own scope and so play no part in
    /// `disable`/`enable` pairing -- upstream's
    /// `CommentConfig#self_closing_directive?`.
    #[must_use]
    pub const fn self_closing(self) -> bool {
        self.is_next() || matches!(self, Self::Push | Self::Pop)
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

/// A `+`/`-` operation in front of one cop name of a `push`/`next`
/// directive (upstream's `DirectiveComment::SIGNED_OPERATIONS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sign {
    /// `+Cop`: re-enable the cop within the directive's scope.
    Plus,
    /// `-Cop`: disable the cop within the directive's scope.
    Minus,
}

/// One parsed `# rubocop:<mode> ...` directive comment.
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
    /// The `+`/`-` sign written in front of each entry of [`Self::cops`], in
    /// the same order and of the same length, for the signed modes
    /// (`push`/`next`); empty for every other mode.
    pub signs: Vec<Sign>,
    /// True when this comment is an end-of-line directive: something other
    /// than whitespace precedes it on its physical source line. Inline
    /// directives affect only their own line; own-line directives affect
    /// every line from themselves to a matching `enable` (or end of file).
    pub inline: bool,
    /// For the statement-scoped modes ([`DirectiveKind::is_next`]), the
    /// inclusive `(first_line, last_line)` of the statement the directive
    /// attached to -- upstream's `CommentConfig#statement_scope_after`.
    /// `None` when the directive attached to nothing (it sits at the end of
    /// a code line, or only blank lines / end of file follow it), which makes
    /// it one of [`Directives::detached_next_directives`]. Always `None` for
    /// every other mode.
    pub scope: Option<(u32, u32)>,
}

impl Directive {
    /// The `(sign, cop)` pairs of a signed (`push`/`next`) directive, in
    /// written order; empty for every other mode.
    pub fn signed_args(&self) -> impl Iterator<Item = (Sign, &CopRef)> {
        self.signs.iter().copied().zip(self.cops.iter())
    }
}

/// The directive comments found in one file, and the disabled/enabled state
/// they imply at any given line.
///
/// Mirrors RuboCop's `CommentConfig#cop_enabled_at_line?` /
/// `#cop_disabled_line_ranges`.
#[derive(Debug, Clone, Default)]
pub struct Directives {
    directives: Vec<Directive>,
    /// Per 1-based source line (index `line - 1`): true when the line holds
    /// no code -- upstream's `CommentConfig#comment_only_line?`, which is
    /// also true for a blank line (it has no tokens at all).
    comment_only: Vec<bool>,
}

// Ported from RuboCop::DirectiveComment (directive_comment.rb, 1.91.0):
//   DIRECTIVE_MARKER_PATTERN = '# rubocop : '  (every space -> \s*)
//   AVAILABLE_MODES = disable enable todo push pop disable-next todo-next
//                     enable-next next
//   DIRECTIVE_HEADER_PATTERN = marker + "((?:MODES_PATTERN))\b"
//   COP_NAME_PATTERN = ([A-Za-z]\w+/)*(?:[A-Za-z]\w+)
//   COPS_PATTERN = (all|(?:COP_NAME_PATTERN , )*COP_NAME_PATTERN)
//   PUSH_POP_ARGS_PATTERN = ([+-]COP_NAME_NC(?:\s+[+-]COP_NAME_NC)*)
//   DIRECTIVE_COMMENT_REGEXP = header + "(?:\s+COPS_PATTERN|\s+PUSH_POP_ARGS_PATTERN)?"
// Modes are listed longest-first in the alternation (matching upstream's
// `MODES_PATTERN = AVAILABLE_MODES.sort_by { |mode| -mode.length }`: "Longest
// first, so a `-next` mode is not matched as its prefix"): the `regex` crate
// picks the first alternative that matches at a position, not the longest, so
// `disable-next Foo` would otherwise match mode `disable` and leave a
// dangling `-next Foo` that fails the optional cops group.
// `(?-u)` restricts \w/\s/\b to ASCII, matching Ruby's default \w for cop
// identifiers (which are always ASCII) and avoiding any UTF-8-boundary
// subtleties on raw comment bytes.
static DIRECTIVE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?-u)\#\s*rubocop\s*:\s*(?P<mode>disable-next|enable-next|todo-next|disable|enable|push|todo|next|pop)\b(?:\s+(?P<cops>all|(?:[A-Za-z]\w+/)*[A-Za-z]\w+(?:\s*,\s*(?:[A-Za-z]\w+/)*[A-Za-z]\w+)*)|\s+(?P<signed>[+\-](?:[A-Za-z]\w+/)*[A-Za-z]\w+(?:\s+[+\-](?:[A-Za-z]\w+/)*[A-Za-z]\w+)*))?",
    )
    .expect("static directive regex is valid")
});

/// `\A#\s*\z`: upstream drops a match whose `pre_match` is nothing but a
/// second comment marker, so a commented-out directive (`# # rubocop:disable
/// Foo`) is an ordinary comment rather than a directive.
fn is_commented_out(prefix: &[u8]) -> bool {
    matches!(prefix.split_first(), Some((b'#', rest)) if rest.iter().all(u8::is_ascii_whitespace))
}

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

/// `DirectiveComment#parse_signed_args`: whitespace-separated `+`/`-` prefixed
/// cop names. `all` is not special here (upstream's push/next arguments go
/// through `expand_cop_name`, never through the `all` short-circuit), so a
/// literal `-all` names a department that no cop belongs to.
fn parse_signed_args(raw: &[u8]) -> (Vec<Sign>, Vec<CopRef>) {
    let raw = std::str::from_utf8(raw).unwrap_or("");
    let mut signs = Vec::new();
    let mut cops = Vec::new();
    for token in raw.split_ascii_whitespace() {
        let sign = match token.as_bytes().first() {
            Some(b'+') => Sign::Plus,
            Some(b'-') => Sign::Minus,
            _ => continue,
        };
        signs.push(sign);
        cops.push(classify_cop(&token[1..]));
    }
    (signs, cops)
}

fn is_inline(source: &SourceFile, comment_start: u32) -> bool {
    let line = source.line_col(comment_start).line;
    let line_start = source.lines().line_start(line);
    let prefix = &source.bytes()[line_start as usize..comment_start as usize];
    prefix.iter().any(|b| !b.is_ascii_whitespace())
}

fn parse_comment(source: &SourceFile, span: Span, text: &[u8]) -> Option<Directive> {
    let captures = DIRECTIVE_REGEX.captures(text)?;
    let whole = captures.get(0).expect("regex match always has a full match");
    if is_commented_out(&text[..whole.start()]) {
        return None;
    }
    let kind = match captures.name("mode")?.as_bytes() {
        b"disable" => DirectiveKind::Disable,
        b"enable" => DirectiveKind::Enable,
        b"todo" => DirectiveKind::Todo,
        b"disable-next" => DirectiveKind::DisableNext,
        b"todo-next" => DirectiveKind::TodoNext,
        b"enable-next" => DirectiveKind::EnableNext,
        b"push" => DirectiveKind::Push,
        b"pop" => DirectiveKind::Pop,
        b"next" => DirectiveKind::Next,
        _ => return None,
    };
    // Upstream keeps whichever of the two argument groups matched; the cop
    // list and the signed list are only meaningful for their own modes.
    let (signs, cops) = if kind.is_signed() {
        captures.name("signed").map(|m| parse_signed_args(m.as_bytes())).unwrap_or_default()
    } else {
        (Vec::new(), captures.name("cops").map(|m| parse_cops(m.as_bytes())).unwrap_or_default())
    };
    let directive_span = Span::new(
        span.start + u32::try_from(whole.start()).unwrap_or(u32::MAX),
        span.start + u32::try_from(whole.end()).unwrap_or(u32::MAX),
    );
    let line = source.line_col(span.start).line;
    let inline = is_inline(source, span.start);
    Some(Directive { span: directive_span, line, kind, cops, signs, inline, scope: None })
}

impl Directives {
    /// Parses directive comments out of `comments`.
    ///
    /// Each item is one comment's byte span (used to compute its 1-based
    /// line and inline/own-line status) paired with its raw text (including
    /// the leading `#`). Without a syntax tree the statement a `-next`/`next`
    /// directive scopes to is taken to be the single code line it attaches
    /// to; [`Directives::from_parsed`] resolves the real statement extent.
    pub fn parse<'a>(
        source: &SourceFile,
        comments: impl Iterator<Item = (Span, &'a [u8])>,
    ) -> Self {
        Self::build(source, comments, &BTreeMap::new())
    }

    /// Convenience wrapper over [`Directives::parse`] for a Prism [`Parsed`] tree.
    #[must_use]
    pub fn from_parsed(parsed: &Parsed<'_>) -> Self {
        let source = parsed.source();
        let comments: Vec<_> = parsed.comments().collect();
        let statement_ends = statement_end_lines(&parsed.root(), source);
        Self::build(
            source,
            comments.iter().map(|c| (c.location().span(), c.text())),
            &statement_ends,
        )
    }

    fn build<'a>(
        source: &SourceFile,
        comments: impl Iterator<Item = (Span, &'a [u8])>,
        statement_ends: &BTreeMap<u32, u32>,
    ) -> Self {
        let comments: Vec<(Span, &[u8])> = comments.collect();
        let kinds = line_kinds(source, &comments);
        let mut directives: Vec<Directive> =
            comments.iter().filter_map(|&(span, text)| parse_comment(source, span, text)).collect();
        for directive in &mut directives {
            if directive.kind.is_next() {
                directive.scope = statement_scope_after(directive.line, &kinds, statement_ends);
            }
        }
        let comment_only = kinds.iter().map(|&kind| kind != LineKind::Code).collect();
        Self { directives, comment_only }
    }

    /// Every directive found, in source order.
    #[must_use]
    pub fn directives(&self) -> &[Directive] {
        &self.directives
    }

    /// Upstream's `CommentConfig#comment_only_line?`: true when `line` (1-based)
    /// carries no code token. A blank line -- and any line past the end of the
    /// file -- qualifies, exactly as it does upstream (it has no tokens at all).
    #[must_use]
    pub fn comment_only_line(&self, line: u32) -> bool {
        let index = line.checked_sub(1).map(|i| i as usize);
        index.is_none_or(|i| self.comment_only.get(i).copied().unwrap_or(true))
    }

    /// Upstream's `CommentConfig#detached_next_directives`: the statement-scoped
    /// directives that affect nothing, because they sit at the end of a code
    /// line or because no statement follows them.
    pub fn detached_next_directives(&self) -> impl Iterator<Item = &Directive> {
        self.directives.iter().filter(|d| d.kind.is_next() && d.scope.is_none())
    }

    /// Builds the inclusive `(start_line, end_line)` ranges (RuboCop's
    /// `CopAnalysis#line_ranges`) that leave `target` disabled, by replaying
    /// every directive in source order, starting from `seed` (`None`: the cop
    /// begins the file enabled; `Some(0)`: the cop begins the file disabled,
    /// modeling `CommentConfig#inject_disabled_cops_directives`'s synthetic
    /// `-Infinity`-anchored disable for a cop the configuration disables --
    /// see [`Self::is_disabled_for_opted_in_cop`]). Line `0` never occurs for
    /// a real directive (lines are 1-based), so it sorts before every real
    /// line exactly like `-Float::INFINITY` does upstream.
    ///
    /// Mirrors `CommentConfig#analyze`'s per-cop state machine
    /// (`analyze_single_line`/`analyze_disabled`/`analyze_rest`, plus
    /// `CommentConfig::PushPop` and `CommentConfig::DisableNext`), specialized
    /// to one cop instead of a registry-expanded map of them: the `push`/`pop`
    /// stack saves and restores only this cop's open range, since
    /// `popped_analysis` keeps the already-closed ranges of the popped state
    /// and restores nothing but `start_line_number`.
    fn disabled_ranges_from(&self, target: Target<'_>, seed: Option<u32>) -> Vec<(u32, u32)> {
        let mut ranges = Vec::new();
        let mut start = seed;
        let mut stack: Vec<Option<u32>> = Vec::new();
        for directive in &self.directives {
            match directive.kind {
                DirectiveKind::Push => {
                    stack.push(start);
                    for (sign, cop) in directive.signed_args() {
                        if target.matches(cop) {
                            apply_sign(&mut ranges, &mut start, sign, directive.line);
                        }
                    }
                }
                DirectiveKind::Pop => {
                    if let Some(restored) = stack.pop() {
                        // popped_analysis: the open range closes just above
                        // the `pop`, and a disable that was open at the
                        // matching `push` resumes *from the `pop` line*.
                        close(&mut ranges, &mut start, directive.line.saturating_sub(1));
                        start = restored.map(|_| directive.line);
                    }
                }
                DirectiveKind::DisableNext
                | DirectiveKind::TodoNext
                | DirectiveKind::EnableNext => {
                    let Some(bounds) = directive.scope else { continue };
                    if !directive.cops.iter().any(|cop| target.matches(cop)) {
                        continue;
                    }
                    if directive.kind.disables() {
                        // add_next_range: an independent one-off range that
                        // doesn't disturb any already-open `start`.
                        ranges.push(bounds);
                    } else {
                        suspend(&mut ranges, &mut start, bounds);
                    }
                }
                DirectiveKind::Next => {
                    let Some(bounds) = directive.scope else { continue };
                    for (sign, cop) in directive.signed_args() {
                        if !target.matches(cop) {
                            continue;
                        }
                        match sign {
                            Sign::Minus => ranges.push(bounds),
                            Sign::Plus => suspend(&mut ranges, &mut start, bounds),
                        }
                    }
                }
                DirectiveKind::Disable | DirectiveKind::Todo | DirectiveKind::Enable => {
                    if !directive.cops.iter().any(|cop| target.matches(cop)) {
                        continue;
                    }
                    if directive.inline {
                        // analyze_single_line: an inline `enable` has no effect;
                        // an inline disable/todo affects only its own line.
                        if directive.kind.disables() {
                            ranges.push((directive.line, directive.line));
                        }
                    } else {
                        // analyze_disabled closes any already-open range at this
                        // line (the "double disable" case) and reopens here;
                        // analyze_rest just closes.
                        close(&mut ranges, &mut start, directive.line);
                        if directive.kind.disables() {
                            start = Some(directive.line);
                        }
                    }
                }
            }
        }
        if let Some(s) = start {
            ranges.push((s, u32::MAX));
        }
        ranges
    }

    /// [`Self::disabled_ranges_from`] with no seed: the cop starts the file enabled.
    fn disabled_ranges(&self, target: Target<'_>) -> Vec<(u32, u32)> {
        self.disabled_ranges_from(target, None)
    }

    /// True when `cop_name` is disabled at `line`, matching RuboCop's
    /// `CommentConfig#cop_enabled_at_line?` (negated).
    #[must_use]
    pub fn is_disabled(&self, cop_name: &str, line: u32) -> bool {
        self.is_disabled_in(cop_name, line, line)
    }

    /// True when `cop_name` is disabled on any line of `first_line..=last_line`,
    /// matching RuboCop's `CommentConfig#cop_enabled_at_lines?` (negated): a
    /// directive on any line of a multi-line offense suppresses it
    /// (`Cop::Base#enabled_lines?`).
    #[must_use]
    pub fn is_disabled_in(&self, cop_name: &str, first_line: u32, last_line: u32) -> bool {
        overlaps(
            &self.disabled_ranges(Target::Cop { name: cop_name, include_all: true }),
            first_line,
            last_line,
        )
    }

    /// Like [`Self::is_disabled`], but ignores `# rubocop:disable all` coverage: only an
    /// explicit mention of `cop_name` (or its department) counts. `Lint/RedundantCopDisableDirective`
    /// is excluded from `all`/`Lint` department expansion by RuboCop's own registry
    /// (`DirectiveComment#exclude_lint_department_cops`), so `all` never actually silences it,
    /// even though this crate's registry-less [`CopRef::All`] otherwise `covers` every name.
    #[must_use]
    pub fn is_disabled_by_name(&self, cop_name: &str, first_line: u32, last_line: u32) -> bool {
        overlaps(
            &self.disabled_ranges(Target::Cop { name: cop_name, include_all: false }),
            first_line,
            last_line,
        )
    }

    /// True when a `# rubocop:disable all` (or `todo all`) directive covers
    /// any line of `first_line..=last_line`.
    #[must_use]
    pub fn all_disabled_in(&self, first_line: u32, last_line: u32) -> bool {
        overlaps(&self.disabled_ranges(Target::AllOnly), first_line, last_line)
    }

    /// True when `cop_name` has been "opted in" for this file by an explicit
    /// `# rubocop:enable <cop_name>` directive naming it exactly, anywhere in the
    /// file (inline or own-line) -- RuboCop's `CommentConfig#cop_opted_in?`
    /// (`comment_config.rb:48-50`), backed by `#opt_in_cops` (`comment_config.rb:84-95`):
    /// `opt_in_cops` merges each `enable` directive's *unresolved* `raw_cop_names`, so a
    /// `# rubocop:enable all` (`next if directive.all_cops?`, line 89) or a department-only
    /// mention (`# rubocop:enable Layout` does not expand to `Layout/LineLength`, unlike the
    /// registry-expanded `cop_names` used for line-range analysis) does not opt a specific cop
    /// in -- only naming it exactly does.
    ///
    /// `Cop::Team#roundup_relevant_cops` (`team.rb:178-186`) checks this *before*
    /// `cop.excluded_file?` or `@registry.enabled?(cop, @config)`
    /// (`next true if processed_source.comment_config.cop_opted_in?(cop)` at line 180 precedes
    /// both), so an opted-in cop reactivates for the whole file regardless of *why* it was
    /// disabled -- `AllCops: DisabledByDefault: true`, an explicit `Enabled: false`, or a
    /// disabled department -- and regardless of `Include`/`Exclude`.
    #[must_use]
    pub fn is_opted_in(&self, cop_name: &str) -> bool {
        self.directives.iter().any(|d| {
            let named =
                |c: &CopRef| matches!(c, CopRef::Cop(n) | CopRef::Department(n) if n == cop_name);
            if d.kind.enables() {
                d.cops.iter().any(named)
            } else if d.kind.is_signed() {
                // A `+` argument of a `push`/`next` opts the cop in too.
                d.signed_args().any(|(sign, cop)| sign == Sign::Plus && named(cop))
            } else {
                false
            }
        })
    }

    /// True when `cop_name` -- a cop [`Self::is_opted_in`] has reactivated after it was
    /// disabled by configuration -- is *still* disabled on any line of
    /// `first_line..=last_line`.
    ///
    /// Mirrors `CommentConfig#inject_disabled_cops_directives` (`comment_config.rb:168-175`):
    /// before folding in the file's real directives, `#analyze` seeds every config-disabled
    /// cop with a synthetic own-line `# rubocop:disable <cop>` directive at
    /// `CONFIG_DISABLED_LINE_RANGE_MIN` (`-Float::INFINITY`, `comment_config.rb:9`). Replaying
    /// the real directives from that seed closes the disabled range at the first own-line
    /// `enable` naming the cop (directly or via its department/`all`), so offenses starting
    /// strictly after that `enable` line are reported and everything up to and including it
    /// -- the entire file, absent such a directive -- is not. Line `0` (never a real 1-based
    /// source line) stands in for `-Infinity`: it sorts before every real line the same way.
    #[must_use]
    pub fn is_disabled_for_opted_in_cop(
        &self,
        cop_name: &str,
        first_line: u32,
        last_line: u32,
    ) -> bool {
        overlaps(
            &self.disabled_ranges_from(Target::Cop { name: cop_name, include_all: true }, Some(0)),
            first_line,
            last_line,
        )
    }

    /// Port of `CommentConfig#extra_enabled_comments` (`comment_config.rb:55-62`,
    /// `#extra_enabled_comments_with_names`/`#handle_enable_all`/`#handle_switch`,
    /// lines 70-82 and 334-357): for every own-line `enable` directive, the cop
    /// references it names that were not actually disabled at that point, so
    /// enabling them is redundant. Backs `Lint/RedundantCopEnableDirective`.
    ///
    /// `cop_names` is the cop universe (`RuleOptions::peer_names()`), used to
    /// expand `all` and department references the way upstream's cop registry
    /// does: a `# rubocop:disable Layout` counts as a disable of every `Layout`
    /// cop, so a later `# rubocop:enable Layout/LineLength` is not redundant.
    /// `disabled_by_config` is every cop the configuration itself disables
    /// (upstream's `registry.disabled(config)`), each of which
    /// `#extra_enabled_comments` seeds with one pending disable, so the first
    /// `enable` of such a cop is legitimate and only a second one is redundant.
    ///
    /// Each returned pair is one directive's [`Directive::span`] and the cop
    /// references it redundantly enables, spelled as written (`all`, a
    /// department name, or a cop name) and deduplicated, in written order --
    /// the shape `Lint/RedundantCopEnableDirective#register_offense` needs to
    /// locate each name inside the comment. Upstream reaches the same shape by
    /// mapping an expanded name back to its department (`name.split('/').first`)
    /// and letting duplicate offenses collapse.
    ///
    /// Deviations, on top of the crate-wide ones in the module docs: upstream
    /// tests `comment_only_line?` against the token stream, this uses
    /// [`Directive::inline`]. The self-closing modes (`push`, `pop`, and the
    /// statement-scoped ones) take no part in the disable/enable pairing, per
    /// upstream's `self_closing_directive?` guard.
    #[must_use]
    pub fn redundant_enables<'a>(
        &self,
        cop_names: impl Iterator<Item = &'a str>,
        disabled_by_config: impl Iterator<Item = &'a str>,
    ) -> Vec<(Span, Vec<String>)> {
        let universe: Vec<&str> = cop_names.collect();
        // `disable_count` upstream: how many pending disables each cop has.
        let mut pending: BTreeMap<String, u32> = BTreeMap::new();
        for name in disabled_by_config {
            *pending.entry(name.to_string()).or_default() += 1;
        }

        let mut extras: Vec<(Span, Vec<String>)> = Vec::new();
        let relevant = self
            .directives
            .iter()
            .filter(|d| !d.inline && !d.kind.self_closing() && !d.cops.is_empty());
        for directive in relevant {
            let disables = directive.kind.disables();
            if !disables && directive.cops == [CopRef::All] {
                // handle_enable_all: `enable all` is redundant only when it
                // enables nothing; otherwise it clears one pending disable
                // from every cop that has one.
                let mut enabled = 0_u32;
                for count in pending.values_mut() {
                    if *count > 0 {
                        *count -= 1;
                        enabled += 1;
                    }
                }
                if enabled == 0 {
                    record_extra(&mut extras, directive.span, "all");
                }
                continue;
            }
            for cop in &directive.cops {
                let written = match cop {
                    CopRef::All => "all",
                    CopRef::Department(name) | CopRef::Cop(name) => name.as_str(),
                };
                for name in expand(&universe, cop) {
                    if disables {
                        *pending.entry(name).or_default() += 1;
                    } else if let Some(count) = pending.get_mut(&name).filter(|c| **c > 0) {
                        *count -= 1;
                    } else {
                        record_extra(&mut extras, directive.span, written);
                    }
                }
            }
        }
        extras
    }
}

/// Which cop one replay of the directive stream is about.
#[derive(Debug, Clone, Copy)]
enum Target<'a> {
    /// One concrete cop name; `include_all` decides whether a `disable all`
    /// counts as covering it.
    Cop { name: &'a str, include_all: bool },
    /// `all` references only, whatever cops they would expand to.
    AllOnly,
}

impl Target<'_> {
    fn matches(self, cop: &CopRef) -> bool {
        match self {
            Self::Cop { name, include_all } => {
                (include_all || !matches!(cop, CopRef::All)) && cop.covers(name)
            }
            Self::AllOnly => matches!(cop, CopRef::All),
        }
    }
}

/// `CopAnalysis#close`: the open range, if any, ends at `line` -- unless that
/// would be above its own start, in which case it yields no range at all.
fn close(ranges: &mut Vec<(u32, u32)>, start: &mut Option<u32>, line: u32) {
    if let Some(s) = start.take() {
        if line >= s {
            ranges.push((s, line));
        }
    }
}

/// `PushPop#apply_cop_op`: a `-` opens a range unless one is already open, a
/// `+` closes the open one, and either is a no-op otherwise.
fn apply_sign(ranges: &mut Vec<(u32, u32)>, start: &mut Option<u32>, sign: Sign, line: u32) {
    match sign {
        Sign::Minus if start.is_none() => *start = Some(line),
        Sign::Plus if start.is_some() => close(ranges, start, line),
        _ => {}
    }
}

/// `DisableNext#suspend_disable`: punch a statement-sized hole into the open
/// disable -- it closes just above the statement and reopens right below it.
/// A no-op when nothing is open.
fn suspend(ranges: &mut Vec<(u32, u32)>, start: &mut Option<u32>, bounds: (u32, u32)) {
    if start.is_none() {
        return;
    }
    close(ranges, start, bounds.0.saturating_sub(1));
    *start = Some(bounds.1 + 1);
}

/// What one physical source line holds, as far as directive analysis cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineKind {
    /// The line has at least one code token.
    Code,
    /// The line is nothing but a comment.
    Comment,
    /// The line is empty or all whitespace.
    Blank,
}

/// Classifies every 1-based source line, standing in for upstream's
/// `CommentConfig#non_comment_token_line_numbers` (which it derives from the
/// token stream): a line is [`LineKind::Comment`] when a comment starts on it
/// with nothing but whitespace in front, and [`LineKind::Blank`] when it holds
/// no non-whitespace byte at all. Both count as `comment_only_line?`.
fn line_kinds(source: &SourceFile, comments: &[(Span, &[u8])]) -> Vec<LineKind> {
    let count = source.line_count();
    let mut kinds: Vec<LineKind> = (1..=count)
        .map(|line| {
            if source.line_text(line).iter().all(u8::is_ascii_whitespace) {
                LineKind::Blank
            } else {
                LineKind::Code
            }
        })
        .collect();
    for &(span, _) in comments {
        if is_inline(source, span.start) {
            continue;
        }
        let line = source.line_col(span.start).line;
        if let Some(kind) = line.checked_sub(1).and_then(|i| kinds.get_mut(i as usize)) {
            *kind = LineKind::Comment;
        }
    }
    kinds
}

fn kind_at(kinds: &[LineKind], line: u32) -> Option<LineKind> {
    line.checked_sub(1).and_then(|i| kinds.get(i as usize)).copied()
}

/// `DisableNext#statement_scope_after`: the inclusive line range of the
/// statement a next-statement directive on `line` attaches to, or `None` when
/// it attaches to nothing. The directive is honored only on a comment-only
/// line; comment-only lines chain (so several directives can stack) while a
/// blank line breaks the attachment.
fn statement_scope_after(
    line: u32,
    kinds: &[LineKind],
    statement_ends: &BTreeMap<u32, u32>,
) -> Option<(u32, u32)> {
    if kind_at(kinds, line) == Some(LineKind::Code) {
        return None;
    }
    let code_line = attached_code_line(line, kinds)?;
    // `statement_bounds_at`: a code line where no statement starts (e.g. a
    // lone `end`) scopes the directive to that line alone.
    let end = statement_ends.get(&code_line).copied().unwrap_or(code_line);
    Some((code_line, end.max(code_line)))
}

/// `DisableNext#attached_code_line`.
fn attached_code_line(directive_line: u32, kinds: &[LineKind]) -> Option<u32> {
    let mut line = directive_line + 1;
    while let Some(kind) = kind_at(kinds, line) {
        match kind {
            LineKind::Code => return Some(line),
            LineKind::Blank => return None,
            LineKind::Comment => line += 1,
        }
    }
    None
}

/// The last line of the statement starting on each code line -- upstream's
/// `DisableNext#statement_bounds_at`/`#statement_end_line`, precomputed in one
/// post-order walk instead of re-scanning the tree per directive.
///
/// Upstream picks the node starting on the line whose own `last_line` is
/// greatest and then takes the maximum end line over *its* descendants
/// (counting a heredoc's closing line, since a heredoc node's own location
/// stops at the opener); taking the maximum over every node that starts on the
/// line reaches the same result, because the node that reaches furthest is the
/// one upstream selects. `StatementsNode` is skipped for the same reason
/// upstream skips `begin` nodes: a sequence of statements is not a statement,
/// and scoping a directive to one would cover everything up to the last
/// statement of the sequence.
fn statement_end_lines(root: &Node<'_>, source: &SourceFile) -> BTreeMap<u32, u32> {
    struct Walker<'a> {
        source: &'a SourceFile,
        /// One entry per open ancestor: the greatest end line seen below it.
        stack: Vec<u32>,
        ends: BTreeMap<u32, u32>,
    }

    impl<'pr> Visitor<'pr> for Walker<'_> {
        fn enter(&mut self, _node: &Node<'pr>) {
            self.stack.push(0);
        }

        fn leave(&mut self, node: &Node<'pr>) {
            let below = self.stack.pop().unwrap_or(0);
            let span = node.span();
            let end = heredoc_end_line(node, self.source)
                .unwrap_or_else(|| self.source.line_col(span.end.saturating_sub(1)).line);
            let reach = below.max(end);
            if let Some(parent) = self.stack.last_mut() {
                *parent = (*parent).max(reach);
            }
            if !matches!(node, Node::StatementsNode { .. } | Node::ProgramNode { .. }) {
                let first = self.source.line_col(span.start).line;
                let entry = self.ends.entry(first).or_default();
                *entry = (*entry).max(reach);
            }
        }
    }

    let mut walker = Walker { source, stack: Vec::new(), ends: BTreeMap::new() };
    walk(root, &mut walker);
    walker.ends
}

/// The line of a heredoc's terminator, which its node's own location (stopping
/// at the opener) does not reach -- `Node#heredoc?`'s `loc.heredoc_end.line`.
fn heredoc_end_line(node: &Node<'_>, source: &SourceFile) -> Option<u32> {
    if !is_heredoc(node) {
        return None;
    }
    let closing = match node {
        Node::StringNode { .. } => node.as_string_node().and_then(|n| n.closing_loc()),
        Node::InterpolatedStringNode { .. } => {
            node.as_interpolated_string_node().and_then(|n| n.closing_loc())
        }
        Node::XStringNode { .. } => node.as_x_string_node().map(|n| n.closing_loc()),
        Node::InterpolatedXStringNode { .. } => {
            node.as_interpolated_x_string_node().map(|n| n.closing_loc())
        }
        _ => None,
    }?;
    Some(source.line_col(closing.span().start).line)
}

/// Every cop name `cop` stands for, against the known cop universe --
/// upstream's `DirectiveComment#cop_names` (registry expansion of `all` and
/// of department names). A reference the universe does not know (an unknown
/// or misspelled cop, or a universe that was never supplied) stands for
/// itself, as `Registry.qualified_cop_name` leaves it.
fn expand(universe: &[&str], cop: &CopRef) -> Vec<String> {
    if matches!(cop, CopRef::All) {
        return universe.iter().map(|name| (*name).to_string()).collect();
    }
    let matched: Vec<String> =
        universe.iter().filter(|name| cop.covers(name)).map(|name| (*name).to_string()).collect();
    if matched.is_empty() {
        match cop {
            CopRef::Department(name) | CopRef::Cop(name) => vec![name.clone()],
            CopRef::All => Vec::new(),
        }
    } else {
        matched
    }
}

/// Adds `name` to the entry for `span`, keeping entries in first-seen order
/// and each entry's names unique (upstream dedups by offense range instead).
fn record_extra(extras: &mut Vec<(Span, Vec<String>)>, span: Span, name: &str) {
    let index = extras.iter().position(|(s, _)| *s == span).unwrap_or_else(|| {
        extras.push((span, Vec::new()));
        extras.len() - 1
    });
    let names = &mut extras[index].1;
    if !names.iter().any(|existing| existing == name) {
        names.push(name.to_string());
    }
}

/// `CommentConfig#cop_enabled_at_lines?`'s overlap test between disabled
/// line ranges and an offense's `first_line..=last_line`.
fn overlaps(ranges: &[(u32, u32)], first_line: u32, last_line: u32) -> bool {
    ranges.iter().any(|&(s, e)| e >= first_line && s <= last_line)
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
        assert!(d.all_disabled_in(2, 2));
        assert!(!d.all_disabled_in(4, 4));
    }

    #[test]
    fn cop_name_containing_substring_all_is_not_confused_with_all_keyword() {
        // "does not confuse a cop name including 'all' with all cops"
        let d = directives_for("foo # rubocop:disable Style/MethodCallWithoutArgsParentheses\n");
        assert!(d.is_disabled("Style/MethodCallWithoutArgsParentheses", 1));
        assert!(!d.is_disabled("Alias", 1));
        assert!(!d.all_disabled_in(1, 1));
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
    fn disable_next_at_the_end_of_a_code_line_is_detached() {
        // comment_config_spec.rb, "disable-next directives" > "when the
        // directive sits at the end of a code line" -> it is not honored (a
        // next-statement directive is only honored on a comment-only line)
        // and is recorded as detached instead.
        let d = directives_for("puts 1 # rubocop:disable-next Metrics/MethodLength\nputs 2\n");
        assert!(!d.is_disabled("Metrics/MethodLength", 1));
        assert!(!d.is_disabled("Metrics/MethodLength", 2));
        assert_eq!(d.detached_next_directives().count(), 1);
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

    // Ports of RuboCop::Cop::Team / RuboCop::CommentConfig opt-in behavior
    // (team.rb, comment_config.rb, RuboCop 1.82.1): a cop the configuration
    // disabled (`AllCops: DisabledByDefault: true` or an explicit
    // `Enabled: false`) is reactivated for the whole file by a
    // `# rubocop:enable <cop>` directive naming it exactly.

    #[test]
    fn enable_directive_opts_a_cop_in_by_exact_name() {
        let d = directives_for("x = 1\n# rubocop:enable Layout/LineLength\ny = 2\n");
        assert!(d.is_opted_in("Layout/LineLength"));
    }

    #[test]
    fn no_enable_directive_means_not_opted_in() {
        let d = directives_for("x = 1\ny = 2\n");
        assert!(!d.is_opted_in("Layout/LineLength"));
    }

    #[test]
    fn enable_all_does_not_opt_a_specific_cop_in() {
        // comment_config.rb:89, `next if directive.all_cops?`.
        let d = directives_for("# rubocop:enable all\n");
        assert!(!d.is_opted_in("Layout/LineLength"));
    }

    #[test]
    fn enable_department_does_not_opt_a_child_cop_in() {
        // `opt_in_cops` merges unresolved `raw_cop_names`, not the
        // registry-expanded department members (comment_config.rb:91).
        let d = directives_for("# rubocop:enable Layout\n");
        assert!(!d.is_opted_in("Layout/LineLength"));
    }

    #[test]
    fn opted_in_cop_is_disabled_up_to_and_including_the_enable_line() {
        let d = directives_for(
            "line1 = 1\nline2 = 2\n# rubocop:enable Layout/LineLength\nline4 = 4\nline5 = 5\n",
        );
        assert!(d.is_opted_in("Layout/LineLength"));
        for line in 1..=3 {
            assert!(
                d.is_disabled_for_opted_in_cop("Layout/LineLength", line, line),
                "line {line} should still be disabled"
            );
        }
        for line in 4..=5 {
            assert!(
                !d.is_disabled_for_opted_in_cop("Layout/LineLength", line, line),
                "line {line} should be reactivated"
            );
        }
    }

    #[test]
    fn cop_never_named_in_a_directive_is_always_disabled_for_the_opted_in_check() {
        // Nothing opts `Layout/LineLength` in, so even though this helper
        // is only ever consulted after `is_opted_in` gates the caller, it
        // independently mirrors the config-disabled synthetic range never
        // being closed: every line stays covered.
        let d = directives_for("x = 1\ny = 2\n");
        assert!(d.is_disabled_for_opted_in_cop("Layout/LineLength", 1, 1));
        assert!(d.is_disabled_for_opted_in_cop("Layout/LineLength", 2, 2));
    }

    #[test]
    fn department_enable_still_reactivates_an_opted_in_cop_once_named() {
        // `is_opted_in` requires the exact name, but once a cop is opted in
        // by *some* directive, the range analysis itself resolves
        // departments/`all` generically (mirroring the registry-expanded
        // `cop_names` upstream's `#analyze` uses), so a later department
        // disable still suppresses it and a department enable still
        // reopens it.
        let d = directives_for(
            "line1 = 1\n# rubocop:enable Layout/LineLength\nline3 = 3\n# rubocop:disable Layout\nline5 = 5\n# rubocop:enable Layout\nline7 = 7\n",
        );
        assert!(d.is_opted_in("Layout/LineLength"));
        assert!(d.is_disabled_for_opted_in_cop("Layout/LineLength", 1, 1));
        assert!(!d.is_disabled_for_opted_in_cop("Layout/LineLength", 3, 3));
        assert!(d.is_disabled_for_opted_in_cop("Layout/LineLength", 5, 5));
        assert!(!d.is_disabled_for_opted_in_cop("Layout/LineLength", 7, 7));
    }

    // Ports of spec/rubocop/cop/lint/redundant_cop_enable_directive_spec.rb
    // (RuboCop 1.82.1), exercising `#extra_enabled_comments` through
    // `redundant_enables`.

    const UNIVERSE: [&str; 5] = [
        "Layout/LineLength",
        "Layout/IndentationStyle",
        "Lint/Debugger",
        "Metrics/AbcSize",
        "Metrics/MethodLength",
    ];

    fn extras(source: &str) -> Vec<Vec<String>> {
        directives_for(source)
            .redundant_enables(UNIVERSE.into_iter(), std::iter::empty())
            .into_iter()
            .map(|(_, names)| names)
            .collect()
    }

    #[test]
    fn enable_without_a_disable_is_redundant() {
        // "registers offense and corrects unnecessary enable"
        assert_eq!(extras("foo\n# rubocop:enable Layout/LineLength\n"), [["Layout/LineLength"]]);
    }

    #[test]
    fn enable_after_a_matching_disable_is_not_redundant() {
        // "registers correct offense when combined with necessary enable"
        assert_eq!(
            extras(
                "# rubocop:disable Layout/LineLength\nfoo\n# rubocop:enable Metrics/AbcSize, Layout/LineLength\n"
            ),
            [["Metrics/AbcSize"]]
        );
    }

    #[test]
    fn second_enable_of_the_same_cop_is_redundant() {
        // "registers offense and corrects redundant enabling of same cop"
        assert_eq!(
            extras(
                "# rubocop:disable Layout/LineLength\nfoo\n# rubocop:enable Layout/LineLength\nbar\n# rubocop:enable Layout/LineLength\n"
            ),
            [["Layout/LineLength"]]
        );
    }

    #[test]
    fn enable_of_a_cop_disabled_by_its_department_is_not_redundant() {
        // "registers offense and corrects redundant enabling of cop of same
        // department": the department disable expands, so enabling one of its
        // cops is legitimate -- and enabling it twice is not.
        assert!(extras("# rubocop:disable Layout\nfoo\n# rubocop:enable Layout/LineLength\n")
            .is_empty());
        assert_eq!(
            extras("# rubocop:disable Layout\nfoo\n# rubocop:enable Layout, Layout/LineLength\n"),
            [["Layout/LineLength"]]
        );
    }

    #[test]
    fn enable_all_is_redundant_only_when_nothing_was_disabled() {
        // "all switch": bare `enable all` is redundant; `enable all` after any
        // disable is not.
        assert_eq!(extras("foo\n# rubocop:enable all\n"), [["all"]]);
        assert!(
            extras("# rubocop:disable Layout/LineLength\nfoo\n# rubocop:enable all\n").is_empty()
        );
    }

    #[test]
    fn config_disabled_cop_absorbs_the_first_enable() {
        // "when cop is disabled in the configuration": the first enable is
        // legitimate, the second is redundant.
        let source =
            "foo\n# rubocop:enable Layout/LineLength\n# rubocop:enable Layout/LineLength\n";
        let seeded: Vec<Vec<String>> = directives_for(source)
            .redundant_enables(UNIVERSE.into_iter(), std::iter::once("Layout/LineLength"))
            .into_iter()
            .map(|(_, names)| names)
            .collect();
        assert_eq!(seeded, [["Layout/LineLength"]]);
        assert_eq!(extras(source), [["Layout/LineLength"], ["Layout/LineLength"]]);
    }

    #[test]
    fn inline_enable_is_ignored_and_span_covers_the_directive_text() {
        // Upstream's `comment_only_line?` guard: a trailing directive never
        // takes part. The reported span is the directive's own text.
        assert!(extras("foo # rubocop:enable Layout/LineLength\n").is_empty());
        let d = directives_for("foo\n# rubocop:enable Metrics/AbcSize\n");
        let (span, names) = d
            .redundant_enables(UNIVERSE.into_iter(), std::iter::empty())
            .pop()
            .expect("one redundant enable");
        assert_eq!(names, ["Metrics/AbcSize"]);
        assert_eq!(span, Span::new(4, 36));
    }

    #[test]
    fn unknown_cop_names_still_pair_up() {
        // A name the universe does not know stands for itself, so a
        // disable/enable pair of it is not redundant while a lone enable is.
        assert!(
            extras("# rubocop:disable Custom/Cop\nfoo\n# rubocop:enable Custom/Cop\n").is_empty()
        );
        assert_eq!(extras("foo\n# rubocop:enable Custom/Cop\n"), [["Custom/Cop"]]);
    }

    #[test]
    fn push_with_a_minus_argument_disables_until_the_pop() {
        // comment_config_spec.rb, "push/pop directives" > "temporarily
        // disable a cop for a problematic block".
        let d = directives_for(
            "def process_data(input)\n  result = input.upcase\n  # rubocop:push -Style/GuardClause\n  if result.present?\n    return result.strip\n  end\n  # rubocop:pop\n  nil\nend\n",
        );
        assert!(d.is_disabled("Style/GuardClause", 4));
        assert!(d.is_disabled("Style/GuardClause", 6));
        assert!(!d.is_disabled("Style/GuardClause", 2));
        assert!(!d.is_disabled("Style/GuardClause", 7));
        assert!(!d.is_disabled("Style/GuardClause", 8));
    }

    #[test]
    fn push_with_a_plus_argument_suspends_an_open_disable_until_the_pop() {
        // comment_config_spec.rb, "push/pop directives" > "enable a disabled
        // cop temporarily": the `pop` restores the disable the `push` saved.
        let d = directives_for(
            "# rubocop:disable Metrics/MethodLength\ndef long_method\n  line1\n  line2\n  # rubocop:push +Metrics/MethodLength\n  def short_method\n    line3\n  end\n  # rubocop:pop\n  line4\nend\n",
        );
        for line in [1, 2, 3, 4, 5] {
            assert!(d.is_disabled("Metrics/MethodLength", line), "line {line}");
        }
        for line in [6, 7, 8] {
            assert!(!d.is_disabled("Metrics/MethodLength", line), "line {line}");
        }
        for line in [9, 10, 11] {
            assert!(d.is_disabled("Metrics/MethodLength", line), "line {line}");
        }
    }

    #[test]
    fn an_unmatched_pop_leaves_the_state_alone() {
        // `pop_state` only runs `if @stack.any?`.
        let d = directives_for("# rubocop:disable Style/For\nfoo\n# rubocop:pop\nbar\n");
        assert!(d.is_disabled("Style/For", 2));
        assert!(d.is_disabled("Style/For", 4));
    }

    #[test]
    fn bare_next_applies_its_signed_arguments_to_the_statement_below() {
        // `apply_next_directive`: `-` disables the attached statement, `+`
        // suspends an open disable for it.
        let d = directives_for(
            "# rubocop:disable Style/Not\n# rubocop:next -Style/For +Style/Not\nfor y in [3, 4] do not y end\nnot z\n",
        );
        assert!(d.is_disabled("Style/For", 3));
        assert!(!d.is_disabled("Style/For", 4));
        assert!(!d.is_disabled("Style/Not", 3));
        assert!(d.is_disabled("Style/Not", 4));
        assert!(d.is_opted_in("Style/Not"));
    }

    #[test]
    fn a_next_statement_directive_scopes_to_the_whole_statement() {
        // `statement_bounds_at`: the multi-line statement below the
        // directive, heredoc terminator included.
        let d = directives_for(
            "# rubocop:disable-next Layout/LineLength\nfoo(<<~TEXT)\n  body\nTEXT\nbar\n",
        );
        assert!(d.is_disabled("Layout/LineLength", 2));
        assert!(d.is_disabled("Layout/LineLength", 4));
        assert!(!d.is_disabled("Layout/LineLength", 5));
    }

    #[test]
    fn a_next_statement_directive_chains_past_comments_but_not_blank_lines() {
        // `attached_code_line`: comment-only lines chain, a blank line
        // detaches the directive.
        let chained =
            directives_for("# rubocop:disable-next Style/For\n# a note\nfor x in y do x end\n");
        assert!(chained.is_disabled("Style/For", 3));
        assert_eq!(chained.detached_next_directives().count(), 0);

        let detached = directives_for("# rubocop:disable-next Style/For\n\nfor x in y do x end\n");
        assert!(!detached.is_disabled("Style/For", 3));
        assert_eq!(detached.detached_next_directives().count(), 1);
    }

    #[test]
    fn a_commented_out_directive_is_not_a_directive() {
        // `DirectiveComment#initialize` drops a match whose `pre_match` is
        // nothing but a second comment marker.
        let d = directives_for("# # rubocop:disable Style/For\nfor x in y do x end\n");
        assert!(d.directives().is_empty());
        assert!(!d.is_disabled("Style/For", 2));
    }

    #[test]
    fn push_and_pop_take_no_part_in_the_enable_pairing() {
        // `self_closing_directive?`: a `push -Cop` is not an extra enable,
        // and neither is the `pop` that closes it.
        assert!(extras("# rubocop:push -Layout/LineLength\nfoo\n# rubocop:pop\n").is_empty());
    }
}
