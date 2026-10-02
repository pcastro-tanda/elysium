//! `Style/DirectiveScope`, ported from RuboCop's
//! `lib/rubocop/cop/style/directive_scope.rb`, plus a private replay of
//! `RuboCop::CommentConfig`'s per-cop disabled-range analysis
//! (`lib/rubocop/comment_config.rb`, `comment_config/push_pop.rb`,
//! `comment_config/disable_next.rb`) that tracks which directive opened and
//! closed each disabled range, and a private replay of
//! `CommentConfig::DisableNext#statement_scope_after` generalized to an
//! arbitrary directive's own line (not just a `-next` directive's), since
//! `ruby_directives` only resolves that scope internally, for its own
//! `-next` directives.
//!
//! # No cop registry, so ranges are grouped by literal reference
//!
//! Same limitation `ruby_directives` and `lint/missing_cop_enable_directive.rs`
//! document: without a registry, a department or `all` reference is grouped by
//! its own literal spelling instead of the real cop names it would expand to.
//! This cop's own checks additionally require the *exact written cop list* to
//! match between a directive and its closing pair (`raw_cop_names.sort ==
//! ...sort`), which already rules out the one case the gap would otherwise
//! bite: a disable naming one spelling closed or reopened by a directive
//! naming a different (but overlapping) spelling is never treated as a pair
//! here, matching upstream's own decision to decline the conversion whenever
//! a department-wide disable is only partially closed.

use std::collections::BTreeMap;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::is_heredoc;
use ruby_ast::{walk, LocationExt, Node, NodeExt, Visitor};
use ruby_directives::{CopRef, Directive, DirectiveKind, Sign};
use ruby_source::{SourceFile, Span};

const MSG_ENABLE_PAIR: &str =
    "Use `enable-next` instead of an `enable`/`disable` pair around a single statement.";

/// Checks for directive scopes that can be expressed with the tighter
/// `disable-next`/`enable-next`/`next` forms.
#[derive(Debug, Clone)]
pub struct DirectiveScope;

impl Rule for DirectiveScope {
    const META: RuleMeta = RuleMeta {
        name: "Style/DirectiveScope",
        department: Department::Style,
        summary: "Checks for directive scopes that can be expressed with the tighter \
                   `disable-next` form.",
        explanation: "\
Checks for directive scopes that can be expressed with the tighter
next-statement forms: a `disable`/`enable` pair, an
`enable`/`disable` pair, or a `push`/`pop` with signed arguments
wrapping exactly one statement. A statement-scoped directive cannot
drift as the surrounding code changes and needs no closing boundary.

@safety
  The autocorrection is unsafe because the suppression scope shrinks
  from the whole region to the statement: offenses of the suppressed
  cops located on the directive lines themselves resurface.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "\
Ranges are grouped by each directive's own literal reference (`all`, a bare
department word, or a slash-qualified cop path) rather than by real,
registry-expanded cop name -- see the module docs for why the exact-cop-list
match this cop already requires between a directive and its closing pair
rules out the only case that gap would otherwise bite.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        check(ctx);
    }
}

// ---------------------------------------------------------------------------
// Line classification and statement extent (private copy of
// `ruby_directives`'s `line_kinds`/`statement_end_lines`/`statement_scope_after`,
// generalized to run for any line, not only a `-next` directive's).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineKind {
    Code,
    Comment,
    Blank,
}

fn is_inline(ctx: &Context<'_>, comment_start: u32) -> bool {
    let line = ctx.line_col(comment_start).line;
    let line_start = ctx.line_span(line).start;
    ctx.text(Span::new(line_start, comment_start)).iter().any(|b| !b.is_ascii_whitespace())
}

