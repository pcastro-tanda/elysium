//! `Style/Semicolon`, ported from RuboCop's
//! `lib/rubocop/cop/style/semicolon.rb`.
//!
//! Upstream drives this cop off a real token stream (`processed_source.tokens`),
//! which elysium has no equivalent of. This port instead works from
//! [`ruby_ast::ext::is_heredoc`]'s opaque-span machinery plus a handful of
//! AST facts gathered once per file (`enter`), and does the actual
//! byte-level classification in `file_end`:
//!
//! - **Multi-expression lines** (upstream's `on_begin`): every
//!   [`NodeKind::StatementsNode`] with more than one child is grouped by
//!   [`Context::last_line`]; a line more than one child ends on gets *every*
//!   non-opaque `;` on it reported, replaced with a newline (unless a
//!   heredoc opened earlier on the same line -- then the offense is
//!   reported with no fix, matching `heredoc_opened_before_semicolon?`).
//!   Skipped entirely when `AllowAsExpressionSeparator` is set.
//! - **Line terminator/opener** (upstream's `check_for_line_terminator_or_opener`,
//!   via `each_semicolon`/`semicolon_position`): for each physical line,
//!   at most one `;` is classified, in priority order: the last non-opaque
//!   token of the line (`tokens.last.semicolon?`), the first
//!   (`tokens.first.semicolon?`), immediately before a block-closing `}`
//!   (`exist_semicolon_before_right_curly_brace?`), immediately after a
//!   block/lambda-opening `{` or a string interpolation's opening `#{`
//!   (`exist_semicolon_after_left_curly_brace?`/
//!   `exist_semicolon_after_left_lambda_curly_brace?`/
//!   `exist_semicolon_after_left_string_interpolation_brace?`), or
//!   immediately before an interpolation's closing `}`
//!   (`exist_semicolon_before_right_string_interpolation_brace?`). Always
//!   runs, regardless of `AllowAsExpressionSeparator`. Only the first
//!   ("last token") case can wrap a node in parens (see below); every other
//!   case is a plain removal of the `;` alone.
//!
//! A given physical line yields at most one "line terminator/opener"
//! offense; if a multi-expression line's chosen `;` there is the same one
//! "line terminator/opener" already claimed, it is not reported twice --
//! matching RuboCop's own offense deduplication by range (`on_new_investigation`
//! runs before node traversal, so the "opener" callback's offense always
//! wins the race for that exact position).
//!
//! The "last token of the line" case additionally wraps a node in parens
//! when the token right before the `;` is:
//! - the `..`/`...` of an endless range (`RangeNode` with no `right`): wraps
//!   the whole range (`42..;` -> `(42..)`).
//! - a value-omitted keyword argument's label (`KeywordHashNode` whose last
//!   `AssocNode` ends its own source in `:`) belonging to a parenthesis-less
//!   call: wraps the whole keyword hash and closes the gap between the
//!   call's message and it (`m key:;` -> `m(key:)`).

use std::collections::{HashMap, HashSet};

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{ext::is_heredoc, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Do not use semicolons to terminate expressions.";

/// A node to wrap in parens when correcting a "last token of the line"
/// semicolon, plus (for the keyword-argument case) the call message's end
/// offset so the gap before the wrapped node can be closed too.
enum Wrap {
    /// An endless range: wrap `span` in `(` `)`.
    Range(Span),
    /// A parenthesis-less call's trailing value-omitted keyword hash: wrap
    /// `span` in `(` `)` and replace the gap between `message_end` and
    /// `span.start` with nothing (closing `m key:` to `m(key:`).
    Kwargs {
        span: Span,
        message_end: u32,
    },
    None,
}

/// How a "line terminator/opener" semicolon was classified. Only
/// `Trailing` (upstream's `tokens.last.semicolon?`, `after_expression:
/// false` but reached via `semicolon_pos == -1`) can additionally wrap a
/// node; RuboCop's `token_before_semicolon` is the semicolon token itself
/// for every other case, so `regexp_dots?`/`type == :tLABEL` can never
/// match there.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OpenerKind {
    Trailing,
    Other,
}

/// Checks for multiple expressions placed on the same line. It also checks
/// for lines terminated with a semicolon.
#[derive(Debug, Clone)]
pub struct Semicolon {
    allow_as_expression_separator: bool,
    /// Lines with more than one statement, from a same [`NodeKind::StatementsNode`],
    /// ending on it.
    multi_expr_lines: HashSet<u32>,
    /// `(first_line, opening_delimiter_end)` for every heredoc found.
    heredoc_opens: Vec<(u32, u32)>,
    /// Byte spans of the exact `{` character of every block/lambda that
    /// opens with a brace (as opposed to `do`).
    block_open: Vec<Span>,
    /// Byte spans of the exact `}` character of every such block/lambda.
    block_close: Vec<Span>,
    /// Byte spans of every string interpolation's opening `#{`.
    interp_open: Vec<Span>,
    /// Byte spans of every string interpolation's closing `}`.
    interp_close: Vec<Span>,
    /// Full span of every endless range (`RangeNode` with no `right`).
    endless_ranges: Vec<Span>,
    /// `(keyword_hash_span, call_message_end)` for every parenthesis-less
    /// call whose trailing keyword hash's last pair omits its value.
    omitted_kwargs: Vec<(Span, u32)>,
}

