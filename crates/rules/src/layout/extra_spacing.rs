//! `Layout/ExtraSpacing`, ported from RuboCop's `lib/rubocop/cop/layout/extra_spacing.rb`
//! plus the `PrecedingFollowingAlignment` mixin
//! (`lib/rubocop/cop/mixin/preceding_following_alignment.rb`) it includes.
//!
//! RuboCop drives this cop entirely off the real token stream
//! (`processed_source.tokens.each_cons(2)`). Prism does not hand this engine
//! a token stream, so this port approximates one:
//!
//! - "Opaque" byte ranges (string/x-string/regexp content, quoted-symbol
//!   text, and every comment) are collected while the tree is walked, using
//!   each literal node's own `content_loc`/`value_loc` -- never the node's
//!   outer span -- so a heredoc's opening tag line still has its trailing
//!   code (after `<<~ID`) checked normally; only the body (and, for
//!   interpolated literals, each literal segment) is opaque.
//! - Runs of two or more space characters on one physical line, outside
//!   every opaque range, are "extra spacing" candidates, mirroring
//!   `token1.end_pos .. token2.begin_pos` for adjacent real tokens: a run
//!   touching column 0 (nothing precedes it on the line) or the line's end
//!   (nothing follows) is never a candidate, matching RuboCop's `token1.line
//!   != token2.line` early return and its indifference to trailing
//!   whitespace.
//! - `AllowForAlignment`'s "is this column meaningful elsewhere" checks
//!   (`aligned_words?`/`aligned_equals_operator?`) need one following
//!   token's text and length; [`token_extent`] is a small lexical
//!   tokenizer (identifiers/ivars/gvars as word runs, `.`/`..`/`...`/`::`
//!   as their own tokens, everything else in RuboCop's operator alphabet as
//!   a greedy run) good enough to reproduce the mixin's exact-text
//!   fallback, not a full Ripper-grade lexer.
//! - `ForceEqualSignAlignment`'s assignment operators are found the same
//!   way RuboCop's own `assignment_tokens` are: lexically, by scanning for
//!   `=`-ending operators outside every opaque range, then subtracting the
//!   two AST-only exclusions the mixin's `remove_equals_in_def` applies
//!   (`OptionalParameterNode`'s default-value `=`, and an endless
//!   `DefNode`'s own `=`) -- rather than enumerating every Prism
//!   assignment-node kind, which do not uniformly expose an operator
//!   location (plain attribute writers such as `a.b = 1` are ordinary
//!   `CallNode`s with no operator field at all).
//!
//! See `META.blind_spots` for the behavioural gaps this approximation
//! knowingly accepts.

use std::collections::{BTreeMap, HashSet};