fn line_kinds(ctx: &Context<'_>) -> Vec<LineKind> {
    let count = ctx.line_count();
    let mut kinds: Vec<LineKind> = (1..=count)
        .map(|line| {
            if ctx.line_text(line).iter().all(u8::is_ascii_whitespace) {
                LineKind::Blank
            } else {
                LineKind::Code
            }
        })
        .collect();
    for comment in ctx.comments() {
        if is_inline(ctx, comment.span.start) {
            continue;
        }
        if let Some(kind) = comment.line.checked_sub(1).and_then(|i| kinds.get_mut(i as usize)) {
            *kind = LineKind::Comment;
        }
    }
    kinds
}

fn kind_at(kinds: &[LineKind], line: u32) -> Option<LineKind> {
    line.checked_sub(1).and_then(|i| kinds.get(i as usize)).copied()
}

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

/// `CommentConfig::DisableNext#statement_scope_after`, generalized to any
/// line (upstream calls it with a `-next` directive's own line only; this
/// cop also calls it with a plain `disable`/`push`/`enable` directive's line).
fn statement_scope_after(
    line: u32,
    kinds: &[LineKind],
    statement_ends: &BTreeMap<u32, u32>,
) -> Option<(u32, u32)> {
    if kind_at(kinds, line) == Some(LineKind::Code) {
        return None;
    }
    let code_line = attached_code_line(line, kinds)?;
    let end = statement_ends.get(&code_line).copied().unwrap_or(code_line);
    Some((code_line, end.max(code_line)))
}

/// `DisableNext#statement_bounds_at`/`#statement_end_line`, precomputed in one
/// post-order walk: the last line of the statement starting on each code line.
fn statement_end_lines(root: &Node<'_>, source: &SourceFile) -> BTreeMap<u32, u32> {
    struct Walker<'a> {
        source: &'a SourceFile,
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
            if let Some(begin) = node.as_begin_node() {
                for (line, end_line) in synthetic_rescue_ensure_lines(&begin, self.source) {
                    let entry = self.ends.entry(line).or_default();
                    *entry = (*entry).max(end_line);
                }
            }
        }
    }

    /// Parser-gem-compatible synthetic ranges for the implicit `:rescue`/
    /// `:ensure` wrapper nodes `Builders::Default#begin_body`/
    /// `#eh_keyword_map` construct around a `begin...end` body --
    /// reproduced by RuboCop's own Prism translation layer
    /// (`Prism::Translation::Parser::Compiler#visit_begin_node`, which
    /// calls that same builder). Each wrapper's own "expression" range
    /// starts at the *preceding* compound statement's start (not its own
    /// `rescue`/`ensure` keyword), through its own last statement (its
    /// `else` body, if present, for `:rescue`; otherwise its own keyword,
    /// if its own body is empty). A directive right after the body's
    /// first line must therefore see the *whole* rescue/ensure region as
    /// that statement's reach, not just the body, matching upstream.
    fn synthetic_rescue_ensure_lines(
        begin: &ruby_ast::node::BeginNode<'_>,
        source: &SourceFile,
    ) -> Vec<(u32, u32)> {
        let line_of = |span: Span| source.line_col(span.start).line;
        let last_line_of = |span: Span| source.line_col(span.end.saturating_sub(1)).line;
        let body_start_line = begin.statements().map(|s| line_of(s.as_node().span()));

        let mut out = Vec::new();
        let mut compstmt_start_line = body_start_line;

        if let Some(first_rescue) = begin.rescue_clause() {
            let start_line =
                compstmt_start_line.unwrap_or_else(|| line_of(first_rescue.keyword_loc().span()));
            let mut last = first_rescue;
            while let Some(next) = last.subsequent() {
                last = next;
            }
            let end_line = if let Some(else_node) = begin.else_clause() {
                else_node.statements().map_or_else(
                    || line_of(else_node.else_keyword_loc().span()),
                    |s| last_line_of(s.as_node().span()),
                )
            } else {
                last.statements().map_or_else(
                    || line_of(last.keyword_loc().span()),
                    |s| last_line_of(s.as_node().span()),
                )
            };
            out.push((start_line, end_line));
            compstmt_start_line = Some(start_line);
        }

        if let Some(ensure) = begin.ensure_clause() {
            let start_line =
                compstmt_start_line.unwrap_or_else(|| line_of(ensure.ensure_keyword_loc().span()));
            let end_line = ensure.statements().map_or_else(
                || line_of(ensure.ensure_keyword_loc().span()),
                |s| last_line_of(s.as_node().span()),
            );
            out.push((start_line, end_line));
        }

        out
    }

    let mut walker = Walker { source, stack: Vec::new(), ends: BTreeMap::new() };
    walk(root, &mut walker);
    walker.ends
}

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

