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
//!   whitespace; a run whose only preceding bytes on the line are more
//!   whitespace (e.g. a tab-then-spaces indent) is likewise never a
//!   candidate -- there is no real *token* before it either, just more of
//!   the same line's leading indentation.
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
//! - 1.91.0's `spacing_varies?` gate (an extra-space run whose following
//!   token is not itself an assignment/comparison operator must also show
//!   *varying* spacing across a block, not merely line up with something)
//!   needs its own light lexical tokenizer per physical line: [`LineTok`]/
//!   [`classify_line_token`] classify each token into a coarse [`TokKind`]
//!   (word/number/literal-with-delimiters/one operator byte) good enough
//!   to group "the same kind of thing" at a shared column, using
//!   [`ExtraSpacing::literal_spans`] (a literal's *whole* span, delimiters
//!   included, unlike the content-only `opaque`) so a token search landing
//!   on an opening quote -- which `run_end` always does, since the quote
//!   itself is ordinary non-opaque text -- still finds it.
//! - 1.91.0's `ForceEqualSignAlignment` block boundary
//!   (`interrupting_operator_lines`) is found the same lexical way:
//!   scanning for comparison operators and a bare `<<` append operator
//!   (told apart from a bare heredoc opener by requiring an
//!   expression-like byte immediately before it) outside every opaque
//!   range.
//!
//! See `META.blind_spots` for the behavioural gaps this approximation
//! knowingly accepts.

use std::collections::{BTreeMap, HashMap, HashSet};

use linter::{
    Applicability, CommentInfo, ConfigDefault, ConfigOption, Context, Department, Edit, Fix,
    FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{is_ruby_whitespace, Span};

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

/// A coarse lexical classification for [`LineTok`], used only to group
/// tokens by "does this look like the same kind of thing" the way RuboCop's
/// real token `type` does for `spacing_varies?`'s `typed[[column, type]]`
/// bucketing -- fine enough to keep a quoted string from ever being
/// confused with a `%w[]` word or an operator run, but not a faithful
/// reproduction of every real lexer token type (a documented blind spot).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TokKind {
    /// An identifier, keyword, constant, ivar, or gvar word run.
    Word,
    /// A run of decimal digits (and `_` separators).
    Number,
    /// The full span of a string/x-string/regexp/quoted-symbol literal
    /// (one of [`ExtraSpacing::opaque`]'s spans), the whole thing treated
    /// as one token.
    Literal,
    /// Anything else: a single operator/punctuation byte, or (for `.`/`..`/
    /// `...`/`::`/a greedy run of [`OP_CHARS`]) that run's first byte.
    Op(u8),
}