use linter::{
    Applicability, CommentInfo, ConfigDefault, ConfigOption, Context, Department, Edit, Fix,
    FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG_UNNECESSARY`.
const MSG_UNNECESSARY: &str = "Unnecessary spacing detected.";
/// RuboCop's `MSG_UNALIGNED_ASGN`, with `location` always `"preceding"` (the
/// only value this cop ever formats it with).
const MSG_UNALIGNED_ASGN: &str = "`=` is not aligned with the preceding assignment.";

/// Ruby's operator alphabet, used both to detect assignment-operator
/// lexemes and by [`token_extent`]'s greedy operator-run fallback.
const OP_CHARS: &[u8] = b"+-*/%=<>!&|^~";

/// Narrows a byte offset back to `u32`, saturating instead of panicking;
/// files large enough to overflow `u32` are not a concern here.
fn u32_of(x: usize) -> u32 {
    u32::try_from(x).unwrap_or(u32::MAX)
}

/// Read-only per-file facts [`ExtraSpacing::scan_lines`] and its helpers
/// consult but never mutate, bundled to keep call sites short.
struct Analysis<'a> {
    comments: &'a [CommentInfo],
    /// Lines of two *consecutive* (in file order, not necessarily adjacent)
    /// comments that start at the same column -- RuboCop's cop-level
    /// `@aligned_comments`.
    comment_pair_aligned: &'a HashSet<u32>,
    /// Lines whose comment is the first thing on the line -- RuboCop's
    /// mixin-level `aligned_comment_lines`, which excludes such lines from
    /// serving as an alignment reference for *other* tokens.
    standalone_comment_lines: &'a HashSet<u32>,
    /// The first `=`-ending operator per line, lexically scanned. RuboCop's
    /// `assignment_tokens`.
    assignment_map: &'a BTreeMap<u32, Span>,
}

/// Looks for extra/unnecessary whitespace.
#[derive(Debug, Clone)]
pub struct ExtraSpacing {
    allow_for_alignment: bool,
    allow_before_trailing_comments: bool,
    force_equal_sign_alignment: bool,
    /// Content-only spans of string/x-string/regexp/quoted-symbol literals,
    /// gathered while the tree is walked.
    opaque: Vec<Span>,
    /// `(key.end_pos, value.begin_pos)` for every pair of a *multiline*
    /// hash literal or bare keyword-argument hash -- RuboCop's
    /// `ignored_ranges`, deferring that spacing to `Layout/HashAlignment`.
    ignored_ranges: Vec<Span>,
    /// Byte offsets of `=` characters that are never assignment operators:
    /// an optional parameter's default-value operator, or an endless
    /// method definition's own `=`.
    excluded_equals: HashSet<u32>,
}

impl Rule for ExtraSpacing {
    const META: RuleMeta = RuleMeta {
        name: "Layout/ExtraSpacing",
        department: Department::Layout,
        summary: "Checks for extra/unnecessary whitespace.",
        explanation: "\
```ruby
# good if AllowForAlignment is true
name      = \"RuboCop\"
# Some comment and an empty line

website  += \"/rubocop/rubocop\" unless cond
puts        \"rubocop\"          if     debug

# bad for any configuration
set_app(\"RuboCop\")
website  = \"https://github.com/rubocop/rubocop\"

# good only if AllowBeforeTrailingComments is true
object.method(arg)  # this is a comment

# good even if AllowBeforeTrailingComments is false or not set
object.method(arg) # this is a comment

# good with either AllowBeforeTrailingComments or AllowForAlignment
object.method(arg)         # this is a comment
another_object.method(arg) # this is another comment
some_object.method(arg)    # this is some comment
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[
            NodeKind::StringNode,
            NodeKind::XStringNode,
            NodeKind::RegularExpressionNode,
            NodeKind::SymbolNode,
            NodeKind::AssocNode,
            NodeKind::OptionalParameterNode,
            NodeKind::DefNode,
        ],
        config: &[
            ConfigOption {
                name: "AllowForAlignment",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Allow extra spacing that lines up code across adjacent lines.",
            },
            ConfigOption {
                name: "AllowBeforeTrailingComments",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Allow extra spacing before a trailing end-of-line comment.",
            },
            ConfigOption {
                name: "ForceEqualSignAlignment",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Force the alignment of `=` in assignments on consecutive lines.",
            },
        ],
        blind_spots: "\
There is no real token stream to work from (see the module docs for the
approximation this file builds instead), which has these consequences:

- `AllowForAlignment`'s alignment search only matches RuboCop's own
  `ASSIGNMENT_OR_COMPARISON_TOKENS` for the equals-sign fallback with plain
  `=` and the compound assignment operators (`+=`, `-=`, ..., `**=`,
  `<<=`, `>>=`, `||=`, `&&=`); comparison operators (`==`, `===`, `!=`,
  `<=`, `>=`) and the bare `<<` append operator are not tracked as
  alignment landmarks. This can only under-recognize an alignment RuboCop
  would allow (an over-reporting risk), not the reverse; no fixture in
  this port exercises it.
- The mixin's second `aligned_with_any_line_range?` pass (retrying with a
  `base_indentation` filter after an unfiltered scan already failed) is
  not implemented: for this cop's call sites the filtered scan is always a
  strict subset of lines the unfiltered scan already visited with the same
  predicate, so it can never change the result.
- Column/token comparisons index by byte offset within a line, i.e. assume
  one byte per character; a line with multi-byte UTF-8 content before the
  compared column can misalign the comparison (offense spans themselves
  remain exact byte spans, unaffected).