// ---------------------------------------------------------------------------
// Per-literal-reference disabled-range replay with directive attribution
// (private replay of `CommentConfig#analyze`, specialized per literal cop
// reference, keeping which directive's `span` opened each finished range --
// upstream's `DirectiveRange#directive`).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct OpenedRange {
    end: Option<u32>,
    opener: Span,
}

#[derive(Debug, Clone, Default)]
struct RangeState {
    ranges: Vec<OpenedRange>,
    start: Option<(u32, Span)>,
    stack: Vec<Option<(u32, Span)>>,
}

impl RangeState {
    fn close(&mut self, line: u32) {
        if let Some((start, opener)) = self.start.take() {
            if line >= start {
                self.ranges.push(OpenedRange { end: Some(line), opener });
            }
        }
    }

    fn apply_plain(&mut self, directive: &Directive) {
        if directive.inline {
            if directive.kind.disables() {
                self.ranges.push(OpenedRange { end: Some(directive.line), opener: directive.span });
            }
            return;
        }
        self.close(directive.line);
        if directive.kind.disables() {
            self.start = Some((directive.line, directive.span));
        }
    }

    fn add_next_range(&mut self, scope: (u32, u32), opener: Span) {
        self.ranges.push(OpenedRange { end: Some(scope.1), opener });
        let _ = scope.0;
    }

    fn suspend(&mut self, scope: (u32, u32)) {
        let Some((_, opener)) = self.start else { return };
        self.close(scope.0.saturating_sub(1));
        self.start = Some((scope.1 + 1, opener));
    }

    fn apply_sign(&mut self, sign: Sign, line: u32, opener: Span) {
        match sign {
            Sign::Minus if self.start.is_none() => self.start = Some((line, opener)),
            Sign::Plus if self.start.is_some() => self.close(line),
            _ => {}
        }
    }

    fn push(&mut self) {
        self.stack.push(self.start);
    }

    fn pop(&mut self, line: u32) {
        let Some(restored) = self.stack.pop() else { return };
        self.close(line.saturating_sub(1));
        self.start = restored.map(|(_, opener)| (line, opener));
    }

    fn into_ranges(mut self) -> Vec<OpenedRange> {
        if let Some((_, opener)) = self.start {
            self.ranges.push(OpenedRange { end: None, opener });
        }
        self.ranges
    }
}

/// The literal key a cop reference is grouped under: `"all"`, the bare
/// department word, or the cop path as written.
fn cop_key(cop: &CopRef) -> &str {
    match cop {
        CopRef::All => "all",
        CopRef::Department(name) | CopRef::Cop(name) => name.as_str(),
    }
}