impl Rule for Semicolon {
    const META: RuleMeta = RuleMeta {
        name: "Style/Semicolon",
        department: Department::Style,
        summary: "Don't use semicolons to terminate expressions.",
        explanation: "\
Checks for multiple expressions placed on the same line. It also checks for
lines terminated with a semicolon. In idiomatic Ruby, each expression should
be on its own line for readability.

This cop has `AllowAsExpressionSeparator` configuration option. It allows
`;` to separate several expressions on the same line.

```ruby
# bad
foo = 1; bar = 2;
baz = 3;

# good
foo = 1
bar = 2
baz = 3
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::StatementsNode,
            NodeKind::CallNode,
            NodeKind::RangeNode,
            NodeKind::BlockNode,
            NodeKind::LambdaNode,
            NodeKind::EmbeddedStatementsNode,
            NodeKind::StringNode,
            NodeKind::InterpolatedStringNode,
            NodeKind::XStringNode,
            NodeKind::InterpolatedXStringNode,
        ],
        config: &[ConfigOption {
            name: "AllowAsExpressionSeparator",
            default: ConfigDefault::Bool(false),
            allowed: &[],
            doc: "Allows `;` to separate several expressions on the same line.",
        }],
        blind_spots: "\
Raw-byte scanning rather than a real token stream: a `;` that is the first
or last *visible* non-whitespace text of its physical line but is actually a
continuation of a token that started on an earlier physical line (a
multi-line, non-heredoc string or a backslash-continued line, e.g. the `;`
right after a multi-line string literal's closing quote) is classified as if
it began its own token there. Not exercised by any known real-world Ruby
style.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            allow_as_expression_separator: options.bool("AllowAsExpressionSeparator"),
            multi_expr_lines: HashSet::new(),
            heredoc_opens: Vec::new(),
            block_open: Vec::new(),
            block_close: Vec::new(),
            interp_open: Vec::new(),
            interp_close: Vec::new(),
            endless_ranges: Vec::new(),
            omitted_kwargs: Vec::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::StatementsNode => collect_statements(self, node, ctx),
            NodeKind::CallNode => collect_call(self, node),
            NodeKind::RangeNode => collect_range(self, node),
            NodeKind::BlockNode => collect_block(self, node),
            NodeKind::LambdaNode => collect_lambda(self, node),
            NodeKind::EmbeddedStatementsNode => collect_interp(self, node),
            NodeKind::StringNode
            | NodeKind::InterpolatedStringNode
            | NodeKind::XStringNode
            | NodeKind::InterpolatedXStringNode => collect_heredoc(self, node, ctx),
            _ => {}
        }
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let opaque: Vec<Span> = ctx.opaque_spans().to_vec();
        let bytes = ctx.source().bytes();

        let mut by_line: Vec<(u32, Vec<u32>)> = Vec::new();
        for (i, &b) in bytes.iter().enumerate() {
            if b != b';' {
                continue;
            }
            let pos = u32::try_from(i).unwrap_or(u32::MAX);
            if opaque_end_covering(&opaque, pos).is_some() {
                continue;
            }
            let line = ctx.line_col(pos).line;
            match by_line.last_mut() {
                Some((last_line, semis)) if *last_line == line => semis.push(pos),
                _ => by_line.push((line, vec![pos])),
            }
        }
        if by_line.is_empty() {
            return;
        }

        // "Line terminator/opener": at most one classified `;` per line,
        // always checked regardless of `AllowAsExpressionSeparator`.
        let mut claimed: HashMap<u32, u32> = HashMap::new();
        for (line, semis) in &by_line {
            let line_span = ctx.line_span(*line);
            let Some((pos, kind)) = classify_opener(bytes, &opaque, line_span, semis, self) else {
                continue;
            };
            claimed.insert(*line, pos);
            let semi_span = Span::new(pos, pos + 1);
            let wrap = if kind == OpenerKind::Trailing {
                find_wrap(bytes, &opaque, pos, &self.endless_ranges, &self.omitted_kwargs)
            } else {
                Wrap::None
            };
            let fix = match wrap {
                Wrap::Range(span) => Fix {
                    applicability: Applicability::Safe,
                    edits: vec![
                        Edit::insert(span.start, b"(".to_vec()),
                        Edit::insert(span.end, b")".to_vec()),
                        Edit::delete(semi_span),
                    ],
                },
                Wrap::Kwargs { span, message_end } => Fix {
                    applicability: Applicability::Safe,
                    edits: vec![
                        Edit::replace(Span::new(message_end, span.start), b"(".to_vec()),
                        Edit::insert(span.end, b")".to_vec()),
                        Edit::delete(semi_span),
                    ],
                },
                Wrap::None => {
                    Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(semi_span)] }
                }
            };
            ctx.report_with_fix(&Self::META, semi_span, MSG, fix);
        }

        // "Multi-expression line": every non-opaque `;` on a line where a
        // `StatementsNode` has more than one child ending on it, skipping
        // whichever one the opener check above already claimed.
        if self.allow_as_expression_separator {
            return;
        }
        for (line, semis) in &by_line {
            if !self.multi_expr_lines.contains(line) {
                continue;
            }
            for &pos in semis {
                if claimed.get(line) == Some(&pos) {
                    continue;
                }
                let semi_span = Span::new(pos, pos + 1);
                let opened_before =
                    self.heredoc_opens.iter().any(|&(hl, end)| hl == *line && end <= pos);
                if opened_before {
                    ctx.report(&Self::META, semi_span, MSG);
                } else {
                    let fix = Fix {
                        applicability: Applicability::Safe,
                        edits: vec![Edit::replace(semi_span, b"\n".to_vec())],
                    };
                    ctx.report_with_fix(&Self::META, semi_span, MSG, fix);
                }
            }
        }
    }
}

/// RuboCop's `on_begin`: records every line more than one child of a
/// multi-statement body ends on, via [`Context::last_line`].
/// `AllowAsExpressionSeparator` is applied later, at `file_end`.
fn collect_statements(rule: &mut Semicolon, node: &Node<'_>, ctx: &Context<'_>) {
    let Some(stmts) = node.as_statements_node() else { return };
    let body = stmts.body();
    if body.len() < 2 {
        return;
    }
    let mut counts: HashMap<u32, u32> = HashMap::new();
    for child in &body {
        *counts.entry(ctx.last_line(child.span())).or_insert(0) += 1;
    }
    for (line, count) in counts {
        if count > 1 {
            rule.multi_expr_lines.insert(line);
        }
    }
}

/// A parenthesis-less call whose trailing keyword hash's last pair omits
/// its value (`value_omission_pair_nodes`/`node.parent`): recorded so the
/// "last token of the line" case can wrap it.
fn collect_call(rule: &mut Semicolon, node: &Node<'_>) {
    let Some(call) = node.as_call_node() else { return };
    let Some(args) = call.arguments() else { return };
    let Some(last_arg) = args.arguments().last() else { return };
    let Some(kw) = last_arg.as_keyword_hash_node() else { return };
    let elements = kw.elements();
    let Some(last_pair) = elements.last() else { return };
    let Some(assoc) = last_pair.as_assoc_node() else { return };
    if !assoc.location().as_slice().ends_with(b":") {
        return;
    }
    let Some(message) = call.message_loc() else { return };
    rule.omitted_kwargs.push((kw.location().span(), message.span().end));
}

/// An endless range (`RangeNode` with no `right`): recorded so the "last
/// token of the line" case can wrap it.
fn collect_range(rule: &mut Semicolon, node: &Node<'_>) {
    let Some(range) = node.as_range_node() else { return };
    if range.right().is_none() {
        rule.endless_ranges.push(node.span());
    }
}

fn collect_block(rule: &mut Semicolon, node: &Node<'_>) {
    let Some(block) = node.as_block_node() else { return };
    let open = block.opening_loc();
    if open.as_slice() == b"{" {
        rule.block_open.push(open.span());
    }
    let close = block.closing_loc();
    if close.as_slice() == b"}" {
        rule.block_close.push(close.span());
    }
}

fn collect_lambda(rule: &mut Semicolon, node: &Node<'_>) {
    let Some(lambda) = node.as_lambda_node() else { return };
    let open = lambda.opening_loc();
    if open.as_slice() == b"{" {
        rule.block_open.push(open.span());
    }
    let close = lambda.closing_loc();
    if close.as_slice() == b"}" {
        rule.block_close.push(close.span());
    }
}

fn collect_interp(rule: &mut Semicolon, node: &Node<'_>) {
    let Some(e) = node.as_embedded_statements_node() else { return };
    rule.interp_open.push(e.opening_loc().span());
    rule.interp_close.push(e.closing_loc().span());
}

/// A heredoc's opening delimiter (`<<~TEXT`), whichever of the four
/// string-literal kinds it is: RuboCop's `heredoc_opened_before_semicolon?`
/// compares a heredoc's (delimiter-only) `source_range` against the
/// semicolon's, so only the opening location -- never the body -- matters.
fn collect_heredoc(rule: &mut Semicolon, node: &Node<'_>, ctx: &Context<'_>) {
    if !is_heredoc(node) {
        return;
    }
    let opening = match node.kind() {
        NodeKind::StringNode => node.as_string_node().and_then(|n| n.opening_loc()),
        NodeKind::InterpolatedStringNode => {
            node.as_interpolated_string_node().and_then(|n| n.opening_loc())
        }
        NodeKind::XStringNode => node.as_x_string_node().map(|n| n.opening_loc()),
        NodeKind::InterpolatedXStringNode => {
            node.as_interpolated_x_string_node().map(|n| n.opening_loc())
        }
        _ => None,
    };
    if let Some(loc) = opening {
        let span = loc.span();
        rule.heredoc_opens.push((ctx.line_col(span.start).line, span.end));
    }
}

/// The end of the opaque span (string/regexp body or comment) covering
/// `pos`, if any. See [`Context::opaque_spans`].
fn opaque_end_covering(opaque: &[Span], pos: u32) -> Option<u32> {
    let idx = opaque.partition_point(|s| s.start <= pos);
    (idx > 0 && pos < opaque[idx - 1].end).then(|| opaque[idx - 1].end)
}

/// True when every byte in `[start, end)` is a plain space/tab or falls
/// inside an opaque span (a trailing comment); a real newline or any other
/// non-blank byte fails it. Used to test that two positions are "adjacent"
/// modulo insignificant whitespace, on the same physical line.
fn only_insignificant(bytes: &[u8], opaque: &[Span], mut pos: u32, end: u32) -> bool {
    while pos < end {
        if let Some(skip_to) = opaque_end_covering(opaque, pos) {
            pos = skip_to;
            continue;
        }
        match bytes[pos as usize] {
            b' ' | b'\t' => pos += 1,
            _ => return false,
        }
    }
    true
}

/// RuboCop's `check_for_line_terminator_or_opener`/`semicolon_position`:
/// classifies at most one `;` on this physical line, in priority order.
fn classify_opener(
    bytes: &[u8],
    opaque: &[Span],
    line_span: Span,
    semis: &[u32],
    rule: &Semicolon,
) -> Option<(u32, OpenerKind)> {
    let (block_open, block_close) = (&rule.block_open, &rule.block_close);
    let (interp_open, interp_close) = (&rule.interp_open, &rule.interp_close);
    // `tokens.last.semicolon?`.
    if let Some(&last) = semis.last() {
        if only_insignificant(bytes, opaque, last + 1, line_span.end) {
            return Some((last, OpenerKind::Trailing));
        }
    }
    // `tokens.first.semicolon?`.
    if let Some(&first) = semis.first() {
        if only_insignificant(bytes, opaque, line_span.start, first) {
            return Some((first, OpenerKind::Other));
        }
    }
    // `exist_semicolon_before_right_curly_brace?`.
    for &pos in semis {
        for close in block_close {
            if close.start > pos
                && only_insignificant(bytes, opaque, pos + 1, close.start)
                && only_insignificant(bytes, opaque, close.end, line_span.end)
            {
                return Some((pos, OpenerKind::Other));
            }
        }
    }
    // `exist_semicolon_after_left_curly_brace?`/
    // `exist_semicolon_after_left_lambda_curly_brace?`/
    // `exist_semicolon_after_left_string_interpolation_brace?`.
    for &pos in semis {
        for open in block_open.iter().chain(interp_open.iter()) {
            if open.end <= pos
                && only_insignificant(bytes, opaque, open.end, pos)
                && !only_insignificant(bytes, opaque, line_span.start, open.start)
            {
                return Some((pos, OpenerKind::Other));
            }
        }
    }
    // `exist_semicolon_before_right_string_interpolation_brace?`.
    for &pos in semis {
        for close in interp_close {
            if close.start > pos && only_insignificant(bytes, opaque, pos + 1, close.start) {
                return Some((pos, OpenerKind::Other));
            }
        }
    }
    None
}

/// The node to wrap for a "last token of the line" semicolon at `pos`,
/// matching upstream's `token_before_semicolon.regexp_dots?`/`type ==
/// :tLABEL` (only reachable when `semicolon_pos == -1`, i.e. exactly this
/// case).
fn find_wrap(
    bytes: &[u8],
    opaque: &[Span],
    pos: u32,
    endless_ranges: &[Span],
    omitted_kwargs: &[(Span, u32)],
) -> Wrap {
    for &span in endless_ranges {
        if span.end <= pos && only_insignificant(bytes, opaque, span.end, pos) {
            return Wrap::Range(span);
        }
    }
    for &(span, message_end) in omitted_kwargs {
        if span.end <= pos && only_insignificant(bytes, opaque, span.end, pos) {
            return Wrap::Kwargs { span, message_end };
        }
    }
    Wrap::None
}