/// One lexical token on a single physical line, as approximated for
/// `spacing_varies?`'s grouping: a byte-offset-within-line `[start, end)`
/// plus its coarse [`TokKind`].
#[derive(Debug, Clone, Copy)]
struct LineTok {
    start: u32,
    end: u32,
    kind: TokKind,
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
    /// serving as an alignment reference for *other* tokens, and (per
    /// 1.91.0) from `spacing_varies?`'s own nearest-line search.
    standalone_comment_lines: &'a HashSet<u32>,
    /// The first `=`-ending operator per line, lexically scanned. RuboCop's
    /// `assignment_tokens`.
    assignment_map: &'a BTreeMap<u32, Span>,
    /// Every line's lexical tokens (see [`LineTok`]), keyed by line number
    /// -- RuboCop's `@tokens_by_line`. Empty when `AllowForAlignment` is
    /// off, since only `spacing_varies?` (itself only reachable through
    /// `AllowForAlignment`) ever consults it.
    tokens_by_line: &'a BTreeMap<u32, Vec<LineTok>>,
    /// Every `def`'s first and last line -- RuboCop's
    /// `definition_boundary_lines`, a hard stop for both `alignment_lines`'
    /// and `nearest_spacing_varies?`'s line search.
    def_boundary_lines: &'a HashSet<u32>,
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
    /// The *whole* span (delimiters included) of every node in [`Self::opaque`],
    /// used only to build [`Analysis::tokens_by_line`]'s one-token-per-literal
    /// entries -- unlike `opaque` itself, this must include a plain literal's
    /// opening/closing quote so a token search landing on the quote (as
    /// `run_end` always does, since a quote is ordinary non-opaque text)
    /// still finds it.
    literal_spans: Vec<Span>,
    /// `(key.end_pos, value.begin_pos)` for every pair of a *multiline*
    /// hash literal or bare keyword-argument hash -- RuboCop's
    /// `ignored_ranges`, deferring that spacing to `Layout/HashAlignment`.
    ignored_ranges: Vec<Span>,
    /// Byte offsets of `=` characters that are never assignment operators:
    /// an optional parameter's default-value operator, or an endless
    /// method definition's own `=`.
    excluded_equals: HashSet<u32>,
    /// Every `def`'s first and last line -- RuboCop's
    /// `definition_boundary_lines`.
    def_boundary_lines: HashSet<u32>,
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
        stability: Stability::Stable,
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

- `spacing_varies?`'s `(column, type)` grouping (1.91.0) relies on
  [`TokKind`], a coarse lexical classification (word/number/literal/one
  operator byte) rather than RuboCop's real, fine-grained lexer token
  types; two genuinely different real token types that happen to share a
  [`TokKind`] at the same column could be grouped together (or the
  reverse) when they wouldn't be upstream, which can tip `spacing_varies?`
  either way.
- `interrupting_operator_lines` (1.91.0, `ForceEqualSignAlignment`'s
  block-boundary detection) finds a bare `<<` append operator lexically,
  telling it apart from a bare heredoc opener (`<<HEREDOC`, no `~`/`-`) by
  requiring an expression-like byte immediately before it; a heredoc
  opener directly preceded by such a byte (unusual, but not impossible)
  would be misdetected as an interrupting append operator.
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
            literal_spans: Vec::new(),
            ignored_ranges: Vec::new(),
            excluded_equals: HashSet::new(),
            def_boundary_lines: HashSet::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.opaque.clear();
        self.literal_spans.clear();
        self.excluded_equals.clear();
        self.def_boundary_lines.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::StringNode => {
                if let Some(n) = node.as_string_node() {
                    self.opaque.push(n.content_loc().span());
                    self.literal_spans.push(node.span());
                }
            }
            NodeKind::XStringNode => {
                if let Some(n) = node.as_x_string_node() {
                    self.opaque.push(n.content_loc().span());
                    self.literal_spans.push(node.span());
                }
            }
            NodeKind::RegularExpressionNode => {
                if let Some(n) = node.as_regular_expression_node() {
                    self.opaque.push(n.content_loc().span());
                    self.literal_spans.push(node.span());
                }
            }
            NodeKind::SymbolNode => {
                if let Some(loc) = node.as_symbol_node().and_then(|n| n.value_loc()) {
                    self.opaque.push(loc.span());
                    self.literal_spans.push(node.span());
                }
            }
            NodeKind::OptionalParameterNode => {
                if let Some(n) = node.as_optional_parameter_node() {
                    self.excluded_equals.insert(n.operator_loc().span().start);
                }
            }
            NodeKind::DefNode => {
                if let Some(n) = node.as_def_node() {
                    if let Some(loc) = n.equal_loc() {
                        self.excluded_equals.insert(loc.span().start);
                    }
                    let span = node.span();
                    self.def_boundary_lines.insert(ctx.line_col(span.start).line);
                    self.def_boundary_lines.insert(ctx.line_col(span.end.saturating_sub(1)).line);
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
        let tokens_by_line = if self.allow_for_alignment {
            build_tokens_by_line(ctx, &self.literal_spans, &comments)
        } else {
            BTreeMap::new()
        };

        let analysis = Analysis {
            comments: &comments,
            comment_pair_aligned: &comment_pair_aligned,
            standalone_comment_lines: &standalone_comment_lines,
            assignment_map: &assignment_map,
            tokens_by_line: &tokens_by_line,
            def_boundary_lines: &self.def_boundary_lines,
        };
        self.scan_lines(ctx, &analysis);

        if self.force_equal_sign_alignment {
            let interrupting =
                scan_interrupting_operator_lines(ctx, &self.opaque, &comments, &assignment_map);
            Self::check_assignment_alignment(ctx, &assignment_map, &interrupting);
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
                let is_leading_indentation =
                    line_bytes[..run_start_local].iter().all(|&b| is_ruby_whitespace(b));
                if j - run_start_local >= 2
                    && run_start_local != 0
                    && j != len
                    && !is_leading_indentation
                {
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
            && (is_assignment_or_comparison_token(line_bytes, run_end, line_span, local_starts)
                || spacing_varies(ctx, a, run_start, run_end, line))
        {
            return;
        }
        report_unnecessary(ctx, run_start, run_end);
    }

    /// RuboCop's `check_assignment`/`align_equal_signs`: for every line
    /// whose first assignment operator is not aligned with the nearest
    /// preceding same-indent assignment line, reports it and attaches a fix
    /// realigning the whole contiguous block.
    fn check_assignment_alignment(
        ctx: &mut Context<'_>,
        assignment_map: &BTreeMap<u32, Span>,
        interrupting: &HashSet<u32>,
    ) {
        for (&line, &op_span) in assignment_map {
            let candidates = relevant_assignment_lines(ctx, assignment_map, interrupting, line, -1);
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
            let edits = build_alignment_edits(ctx, assignment_map, interrupting, line);
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
    comments.iter().filter(|c| ctx.begins_its_line(c.span)).map(|c| c.line).collect()
}

/// RuboCop's `aligned_with_something?`: does `pos` (the start of the token
/// right after an extra-spacing run) line up with something meaningful on
/// another line? Mirrors `aligned_with_adjacent_line?` exactly: each of the
/// "lines before" / "lines after" candidate lists is walked nearest-line
/// first, stopping at (and deciding by) the *first* line that isn't blank
/// and isn't a standalone comment -- it never keeps searching past that one
/// line even when the predicate itself says no. Only if neither direction's
/// nearest candidate matches does a second pass retry both directions, this
/// time additionally requiring the candidate's indentation column to equal
/// `line`'s own (RuboCop's `base_indentation` fallback).
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

    if aligned_with_line(ctx, (1..line).rev(), None, col, token_text, a)
        || aligned_with_line(ctx, (line + 1)..=ctx.line_count(), None, col, token_text, a)
    {
        return true;
    }

    let base_indentation = line_indentation(ctx, line);
    aligned_with_line(ctx, (1..line).rev(), Some(base_indentation), col, token_text, a)
        || aligned_with_line(
            ctx,
            (line + 1)..=ctx.line_count(),
            Some(base_indentation),
            col,
            token_text,
            a,
        )
}

/// RuboCop's `aligned_with_line?`: scans `line_nos` (already ordered nearest
/// candidate first) for the first line that is neither blank nor a
/// standalone comment. Once a required `indent` is given (the
/// `base_indentation` retry pass), a candidate less indented than `indent`
/// ends a whole enclosing block -- an alignment anchor beyond it would be
/// coincidental, so the search stops there entirely -- while a candidate
/// *more* indented is nested content of the current group and is merely
/// skipped over, per 1.91.0. Otherwise returns [`check_line_alignment`]'s
/// verdict on just that single line.
fn aligned_with_line(
    ctx: &Context<'_>,
    line_nos: impl Iterator<Item = u32>,
    indent: Option<usize>,
    col: usize,
    token_text: &[u8],
    a: &Analysis<'_>,
) -> bool {
    for candidate in line_nos {
        if a.standalone_comment_lines.contains(&candidate) {
            continue;
        }
        let line_bytes = ctx.line_text(candidate);
        if is_blank_line(line_bytes) {
            continue;
        }
        if let Some(want) = indent {
            let got = line_indentation(ctx, candidate);
            if got < want {
                break;
            }
            if got > want {
                continue;
            }
        }
        return check_line_alignment(ctx, candidate, col, token_text, a);
    }
    false
}

/// RuboCop's `aligned_token?`: `aligned_words?` (a space-then-non-space
/// column match, or an exact token-text match) or `aligned_equals_operator?`
/// (the candidate's own end column matches an adjacent `=`-ending token).
/// The caller ([`aligned_with_line`]) has already established `candidate`
/// is non-blank and not a standalone comment.
fn check_line_alignment(
    ctx: &Context<'_>,
    candidate: u32,
    col: usize,
    token_text: &[u8],
    a: &Analysis<'_>,
) -> bool {
    let line_bytes = ctx.line_text(candidate);
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
/// assignment lines in the same contiguous block (stopping at a dedent, a
/// blank line at the current indent level, or -- per 1.91.0 -- a line with
/// a comparison/append operator or a *different* assignment op that isn't
/// itself part of the aligned block, RuboCop's `interrupting_operator_lines`).
fn relevant_assignment_lines(
    ctx: &Context<'_>,
    assignment_map: &BTreeMap<u32, Span>,
    interrupting: &HashSet<u32>,
    start_line: u32,
    dir: i64,
) -> Vec<u32> {
    let mut result = Vec::new();
    let total = i64::from(ctx.line_count());
    let original_indent = line_indentation(ctx, start_line);
    let mut relevant_at_level = true;
    let mut line = i64::from(start_line);
    while line >= 1 && line <= total {
        let ln = u32::try_from(line).unwrap_or(u32::MAX);
        let text = ctx.line_text(ln);
        let indent = line_indentation(ctx, ln);
        let blank = is_blank_line(text);
        if (ln != start_line && interrupting.contains(&ln))
            || (indent < original_indent && !blank)
            || (relevant_at_level && blank)
        {
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

/// RuboCop's `ProcessedSource#line_indentation`: the character count of the
/// run of `\s` bytes at the start of `line`, expressed as a display column
/// (equal to the character count, since `\s` bytes are always width 1).
fn line_indentation(ctx: &Context<'_>, line: u32) -> usize {
    let text = ctx.line_text(line);
    let offset = text.iter().take_while(|&&b| is_ruby_whitespace(b)).count();
    let line_start = ctx.line_span(line).start;
    let column = ctx.display_column(line_start + u32::try_from(offset).unwrap_or(u32::MAX));
    usize::try_from(column).unwrap_or(usize::MAX)
}

/// `String#blank?`: every byte on the line is `\s`.
fn is_blank_line(line: &[u8]) -> bool {
    line.iter().all(|&b| is_ruby_whitespace(b))
}

/// RuboCop's `all_relevant_assignment_lines` plus `align_column`/
/// `align_equal_sign`: every edit needed to realign the contiguous
/// same-indent assignment block `line` belongs to.
fn build_alignment_edits(
    ctx: &Context<'_>,
    assignment_map: &BTreeMap<u32, Span>,
    interrupting: &HashSet<u32>,
    line: u32,
) -> Vec<Edit> {
    let mut lines = relevant_assignment_lines(ctx, assignment_map, interrupting, line, -1);
    lines.extend(relevant_assignment_lines(ctx, assignment_map, interrupting, line, 1));
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

/// RuboCop's `ASSIGNMENT_OR_COMPARISON_TOKENS.include?(token.type)`: the
/// lexeme starting at `run_end` is one of `=`, `==`, `===`, `!=`, `<=`,
/// `>=`, `<<`, or a compound assignment (`+=`, `-=`, ..., `**=`, `<<=`,
/// `>>=`, `||=`, `&&=`) -- every operator [`token_extent`]'s greedy
/// operator-alphabet run can produce that either ends with `=` or is
/// exactly `<<`; nothing in `OP_CHARS` ending in `=` falls outside that
/// list (`=>` ends in `>`, `=~` ends in `~`, `<=>` ends in `>`, so none of
/// those are mistaken for one).
fn is_assignment_or_comparison_token(
    line_bytes: &[u8],
    run_end: u32,
    line_span: Span,
    local_starts: &[usize],
) -> bool {
    let col = (run_end - line_span.start) as usize;
    let end = token_extent(line_bytes, col, local_starts);
    let text = &line_bytes[col..end];
    text == b"<<" || text.last() == Some(&b'=')
}

/// One group's worth of RuboCop's `build_group_profile`: every token's
/// start column paired with its spacing from the previous token on the
/// same line, gathered two ways -- by column alone (`by_column`, RuboCop's
/// namesake) and by `(column, type)` (`typed`, keyed exactly like
/// RuboCop's `typed[[column, token.type]]`).
struct GroupProfile {
    by_column: HashMap<u32, Vec<(u32, i64)>>,
    typed: HashMap<(u32, TokKind), (Vec<u32>, Vec<i64>)>,
}

/// RuboCop's `build_group_profile`, run over every line in `group`.
fn build_group_profile(
    tokens_by_line: &BTreeMap<u32, Vec<LineTok>>,
    group: &[u32],
) -> GroupProfile {
    let mut by_column: HashMap<u32, Vec<(u32, i64)>> = HashMap::new();
    let mut typed: HashMap<(u32, TokKind), (Vec<u32>, Vec<i64>)> = HashMap::new();
    for &line in group {
        let Some(toks) = tokens_by_line.get(&line) else { continue };
        for pair in toks.windows(2) {
            let (prev, tok) = (pair[0], pair[1]);
            let spacing = i64::from(tok.start) - i64::from(prev.end);
            by_column.entry(tok.start).or_default().push((line, spacing));
            let entry =
                typed.entry((tok.start, tok.kind)).or_insert_with(|| (Vec::new(), Vec::new()));
            entry.0.push(line);
            entry.1.push(spacing);
        }
    }
    GroupProfile { by_column, typed }
}

/// RuboCop's `alignment_lines`/`alignment_line_ranges`: every code line
/// (excluding standalone-comment lines) in `line`'s contiguous same-indent
/// block, walking both up to line 1 and down to the file's last line,
/// each direction stopped by a `def`/`defs` boundary, a dedent, or a blank
/// line at the current indent level -- exactly [`alignment_lines`]'s own
/// stopping rules, generalized to a caller-supplied predicate and used
/// here to build a `spacing_varies?` `group_profile`.
fn alignment_lines(ctx: &Context<'_>, a: &Analysis<'_>, line: u32) -> Vec<u32> {
    let last = ctx.line_count();
    let mut lines = relevant_alignment_lines(ctx, a, (1..=line).rev(), line);
    lines.extend(relevant_alignment_lines(ctx, a, line..=last, line));
    lines.sort_unstable();
    lines.dedup();
    lines
}

/// RuboCop's `relevant_lines`, specialized to `alignment_line_ranges`'
/// own boundary set (`definition_boundary_lines`) and predicate (not a
/// standalone comment line).
fn relevant_alignment_lines(
    ctx: &Context<'_>,
    a: &Analysis<'_>,
    line_range: impl Iterator<Item = u32>,
    original_line: u32,
) -> Vec<u32> {
    let mut result = Vec::new();
    let original_indent = line_indentation(ctx, original_line);
    let mut relevant_at_level = true;
    for line_number in line_range {
        let indent = line_indentation(ctx, line_number);
        let blank = is_blank_line(ctx.line_text(line_number));
        if (line_number != original_line && a.def_boundary_lines.contains(&line_number))
            || (indent < original_indent && !blank)
            || (relevant_at_level && blank)
        {
            break;
        }
        if !a.standalone_comment_lines.contains(&line_number) && indent == original_indent {
            result.push(line_number);
        }
        if !blank {
            relevant_at_level = indent == original_indent;
        }
    }
    result
}

/// RuboCop's `spacing_varies?`: whether the gap right before the token
/// starting at `run_end` (whose previous real token ends at `run_start`)
/// looks like deliberate tabular alignment rather than an accidental
/// uniform run, per the `(column, type)`/`by_column` grouping built from
/// `line`'s `alignment_lines` block.
fn spacing_varies(
    ctx: &Context<'_>,
    a: &Analysis<'_>,
    run_start: u32,
    run_end: u32,
    line: u32,
) -> bool {
    let line_span = ctx.line_span(line);
    let col = u32_of((run_end - line_span.start) as usize);
    let Some(toks) = a.tokens_by_line.get(&line) else { return false };
    let Some(tok) = toks.iter().find(|t| t.start == col) else { return false };

    let group = alignment_lines(ctx, a, line);
    let profile = build_group_profile(a.tokens_by_line, &group);
    let spacing = i64::from(run_end) - i64::from(run_start);

    if let Some((lines, spacings)) = profile.typed.get(&(col, tok.kind)) {
        if lines.len() > 1 {
            return uniform_alignment_allowed(&profile, col, lines, spacings);
        }
    }
    if let Some(entries) = profile.by_column.get(&col) {
        if entries.len() > 1 {
            let distinct: HashSet<i64> = entries.iter().map(|&(_, s)| s).collect();
            return distinct.len() > 1;
        }
    }
    nearest_spacing_varies(a, line, col, spacing, ctx.line_count())
}

/// RuboCop's `uniform_alignment_allowed?`: the same `(column, type)` gap
/// takes more than one width across `lines` (evidence of hand-padding for
/// differing label lengths), or some *other* shared column shows the same
/// evidence for those exact lines.
fn uniform_alignment_allowed(
    profile: &GroupProfile,
    col: u32,
    lines: &[u32],
    spacings: &[i64],
) -> bool {
    let uniq: HashSet<i64> = spacings.iter().copied().collect();
    uniq.len() > 1 || aligned_on_other_column(profile, col, lines)
}

/// RuboCop's `aligned_on_other_column?`.
fn aligned_on_other_column(profile: &GroupProfile, col: u32, lines: &[u32]) -> bool {
    let line_set: HashSet<u32> = lines.iter().copied().collect();
    profile.by_column.iter().any(|(&c, entries)| {
        if c == col {
            return false;
        }
        let shared: HashSet<i64> =
            entries.iter().filter(|(l, _)| line_set.contains(l)).map(|&(_, s)| s).collect();
        shared.len() > 1
    })
}

/// RuboCop's `nearest_spacing_varies?`: walking away from `line` in both
/// directions (each stopped hard by a `def`/`defs` boundary), does the
/// nearest line with *any* token starting at `col` (skipping standalone
/// comment lines entirely) have a different gap width than `spacing`?
fn nearest_spacing_varies(
    a: &Analysis<'_>,
    line: u32,
    col: u32,
    spacing: i64,
    last_line: u32,
) -> bool {
    nearest_spacing_differs(a, (1..line).rev(), col, spacing)
        || nearest_spacing_differs(a, (line + 1)..=last_line, col, spacing)
}

/// RuboCop's `nearest_spacing_differs?`.
fn nearest_spacing_differs(
    a: &Analysis<'_>,
    line_numbers: impl Iterator<Item = u32>,
    col: u32,
    spacing: i64,
) -> bool {
    for line in line_numbers {
        if a.def_boundary_lines.contains(&line) {
            break;
        }
        if a.standalone_comment_lines.contains(&line) {
            continue;
        }
        if let Some(other) = spacing_before_token_at(a.tokens_by_line, line, col) {
            return other != spacing;
        }
    }
    false
}

/// RuboCop's `spacing_before_token_at`.
fn spacing_before_token_at(
    tokens_by_line: &BTreeMap<u32, Vec<LineTok>>,
    line: u32,
    col: u32,
) -> Option<i64> {
    let toks = tokens_by_line.get(&line)?;
    let idx = toks.iter().position(|t| t.start == col)?;
    if idx == 0 {
        return None;
    }
    Some(i64::from(toks[idx].start) - i64::from(toks[idx - 1].end))
}

/// RuboCop's `prepare_alignment_data`'s `@tokens_by_line`: every physical
/// line's lexical tokens (see [`LineTok`]/[`classify_line_token`]), built
/// once per file for `spacing_varies?`.
fn build_tokens_by_line(
    ctx: &Context<'_>,
    opaque: &[Span],
    comments: &[CommentInfo],
) -> BTreeMap<u32, Vec<LineTok>> {
    let mut spans: Vec<(Span, TokKind)> = opaque.iter().map(|&s| (s, TokKind::Literal)).collect();
    spans.extend(comments.iter().map(|c| (c.span, TokKind::Op(b'#'))));
    spans.sort_by_key(|(s, _)| s.start);

    let mut map = BTreeMap::new();
    for line in 1..=ctx.line_count() {
        let line_span = ctx.line_span(line);
        let line_bytes = ctx.line_text(line);
        let len = line_bytes.len();
        if len == 0 {
            continue;
        }
        let spans_local: Vec<(usize, usize, TokKind)> = spans
            .iter()
            .filter(|(s, _)| s.start < line_span.end && line_span.start < s.end)
            .map(|(s, kind)| {
                let start = (s.start.max(line_span.start) - line_span.start) as usize;
                let end = (s.end.min(line_span.end) - line_span.start) as usize;
                (start, end, *kind)
            })
            .collect();

        let mut toks = Vec::new();
        let mut i = 0usize;
        while i < len {
            if line_bytes[i] == b' ' {
                i += 1;
                continue;
            }
            let (end, kind) = classify_line_token(line_bytes, i, &spans_local);
            toks.push(LineTok { start: u32_of(i), end: u32_of(end), kind });
            i = end;
        }
        if !toks.is_empty() {
            map.insert(line, toks);
        }
    }
    map
}

/// The per-line-token classifier [`build_tokens_by_line`] uses: any
/// position that is the start of an already-known opaque/comment span
/// (`spans_local`) becomes one [`TokKind::Literal`]/`Op('#')` token
/// spanning it; otherwise this is the same word/number/`.`/`::`/
/// operator-alphabet-run classification [`token_extent`] uses, tagged
/// with a [`TokKind`] and stopped early by any other span's start the
/// same way `token_extent` is stopped by `local_opaque_starts`.
fn classify_line_token(
    line_bytes: &[u8],
    start: usize,
    spans_local: &[(usize, usize, TokKind)],
) -> (usize, TokKind) {
    fn is_word(b: u8) -> bool {
        b.is_ascii_alphanumeric() || b == b'_'
    }
    let stops_at = |i: usize| spans_local.iter().any(|&(s, _, _)| s == i);

    if let Some(&(_, end, kind)) = spans_local.iter().find(|&&(s, _, _)| s == start) {
        return (end, kind);
    }
    let c = line_bytes[start];
    if c == b'@' || c == b'$' {
        let mut end = start + 1;
        if c == b'@' && line_bytes.get(end) == Some(&b'@') {
            end += 1;
        }
        while end < line_bytes.len() && is_word(line_bytes[end]) && !stops_at(end) {
            end += 1;
        }
        return (end, TokKind::Word);
    }
    if c.is_ascii_digit() {
        let mut end = start;
        while end < line_bytes.len()
            && (line_bytes[end].is_ascii_digit() || line_bytes[end] == b'_')
            && !stops_at(end)
        {
            end += 1;
        }
        return (end, TokKind::Number);
    }
    if is_word(c) {
        let mut end = start;
        while end < line_bytes.len()
            && (is_word(line_bytes[end]) || line_bytes[end] == b'?' || line_bytes[end] == b'!')
            && !stops_at(end)
        {
            end += 1;
        }
        return (end, TokKind::Word);
    }
    if c == b'.' {
        let mut end = start + 1;
        if line_bytes.get(end) == Some(&b'.') {
            end += 1;
            if line_bytes.get(end) == Some(&b'.') {
                end += 1;
            }
        }
        return (end, TokKind::Op(b'.'));
    }
    if c == b':' && line_bytes.get(start + 1) == Some(&b':') {
        return (start + 2, TokKind::Op(b':'));
    }
    if OP_CHARS.contains(&c) {
        let mut end = start;
        while end < line_bytes.len() && OP_CHARS.contains(&line_bytes[end]) && !stops_at(end) {
            end += 1;
        }
        return (end, TokKind::Op(c));
    }
    (start + 1, TokKind::Op(c))
}

/// RuboCop's `interrupting_operator_lines`: every line holding a
/// comparison operator (`==`, `===`, `!=`, `<=`, `>=`) or an append
/// operator (`<<`, heuristically told apart from a bare heredoc opener by
/// requiring an expression-like byte -- a closing bracket/quote or a
/// word character -- immediately before it) that isn't already one of
/// `assignment_map`'s own assignment lines.
fn scan_interrupting_operator_lines(
    ctx: &Context<'_>,
    opaque: &[Span],
    comments: &[CommentInfo],
    assignment_map: &BTreeMap<u32, Span>,
) -> HashSet<u32> {
    let bytes = ctx.source().bytes();
    let mut skip: Vec<Span> = opaque.to_vec();
    skip.extend(comments.iter().map(|c| c.span));
    skip.sort_by_key(|s| s.start);

    let mut lines = HashSet::new();
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
        let b = bytes[i];
        let matched = match b {
            b'=' if bytes.get(i + 1) == Some(&b'=') && bytes.get(i + 2) == Some(&b'=') => Some(3),
            b'=' | b'!' | b'<' | b'>' if bytes.get(i + 1) == Some(&b'=') => Some(2),
            b'<' if bytes.get(i + 1) == Some(&b'<') => {
                let mut j = i;
                while j > 0 && bytes[j - 1] == b' ' {
                    j -= 1;
                }
                let prev = if j > 0 { Some(bytes[j - 1]) } else { None };
                let looks_like_value = matches!(prev, Some(b')' | b']' | b'}' | b'"' | b'\''))
                    || prev.is_some_and(|p| p.is_ascii_alphanumeric() || p == b'_');
                if looks_like_value {
                    Some(2)
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(n) = matched {
            lines.insert(ctx.line_col(u32_of(i)).line);
            i += n;
        } else {
            i += 1;
        }
    }
    for line in assignment_map.keys() {
        lines.remove(line);
    }
    lines
}