/// Replays every directive in source order, building one [`RangeState`] per
/// literal cop reference -- `CommentConfig#analyze`, specialized to a
/// registry-less, literal-keyed replay (see the module docs).
fn analyze(directives: &[Directive]) -> BTreeMap<String, Vec<OpenedRange>> {
    let mut states: BTreeMap<String, RangeState> = BTreeMap::new();
    for directive in directives {
        match directive.kind {
            DirectiveKind::Pop => {
                for state in states.values_mut() {
                    state.pop(directive.line);
                }
                continue;
            }
            DirectiveKind::Push => {
                for state in states.values_mut() {
                    state.push();
                }
            }
            _ => {}
        }
        for (index, cop) in directive.cops.iter().enumerate() {
            let state = states.entry(cop_key(cop).to_string()).or_default();
            let sign = directive.signs.get(index).copied();
            match directive.kind {
                DirectiveKind::Push => {
                    if let Some(sign) = sign {
                        state.apply_sign(sign, directive.line, directive.span);
                    }
                }
                DirectiveKind::Next => {
                    if let (Some(sign), Some(scope)) = (sign, directive.scope) {
                        match sign {
                            Sign::Minus => state.add_next_range(scope, directive.span),
                            Sign::Plus => state.suspend(scope),
                        }
                    }
                }
                DirectiveKind::DisableNext
                | DirectiveKind::TodoNext
                | DirectiveKind::EnableNext => {
                    if let Some(scope) = directive.scope {
                        if directive.kind.disables() {
                            state.add_next_range(scope, directive.span);
                        } else {
                            state.suspend(scope);
                        }
                    }
                }
                _ => state.apply_plain(directive),
            }
        }
    }
    states.into_iter().map(|(key, state)| (key, state.into_ranges())).collect()
}

// ---------------------------------------------------------------------------
// The cop itself.
// ---------------------------------------------------------------------------

fn comment_at_line(ctx: &Context<'_>, line: u32) -> Option<Span> {
    ctx.comments().iter().find(|comment| comment.line == line).map(|comment| comment.span)
}

fn mode_str(kind: DirectiveKind) -> &'static str {
    match kind {
        DirectiveKind::Todo => "todo",
        DirectiveKind::Enable => "enable",
        _ => "disable",
    }
}

fn sorted_names(cops: &[CopRef]) -> Vec<&str> {
    let mut names: Vec<&str> = cops.iter().map(cop_key).collect();
    names.sort_unstable();
    names
}

/// `\b#{word}\b`, replaced by `word-next`: the first ASCII-word-bounded
/// occurrence of `word` in `text`, with `-next` spliced in right after it.
fn insert_next_after(text: &[u8], word: &str) -> Vec<u8> {
    let word = word.as_bytes();
    let is_word_byte = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut i = 0usize;
    while i + word.len() <= text.len() {
        if &text[i..i + word.len()] == word {
            let before_ok = i == 0 || !is_word_byte(text[i - 1]);
            let after = i + word.len();
            let after_ok = after == text.len() || !is_word_byte(text[after]);
            if before_ok && after_ok {
                let mut out = Vec::with_capacity(text.len() + 5);
                out.extend_from_slice(&text[..after]);
                out.extend_from_slice(b"-next");
                out.extend_from_slice(&text[after..]);
                return out;
            }
        }
        i += 1;
    }
    text.to_vec()
}

fn ltrim(bytes: &[u8]) -> &[u8] {
    let start = bytes.iter().position(|b| !b.is_ascii_whitespace()).unwrap_or(bytes.len());
    &bytes[start..]
}

/// `DirectiveComment#reason`: the directive's optional `-- ...` trailing
/// comment, found after its recognized [`Directive::span`] within the full
/// comment text.
fn reason(ctx: &Context<'_>, directive: &Directive, comment_span: Span) -> Option<String> {
    let full = ctx.text(comment_span);
    let offset = (directive.span.end - comment_span.start) as usize;
    let tail = ltrim(full.get(offset..)?);
    let tail = tail.strip_prefix(b"--")?;
    let text = std::str::from_utf8(tail).ok()?.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

fn replacement_mode(directive: &Directive) -> &'static str {
    if directive.signs.iter().all(|s| *s == Sign::Minus) {
        "disable-next"
    } else if directive.signs.iter().all(|s| *s == Sign::Plus) {
        "enable-next"
    } else {
        "next"
    }
}