- `token_extent`'s lexical tokenizer (identifiers, `.`/`..`/`...`, `::`,
  and a greedy run of RuboCop's operator-alphabet characters) is not a
  full Ruby lexer; an unusual unspaced operator sequence could glom more
  characters into `AllowForAlignment`'s exact-text fallback than a real
  token would, which can only make that fallback harder to satisfy.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            allow_for_alignment: options.bool("AllowForAlignment"),
            allow_before_trailing_comments: options.bool("AllowBeforeTrailingComments"),
            force_equal_sign_alignment: options.bool("ForceEqualSignAlignment"),
            opaque: Vec::new(),
            ignored_ranges: Vec::new(),
            excluded_equals: HashSet::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.opaque.clear();
        self.ignored_ranges.clear();
        self.excluded_equals.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::StringNode => {
                if let Some(n) = node.as_string_node() {
                    self.opaque.push(n.content_loc().span());
                }
            }
            NodeKind::XStringNode => {
                if let Some(n) = node.as_x_string_node() {
                    self.opaque.push(n.content_loc().span());
                }
            }
            NodeKind::RegularExpressionNode => {
                if let Some(n) = node.as_regular_expression_node() {
                    self.opaque.push(n.content_loc().span());
                }
            }
            NodeKind::SymbolNode => {
                if let Some(loc) = node.as_symbol_node().and_then(|n| n.value_loc()) {
                    self.opaque.push(loc.span());
                }
            }
            NodeKind::OptionalParameterNode => {
                if let Some(n) = node.as_optional_parameter_node() {
                    self.excluded_equals.insert(n.operator_loc().span().start);
                }
            }
            NodeKind::DefNode => {
                if let Some(loc) = node.as_def_node().and_then(|n| n.equal_loc()) {
                    self.excluded_equals.insert(loc.span().start);
                }
            }
            NodeKind::AssocNode => self.record_ignored_range(node, ctx),
            _ => {}
        }
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        self.opaque.sort_by_key(|s| s.start);
        self.ignored_ranges.sort_by_key(|s| s.start);

        let comments: Vec<CommentInfo> = ctx.comments().to_vec();
        let comment_pair_aligned = comment_pair_aligned_lines(&comments, ctx);
        let standalone_comment_lines = standalone_comment_lines(&comments, ctx);
        let assignment_map = self.scan_assignment_operators(ctx, &comments);

        let analysis = Analysis {
            comments: &comments,
            comment_pair_aligned: &comment_pair_aligned,
            standalone_comment_lines: &standalone_comment_lines,
            assignment_map: &assignment_map,
        };
        self.scan_lines(ctx, &analysis);

        if self.force_equal_sign_alignment {
            Self::check_assignment_alignment(ctx, &assignment_map);
        }
    }
}

impl ExtraSpacing {
    /// RuboCop's `ignored_ranges`: the key-value gap of a pair belonging to
    /// a *multiline* hash literal or bare keyword-argument hash, deferred
    /// to `Layout/HashAlignment`.
    fn record_ignored_range(&mut self, node: &Node<'_>, ctx: &Context<'_>) {
        let Some(parent) = ctx.parent() else { return };
        if !matches!(parent.kind, NodeKind::HashNode | NodeKind::KeywordHashNode) {
            return;
        }
        let start_line = ctx.line_col(parent.span.start).line;
        let end_line = ctx.line_col(parent.span.end.saturating_sub(1)).line;
        if start_line == end_line {
            return;
        }
        let Some(assoc) = node.as_assoc_node() else { return };
        let key_end = assoc.key().span().end;
        let value_start = assoc.value().span().start;
        if value_start > key_end {
            self.ignored_ranges.push(Span::new(key_end, value_start));
        }
    }

    /// True when `offset` falls inside a multiline hash pair's ignored
    /// key-value gap.
    fn in_ignored_range(&self, offset: u32) -> bool {
        self.ignored_ranges.iter().any(|s| s.start <= offset && offset < s.end)
    }