fn push_replacement(ctx: &Context<'_>, directive: &Directive, comment_span: Span) -> Vec<u8> {
    let mode = replacement_mode(directive);
    let body = match mode {
        "disable-next" | "enable-next" => {
            let sign = if mode == "disable-next" { Sign::Minus } else { Sign::Plus };
            let names: Vec<&str> = directive
                .signed_args()
                .filter(|(s, _)| *s == sign)
                .map(|(_, cop)| cop_key(cop))
                .collect();
            format!("# rubocop:{mode} {}", names.join(", "))
        }
        _ => {
            let tokens: Vec<String> = directive
                .signed_args()
                .map(|(sign, cop)| {
                    let prefix = if sign == Sign::Minus { '-' } else { '+' };
                    format!("{prefix}{}", cop_key(cop))
                })
                .collect();
            format!("# rubocop:next {}", tokens.join(" "))
        }
    };
    let text = match reason(ctx, directive, comment_span) {
        Some(r) => format!("{body} -- {r}"),
        None => body,
    };
    text.into_bytes()
}

/// `RuboCop::Cop::Style::DirectiveScope#on_new_investigation`.
fn check(ctx: &mut Context<'_>) {
    let directives: Vec<Directive> = ctx.directives().directives().to_vec();
    if directives.is_empty() {
        return;
    }

    let ranges = analyze(&directives);
    let kinds = line_kinds(ctx);
    let statement_ends = statement_end_lines(&ctx.parsed().root(), ctx.source());
    let scope_after = |line: u32| statement_scope_after(line, &kinds, &statement_ends);

    for directive in &directives {
        if !ctx.directives().comment_only_line(directive.line) {
            continue;
        }
        let plain_disable = directive.kind.disables()
            && !matches!(directive.kind, DirectiveKind::DisableNext | DirectiveKind::TodoNext);
        let signed_push = directive.kind == DirectiveKind::Push && !directive.signs.is_empty();
        let plain_enable = directive.kind == DirectiveKind::Enable
            && !directive.cops.iter().any(|cop| matches!(cop, CopRef::All));

        if plain_disable {
            check_pair(ctx, directive, &directives, &ranges, scope_after);
        } else if signed_push {
            check_push_pop(ctx, directive, &directives, scope_after);
        } else if plain_enable {
            check_enable_pair(ctx, directive, &ranges, scope_after);
        }
    }
}

/// `#check_pair`/`#single_statement_closing_directive`/`#single_closing_line`/
/// `#ranges_opened_by`.
fn check_pair(
    ctx: &mut Context<'_>,
    directive: &Directive,
    all_directives: &[Directive],
    ranges: &BTreeMap<String, Vec<OpenedRange>>,
    scope_after: impl Fn(u32) -> Option<(u32, u32)>,
) {
    let mut ends: Vec<Option<u32>> = Vec::new();
    for cop in &directive.cops {
        let Some(cop_ranges) = ranges.get(cop_key(cop)) else { return };
        for range in cop_ranges {
            if range.opener == directive.span {
                ends.push(range.end);
            }
        }
    }
    if ends.is_empty() {
        return;
    }
    let first = ends[0];
    if !ends.iter().all(|e| *e == first) {
        return;
    }
    let Some(closing_line) = first else { return };

    let Some(enable) = all_directives.iter().find(|d| d.line == closing_line) else { return };
    if enable.kind != DirectiveKind::Enable {
        return;
    }
    if !wraps_single_statement(directive.line, closing_line, &scope_after) {
        return;
    }
    if sorted_names(&enable.cops) != sorted_names(&directive.cops) {
        return;
    }

    let Some(comment_span) = comment_at_line(ctx, directive.line) else { return };
    let Some(enable_span) = comment_at_line(ctx, closing_line) else { return };

    let mode = mode_str(directive.kind);
    let message =
        format!("Use `{mode}-next` instead of a `{mode}`/`enable` pair around a single statement.");
    let replacement = insert_next_after(ctx.text(comment_span), mode);
    let edits =
        vec![Edit::replace(comment_span, replacement), Edit::delete(ctx.whole_lines(enable_span))];
    ctx.report_with_fix(
        &DirectiveScope::META,
        comment_span,
        message,
        Fix { applicability: Applicability::Unsafe, edits },
    );
}

/// `#check_push_pop`/`#balancing_pop_line`/`#push_replacement`.
fn check_push_pop(
    ctx: &mut Context<'_>,
    directive: &Directive,
    all_directives: &[Directive],
    scope_after: impl Fn(u32) -> Option<(u32, u32)>,
) {
    let Some(pop_line) = balancing_pop_line(directive, all_directives) else { return };
    if !ctx.directives().comment_only_line(pop_line) {
        return;
    }
    if !wraps_single_statement(directive.line, pop_line, &scope_after) {
        return;
    }
    let Some(comment_span) = comment_at_line(ctx, directive.line) else { return };
    let Some(pop_span) = comment_at_line(ctx, pop_line) else { return };

    let message = format!(
        "Use `{}` instead of `push`/`pop` around a single statement.",
        replacement_mode(directive)
    );
    let replacement = push_replacement(ctx, directive, comment_span);
    let edits =
        vec![Edit::replace(comment_span, replacement), Edit::delete(ctx.whole_lines(pop_span))];
    ctx.report_with_fix(
        &DirectiveScope::META,
        comment_span,
        message,
        Fix { applicability: Applicability::Unsafe, edits },
    );
}

fn balancing_pop_line(push_directive: &Directive, all_directives: &[Directive]) -> Option<u32> {
    let mut depth = 0u32;
    for directive in all_directives.iter().filter(|d| d.line > push_directive.line) {
        match directive.kind {
            DirectiveKind::Push => depth += 1,
            DirectiveKind::Pop => {
                if depth == 0 {
                    return Some(directive.line);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

/// `#check_enable_pair`/`#enable_pair_closing`/`#re_disable_below`/
/// `#closed_open_disables?`.
fn check_enable_pair(
    ctx: &mut Context<'_>,
    directive: &Directive,
    ranges: &BTreeMap<String, Vec<OpenedRange>>,
    scope_after: impl Fn(u32) -> Option<(u32, u32)>,
) {
    let Some(scope) = scope_after(directive.line) else { return };
    if scope.0 != directive.line + 1 {
        return;
    }
    let closing_line = scope.1 + 1;
    if !ctx.directives().comment_only_line(closing_line) {
        return;
    }
    let Some(closing) = ctx.directives().directives().iter().find(|d| d.line == closing_line)
    else {
        return;
    };
    let closing_disables = closing.kind.disables()
        && !matches!(closing.kind, DirectiveKind::DisableNext | DirectiveKind::TodoNext);
    if !closing_disables {
        return;
    }
    if sorted_names(&closing.cops) != sorted_names(&directive.cops) {
        return;
    }

    for cop in &directive.cops {
        let Some(cop_ranges) = ranges.get(cop_key(cop)) else { return };
        let closed_at = cop_ranges.iter().any(|r| r.end == Some(directive.line));
        let reopened_by = cop_ranges.iter().any(|r| r.opener == closing.span);
        if !(closed_at && reopened_by) {
            return;
        }
    }

    let Some(comment_span) = comment_at_line(ctx, directive.line) else { return };
    let Some(closing_span) = comment_at_line(ctx, closing_line) else { return };

    let replacement = insert_next_after(ctx.text(comment_span), "enable");
    let edits =
        vec![Edit::replace(comment_span, replacement), Edit::delete(ctx.whole_lines(closing_span))];
    ctx.report_with_fix(
        &DirectiveScope::META,
        comment_span,
        MSG_ENABLE_PAIR,
        Fix { applicability: Applicability::Unsafe, edits },
    );
}

/// `#wraps_single_statement?`.
fn wraps_single_statement(
    directive_line: u32,
    closing_line: u32,
    scope_after: impl Fn(u32) -> Option<(u32, u32)>,
) -> bool {
    let Some(scope) = scope_after(directive_line) else { return false };
    scope.0 == directive_line + 1 && scope.1 + 1 == closing_line
}