    /// RuboCop's `assignment_tokens`: the first `=`-ending operator per
    /// line, found lexically (see the module docs), skipping every opaque
    /// range, every comment, and the two AST-only exclusions RuboCop's
    /// `remove_equals_in_def` applies.
    fn scan_assignment_operators(
        &self,
        ctx: &Context<'_>,
        comments: &[CommentInfo],
    ) -> BTreeMap<u32, Span> {
        let bytes = ctx.source().bytes();
        let mut skip: Vec<Span> = self.opaque.clone();
        skip.extend(comments.iter().map(|c| c.span));
        skip.sort_by_key(|s| s.start);

        let mut map = BTreeMap::new();
        let len = bytes.len();
        let mut i = 0usize;
        let mut skip_idx = 0usize;
        while i < len {
            while skip_idx < skip.len() && (skip[skip_idx].end as usize) <= i {
                skip_idx += 1;
            }
            if skip_idx < skip.len() && (skip[skip_idx].start as usize) <= i {
                i = skip[skip_idx].end as usize;
                continue;
            }
            if bytes[i] != b'=' {
                i += 1;
                continue;
            }
            if self.excluded_equals.contains(&u32_of(i)) {
                i += 1;
                continue;
            }
            let next = bytes.get(i + 1).copied();
            let prev = if i > 0 { Some(bytes[i - 1]) } else { None };
            if next == Some(b'=') {
                // `==`/`===`: not an assignment; skip the whole run of `=`.
                let mut j = i;
                while j < len && bytes[j] == b'=' {
                    j += 1;
                }
                i = j;
                continue;
            }
            if next == Some(b'>') || next == Some(b'~') {
                // `=>`/`=~`: not an assignment.
                i += 1;
                continue;
            }
            let three_char_compound = i >= 2
                && matches!(
                    &bytes[i - 2..i],
                    [b'*', b'*'] | [b'<', b'<'] | [b'>', b'>'] | [b'|', b'|'] | [b'&', b'&']
                );
            let start = if three_char_compound {
                i - 2
            } else if matches!(prev, Some(b'!' | b'<' | b'>')) {
                // `!=`/`<=`/`>=`: a comparison, not an assignment.
                i += 1;
                continue;
            } else if i >= 1
                && matches!(bytes[i - 1], b'+' | b'-' | b'*' | b'/' | b'%' | b'|' | b'&' | b'^')
            {
                i - 1
            } else {
                i
            };
            let op_end = i + 1;
            let span = Span::new(u32_of(start), u32_of(op_end));
            let line = ctx.line_col(span.start).line;
            map.entry(line).or_insert(span);
            i = op_end;
        }
        map
    }

    /// Scans every physical line for runs of two or more spaces outside
    /// every opaque range, reporting each candidate that survives
    /// `ignored_ranges`, `AllowBeforeTrailingComments`, `AllowForAlignment`,
    /// and (when the run precedes a line's own force-aligned assignment
    /// operator) deferring to [`Self::check_assignment_alignment`].
    fn scan_lines(&self, ctx: &mut Context<'_>, a: &Analysis<'_>) {
        for line in 1..=ctx.line_count() {
            let line_span = ctx.line_span(line);
            let line_bytes = ctx.line_text(line);
            let len = line_bytes.len();
            if len < 2 {
                continue;
            }

            let mut spans: Vec<Span> = self
                .opaque
                .iter()
                .chain(a.comments.iter().map(|c| &c.span))
                .copied()
                .filter(|s| s.start < line_span.end && line_span.start < s.end)
                .collect();
            spans.sort_by_key(|s| s.start);
            let local_starts: Vec<usize> = spans
                .iter()
                .map(|s| (s.start.max(line_span.start) - line_span.start) as usize)
                .collect();

            let mut i = 0usize;
            let mut span_idx = 0usize;
            while i < len {
                while span_idx < spans.len()
                    && (spans[span_idx].end.min(line_span.end) - line_span.start) as usize <= i
                {
                    span_idx += 1;
                }
                if span_idx < spans.len() {
                    let s_start =
                        (spans[span_idx].start.max(line_span.start) - line_span.start) as usize;
                    if s_start <= i {
                        i = (spans[span_idx].end.min(line_span.end) - line_span.start) as usize;
                        continue;
                    }
                }
                if line_bytes[i] != b' ' {
                    i += 1;
                    continue;
                }
                let run_start_local = i;
                let mut j = i;
                while j < len && line_bytes[j] == b' ' {
                    if span_idx < spans.len() {
                        let s_start =
                            (spans[span_idx].start.max(line_span.start) - line_span.start) as usize;
                        if s_start == j {
                            break;
                        }
                    }
                    j += 1;
                }
                if j - run_start_local >= 2 && run_start_local != 0 && j != len {
                    let run_start = line_span.start + u32_of(run_start_local);
                    let run_end = line_span.start + u32_of(j);
                    self.handle_run(
                        ctx,
                        run_start,
                        run_end,
                        line,
                        line_bytes,
                        line_span,
                        &local_starts,
                        a,
                    );
                }
                i = j;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_run(
        &self,
        ctx: &mut Context<'_>,
        run_start: u32,
        run_end: u32,
        line: u32,
        line_bytes: &[u8],
        line_span: Span,
        local_starts: &[usize],
        a: &Analysis<'_>,
    ) {
        if self.force_equal_sign_alignment {
            if let Some(&op) = a.assignment_map.get(&line) {
                if op.start == run_end {
                    // The gap right before the line's own force-aligned
                    // assignment operator: handled entirely by
                    // `check_assignment_alignment`, never as plain extra
                    // spacing.
                    return;
                }
            }
        }
        if let Some(comment) = a.comments.iter().find(|c| c.span.start == run_end) {
            if self.allow_before_trailing_comments || self.in_ignored_range(run_start) {
                return;
            }
            if self.allow_for_alignment && a.comment_pair_aligned.contains(&comment.line) {
                return;
            }
            report_unnecessary(ctx, run_start, run_end);
            return;
        }
        if self.in_ignored_range(run_start) {
            return;
        }
        if self.allow_for_alignment
            && aligned_with_something(ctx, run_end, line, line_bytes, line_span, local_starts, a)
        {
            return;
        }
        report_unnecessary(ctx, run_start, run_end);
    }

    /// RuboCop's `check_assignment`/`align_equal_signs`: for every line
    /// whose first assignment operator is not aligned with the nearest
    /// preceding same-indent assignment line, reports it and attaches a fix
    /// realigning the whole contiguous block.
    fn check_assignment_alignment(ctx: &mut Context<'_>, assignment_map: &BTreeMap<u32, Span>) {
        for (&line, &op_span) in assignment_map {
            let candidates = relevant_assignment_lines(ctx, assignment_map, line, -1);
            if candidates.len() < 2 {
                continue;
            }
            let relevant_line = candidates[1];
            let Some(&other_op) = assignment_map.get(&relevant_line) else { continue };
            let this_end_col = ctx.line_col(op_span.end).column;
            let other_end_col = ctx.line_col(other_op.end).column;
            if this_end_col == other_end_col {
                continue;
            }
            let edits = build_alignment_edits(ctx, assignment_map, line);
            let fix = Fix { applicability: Applicability::Safe, edits };
            ctx.report_with_fix(&Self::META, op_span, MSG_UNALIGNED_ASGN, fix);
        }
    }
}

fn report_unnecessary(ctx: &mut Context<'_>, run_start: u32, run_end: u32) {
    let span = Span::new(run_start, run_end - 1);
    let fix = Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(span)] };
    ctx.report_with_fix(&ExtraSpacing::META, span, MSG_UNNECESSARY, fix);
}

/// RuboCop's cop-level `aligned_locations`: lines of two *consecutive* (in
/// file order) comments that start at the same column.
fn comment_pair_aligned_lines(comments: &[CommentInfo], ctx: &Context<'_>) -> HashSet<u32> {
    let mut set = HashSet::new();
    for pair in comments.windows(2) {
        let c1 = ctx.line_col(pair[0].span.start).column;
        let c2 = ctx.line_col(pair[1].span.start).column;
        if c1 == c2 {
            set.insert(pair[0].line);
            set.insert(pair[1].line);
        }
    }
    set
}

/// RuboCop's mixin-level `aligned_comment_lines`: lines whose comment
/// begins its own line (only whitespace precedes it).
fn standalone_comment_lines(comments: &[CommentInfo], ctx: &Context<'_>) -> HashSet<u32> {
    comments
        .iter()
        .filter(|c| {
            let line_span = ctx.line_span(c.line);
            let prefix_len = (c.span.start - line_span.start) as usize;
            ctx.line_text(c.line)
                .get(..prefix_len)
                .is_some_and(|prefix| prefix.iter().all(|&b| b == b' ' || b == b'\t'))
        })
        .map(|c| c.line)
        .collect()
}

/// RuboCop's `aligned_with_something?`: does `pos` (the start of the token
/// right after an extra-spacing run) line up with something meaningful on
/// another line?
#[allow(clippy::too_many_arguments)]
fn aligned_with_something(
    ctx: &Context<'_>,
    pos: u32,
    line: u32,
    line_bytes: &[u8],
    line_span: Span,
    local_starts: &[usize],
    a: &Analysis<'_>,
) -> bool {
    let col = (pos - line_span.start) as usize;
    let end = token_extent(line_bytes, col, local_starts);
    let token_text = &line_bytes[col..end];

    (1..line).rev().any(|candidate| check_line_alignment(ctx, candidate, col, token_text, a))
        || ((line + 1)..=ctx.line_count())
            .any(|candidate| check_line_alignment(ctx, candidate, col, token_text, a))
}

/// RuboCop's `aligned_token?`: `aligned_words?` (a space-then-non-space
/// column match, or an exact token-text match) or `aligned_equals_operator?`
/// (the candidate's own end column matches an adjacent `=`-ending token).
fn check_line_alignment(
    ctx: &Context<'_>,
    candidate: u32,
    col: usize,
    token_text: &[u8],
    a: &Analysis<'_>,
) -> bool {
    let line_bytes = ctx.line_text(candidate);
    if line_bytes.iter().all(|&b| b == b' ' || b == b'\t') {
        return false;
    }
    if a.standalone_comment_lines.contains(&candidate) {
        return false;
    }
    if col >= 1 {
        if let (Some(&before), Some(&at)) = (line_bytes.get(col - 1), line_bytes.get(col)) {
            if before == b' ' && at != b' ' {
                return true;
            }
        }
    }
    if !token_text.is_empty() && line_bytes.get(col..col + token_text.len()) == Some(token_text) {
        return true;
    }
    if token_text.last() == Some(&b'=') {
        if let Some(&op) = a.assignment_map.get(&candidate) {
            let this_end_col = col + token_text.len();
            let op_end_col = (op.end - ctx.line_span(candidate).start) as usize;
            if this_end_col == op_end_col {
                return true;
            }
        }
    }
    false
}

/// A minimal lexical tokenizer used only by `AllowForAlignment`'s exact-text
/// fallback: identifiers/ivars/gvars/keywords as word runs, `.`/`..`/`...`
/// and `::` as their own tokens, everything else in Ruby's operator
/// alphabet as a greedy run, else a single byte.
fn token_extent(line_bytes: &[u8], start: usize, local_opaque_starts: &[usize]) -> usize {
    fn is_word(b: u8) -> bool {
        b.is_ascii_alphanumeric() || b == b'_'
    }

    if start >= line_bytes.len() {
        return start;
    }
    let c = line_bytes[start];
    if c == b'@' || c == b'$' {
        let mut end = start + 1;
        if c == b'@' && line_bytes.get(end) == Some(&b'@') {
            end += 1;
        }
        while end < line_bytes.len() && is_word(line_bytes[end]) {
            end += 1;
        }
        return end;
    }
    if is_word(c) {
        let mut end = start;
        while end < line_bytes.len()
            && (is_word(line_bytes[end]) || line_bytes[end] == b'?' || line_bytes[end] == b'!')
            && !local_opaque_starts.contains(&end)
        {
            end += 1;
        }
        return end;
    }
    if c == b'.' {
        let mut end = start + 1;
        if line_bytes.get(end) == Some(&b'.') {
            end += 1;
            if line_bytes.get(end) == Some(&b'.') {
                end += 1;
            }
        }
        return end;
    }
    if c == b':' && line_bytes.get(start + 1) == Some(&b':') {
        return start + 2;
    }
    if OP_CHARS.contains(&c) {
        let mut end = start;
        while end < line_bytes.len()
            && OP_CHARS.contains(&line_bytes[end])
            && !local_opaque_starts.contains(&end)
        {
            end += 1;
        }
        return end;
    }
    start + 1
}

/// RuboCop's `relevant_assignment_lines`: walking from `start_line` in
/// `dir` (`-1` upward through the file, `1` downward), the same-indent
/// assignment lines in the same contiguous block (stopping at a dedent or a
/// blank line at the current indent level).
fn relevant_assignment_lines(
    ctx: &Context<'_>,
    assignment_map: &BTreeMap<u32, Span>,
    start_line: u32,
    dir: i64,
) -> Vec<u32> {
    let mut result = Vec::new();
    let total = i64::from(ctx.line_count());
    let original_indent = line_indentation(ctx.line_text(start_line));
    let mut relevant_at_level = true;
    let mut line = i64::from(start_line);
    while line >= 1 && line <= total {
        let ln = u32::try_from(line).unwrap_or(u32::MAX);
        let text = ctx.line_text(ln);
        let indent = line_indentation(text);
        let blank = is_blank_line(text);
        if (indent < original_indent && !blank) || (relevant_at_level && blank) {
            break;
        }
        if indent == original_indent && assignment_map.contains_key(&ln) {
            result.push(ln);
        }
        if !blank {
            relevant_at_level = indent == original_indent;
        }
        line += dir;
    }
    result
}

fn line_indentation(line: &[u8]) -> usize {
    line.iter().take_while(|&&b| b == b' ' || b == b'\t').count()
}

fn is_blank_line(line: &[u8]) -> bool {
    line.iter().all(|&b| b == b' ' || b == b'\t')
}

/// RuboCop's `all_relevant_assignment_lines` plus `align_column`/
/// `align_equal_sign`: every edit needed to realign the contiguous
/// same-indent assignment block `line` belongs to.
fn build_alignment_edits(
    ctx: &Context<'_>,
    assignment_map: &BTreeMap<u32, Span>,
    line: u32,
) -> Vec<Edit> {
    let mut lines = relevant_assignment_lines(ctx, assignment_map, line, -1);
    lines.extend(relevant_assignment_lines(ctx, assignment_map, line, 1));
    lines.sort_unstable();
    lines.dedup();

    let mut entries: Vec<(Span, u32, usize)> = Vec::with_capacity(lines.len());
    let mut align_to = 0usize;
    for ln in &lines {
        let op = assignment_map[ln];
        let prev_end = prev_visible_end(ctx, op.start);
        let prev_end_col = ctx.line_col(prev_end).column as usize;
        let op_len = (op.end - op.start) as usize;
        align_to = align_to.max(prev_end_col + op_len + 1);
        entries.push((op, prev_end, op_len));
    }

    let mut edits = Vec::new();
    for (op, prev_end, op_len) in entries {
        let prev_end_col = ctx.line_col(prev_end).column as usize;
        let target_spaces = align_to - prev_end_col - op_len;
        let current_spaces = (op.start - prev_end) as usize;
        if target_spaces != current_spaces {
            edits.push(Edit::replace(
                Span::new(prev_end, op.start),
                " ".repeat(target_spaces).into_bytes(),
            ));
        }
    }
    edits
}

/// The byte offset right after the last non-space character before `op_start`
/// on its own line -- RuboCop's `align_column`'s `leading =~ / *\Z/` trailing
/// space-run start.
fn prev_visible_end(ctx: &Context<'_>, op_start: u32) -> u32 {
    let line = ctx.line_col(op_start).line;
    let line_span = ctx.line_span(line);
    let line_text = ctx.line_text(line);
    let mut i = op_start;
    while i > line_span.start {
        let b = line_text[(i - 1 - line_span.start) as usize];
        if b != b' ' {
            break;
        }
        i -= 1;
    }
    i
}
