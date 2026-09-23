//! `Layout/SpaceInsideBlockBraces`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_inside_block_braces.rb` plus the
//! `SurroundingSpace`/`RangeHelp` mixins it includes.
//!
//! Prism gives every brace-delimited block (`{ }` with explicit `|params|`,
//! numbered `_1`/implicit `it` parameters, or none at all) the same
//! [`NodeKind::BlockNode`], and a stabby lambda (`->(x) { x }`) its own
//! [`NodeKind::LambdaNode`] -- RuboCop's `on_block`/`on_numblock`/`on_itblock`
//! all collapse onto that one node kind here, and lambdas (parsed as a
//! `:block` node wrapping a `(send nil :lambda)` in whitequark) fall out of
//! `on_block` there too. `do`/`end` blocks are skipped exactly like
//! RuboCop's `return if node.keywords?`, by checking the opening delimiter's
//! source text is `{`.
//!
//! The "delimiter right after `{`" RuboCop reads off `node.arguments.loc.begin`
//! is, in Prism, [`BlockParametersNode::opening_loc`] -- the same node shape
//! backs both `|x|` block params and a lambda's `(x)` params, so checking
//! that opening delimiter's text is exactly `|` (not `(`) reproduces
//! RuboCop's `pipe?` check for free; numbered/it-parameter blocks carry a
//! [`NodeKind::NumberedParametersNode`]/[`NodeKind::ItParametersNode`]
//! instead, which never match, giving `args_delimiter = nil` just like
//! RuboCop's `numblock`/`itblock` handling.
//!
//! `node.source_range.column` (used to decide whether a multiline closing
//! brace is "aligned") is the column of the whole `recv.method(args) { ... }`
//! construct, which in Prism is the *parent* [`NodeKind::CallNode`] (or
//! `super`) enclosing a [`NodeKind::BlockNode`] -- confirmed empirically: a
//! `CallNode`'s own span already extends through its block's closing brace,
//! so [`Context::parent`] gives the right starting column directly. A
//! `LambdaNode` is never wrapped this way (the `->` token IS the
//! construct's start), so its own span is used instead.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

const MSG_EMPTY_MISSING: &str = "Space missing inside empty braces.";
const MSG_EMPTY_DETECTED: &str = "Space inside empty braces detected.";
const MSG_LEFT_MISSING: &str = "Space missing inside {.";
const MSG_LEFT_DETECTED: &str = "Space inside { detected.";
const MSG_PIPE_MISSING: &str = "Space between { and | missing.";
const MSG_PIPE_DETECTED: &str = "Space between { and | detected.";
const MSG_RIGHT_MISSING: &str = "Space missing inside }.";
const MSG_RIGHT_DETECTED: &str = "Space inside } detected.";

/// RuboCop's `EnforcedStyle`/`EnforcedStyleForEmptyBraces` (both share the
/// same two values).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Space,
    NoSpace,
}

/// Looks for block braces that have or don't have surrounding space, per
/// configuration.
#[derive(Debug, Clone)]
pub struct SpaceInsideBlockBraces {
    style: Style,
    empty_braces_style: Style,
    space_before_block_parameters: bool,
}

impl Rule for SpaceInsideBlockBraces {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceInsideBlockBraces",
        department: Department::Layout,
        summary: "Checks that block braces have or don't have surrounding space.",
        explanation: "\
For blocks taking parameters, checks that the left brace has or doesn't have
trailing space depending on configuration.

```ruby
# bad (EnforcedStyle: space, the default)
some_array.each {puts e}

# good (EnforcedStyle: space, the default)
some_array.each { puts e }

# bad (EnforcedStyle: no_space)
some_array.each { puts e }

# good (EnforcedStyle: no_space)
some_array.each {puts e}

# bad (EnforcedStyleForEmptyBraces: no_space, the default)
some_array.each { }

# good (EnforcedStyleForEmptyBraces: no_space, the default)
some_array.each {}

# bad (EnforcedStyleForEmptyBraces: space)
some_array.each {}

# good (EnforcedStyleForEmptyBraces: space)
some_array.each { }

# bad (SpaceBeforeBlockParameters: true, the default)
[1, 2, 3].each {|n| n * 2 }

# good (SpaceBeforeBlockParameters: true, the default)
[1, 2, 3].each { |n| n * 2 }

# bad (SpaceBeforeBlockParameters: false)
[1, 2, 3].each { |n| n * 2 }

# good (SpaceBeforeBlockParameters: false)
[1, 2, 3].each {|n| n * 2 }
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::BlockNode, NodeKind::LambdaNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("space"),
                allowed: &["space", "no_space"],
                doc: "Whether block braces have surrounding space.",
            },
            ConfigOption {
                name: "EnforcedStyleForEmptyBraces",
                default: ConfigDefault::Str("no_space"),
                allowed: &["space", "no_space"],
                doc: "Whether empty block braces have a space in between.",
            },
            ConfigOption {
                name: "SpaceBeforeBlockParameters",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether there is a space between `{` and `|`. Overrides `EnforcedStyle` \
                      if there is a conflict.",
            },
        ],
        blind_spots: "\
`do`/`end` blocks are always skipped (matches RuboCop's `node.keywords?` \
guard). Ruby's `/\\R/` (any Unicode line separator) is approximated as a \
bare `\\n` check, and `/\\s/` as ASCII whitespace; both cover every case Ruby \
source can produce here except literal Unicode line separators inside a \
block's braces, which do not occur in practice.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match resolve_style(options, "EnforcedStyle", "space")? {
            "space" => Style::Space,
            _ => Style::NoSpace,
        };
        let empty_braces_style =
            match resolve_style(options, "EnforcedStyleForEmptyBraces", "no_space")? {
                "space" => Style::Space,
                _ => Style::NoSpace,
            };
        Ok(Self {
            style,
            empty_braces_style,
            space_before_block_parameters: options.bool("SpaceBeforeBlockParameters"),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::BlockNode { .. } => {
                let block = node.as_block_node().expect("kind matched");
                let column = ctx.parent().map_or_else(
                    || ctx.line_col(block.opening_loc().span().start).column,
                    |parent| ctx.line_col(parent.span.start).column,
                );
                self.check(
                    ctx,
                    block.opening_loc().span(),
                    block.closing_loc().span(),
                    block.body().is_none(),
                    pipe_delimiter(ctx, block.parameters()),
                    column,
                );
            }
            Node::LambdaNode { .. } => {
                let lambda = node.as_lambda_node().expect("kind matched");
                let column = ctx.line_col(node.location().span().start).column;
                self.check(
                    ctx,
                    lambda.opening_loc().span(),
                    lambda.closing_loc().span(),
                    lambda.body().is_none(),
                    pipe_delimiter(ctx, lambda.parameters()),
                    column,
                );
            }
            _ => {}
        }
    }
}

/// The `|`-delimited block-parameter opening, if any (RuboCop's
/// `node.arguments.loc.begin` narrowed to `pipe?`). `None` for a paramless
/// block, a numbered/it-parameter block, or a lambda's `(...)` params.
fn pipe_delimiter(ctx: &Context<'_>, parameters: Option<Node<'_>>) -> Option<Span> {
    let opening = parameters?.as_block_parameters_node()?.opening_loc()?.span();
    (ctx.text(opening) == b"|").then_some(opening)
}

/// An `EnforcedStyle`-like option that tolerates an explicit YAML `~`
/// (RuboCop's spec fixtures sometimes write this for "no override", which
/// [`RuleOptions::style`] would otherwise reject as a non-string value).
fn resolve_style<'a>(
    options: &'a RuleOptions,
    key: &str,
    default: &'static str,
) -> Result<&'a str, OptionError> {
    if matches!(options.get(key), Some(OptionValue::Null)) {
        return Ok(default);
    }
    options.style(key)
}

impl SpaceInsideBlockBraces {
    /// RuboCop's `on_block#check_inside`, after the `keywords?`/multiline-
    /// empty-braces guards RuboCop applies before calling it.
    fn check(
        &self,
        ctx: &mut Context<'_>,
        left_brace: Span,
        right_brace: Span,
        body_is_none: bool,
        pipe_delim: Option<Span>,
        column: u32,
    ) {
        if ctx.text(left_brace) != b"{" {
            return;
        }
        let left_line = ctx.line_col(left_brace.start).line;
        let right_line = ctx.line_col(right_brace.start).line;
        if body_is_none && left_line != right_line {
            return;
        }

        if left_brace.end == right_brace.start {
            self.adjacent_braces(ctx, left_brace, right_brace);
            return;
        }

        let inner_span = Span::new(left_brace.end, right_brace.start);
        let inner = ctx.text(inner_span);
        if inner.iter().any(|&b| !b.is_ascii_whitespace()) {
            self.check_left_brace(ctx, inner, left_brace, pipe_delim);
            self.check_right_brace(ctx, inner, left_brace, right_brace, column);
        } else if self.empty_braces_style == Style::NoSpace {
            report(ctx, i64::from(inner_span.start), i64::from(inner_span.end), MSG_EMPTY_DETECTED);
        }
    }

    /// RuboCop's `adjacent_braces`.
    fn adjacent_braces(&self, ctx: &mut Context<'_>, left_brace: Span, right_brace: Span) {
        if self.empty_braces_style != Style::Space {
            return;
        }
        report(ctx, i64::from(left_brace.start), i64::from(right_brace.end), MSG_EMPTY_MISSING);
    }

    /// RuboCop's `check_left_brace`.
    fn check_left_brace(
        &self,
        ctx: &mut Context<'_>,
        inner: &[u8],
        left_brace: Span,
        pipe_delim: Option<Span>,
    ) {
        let starts_non_space = inner.first().is_some_and(|&b| !b.is_ascii_whitespace());
        if starts_non_space {
            self.no_space_inside_left_brace(ctx, left_brace, pipe_delim);
        } else {
            self.space_inside_left_brace(ctx, left_brace, pipe_delim);
        }
    }

    /// RuboCop's `no_space_inside_left_brace`.
    fn no_space_inside_left_brace(
        &self,
        ctx: &mut Context<'_>,
        left_brace: Span,
        pipe_delim: Option<Span>,
    ) {
        if let Some(delim) = pipe_delim {
            if left_brace.end == delim.start && self.space_before_block_parameters {
                report(ctx, i64::from(left_brace.start), i64::from(delim.end), MSG_PIPE_MISSING);
            }
            return;
        }
        self.no_space(
            ctx,
            i64::from(left_brace.end),
            i64::from(left_brace.end) + 1,
            MSG_LEFT_MISSING,
        );
    }

    /// RuboCop's `space_inside_left_brace`.
    fn space_inside_left_brace(
        &self,
        ctx: &mut Context<'_>,
        left_brace: Span,
        pipe_delim: Option<Span>,
    ) {
        if let Some(delim) = pipe_delim {
            if !self.space_before_block_parameters {
                report(ctx, i64::from(left_brace.end), i64::from(delim.start), MSG_PIPE_DETECTED);
            }
            return;
        }
        let extended_end = extend_forward(ctx.source().bytes(), left_brace.end);
        self.space(ctx, i64::from(left_brace.end), i64::from(extended_end), MSG_LEFT_DETECTED);
    }

    /// RuboCop's `check_right_brace`.
    fn check_right_brace(
        &self,
        ctx: &mut Context<'_>,
        inner: &[u8],
        left_brace: Span,
        right_brace: Span,
        column: u32,
    ) {
        let single_line =
            ctx.line_col(left_brace.start).line == ctx.line_col(right_brace.start).line;
        let ends_non_space = inner.last().is_some_and(|&b| !b.is_ascii_whitespace());
        if single_line && ends_non_space {
            self.no_space(
                ctx,
                i64::from(right_brace.start),
                i64::from(right_brace.end),
                MSG_RIGHT_MISSING,
            );
            return;
        }
        if !single_line {
            let right_column = ctx.line_col(right_brace.start).column;
            if right_column == column || last_line_space_count(inner) == column {
                return;
            }
        }
        self.space_inside_right_brace(ctx, inner, right_brace, column);
    }

    /// RuboCop's `space_inside_right_brace`.
    fn space_inside_right_brace(
        &self,
        ctx: &mut Context<'_>,
        inner: &[u8],
        right_brace: Span,
        column: u32,
    ) {
        let bytes = ctx.source().bytes();
        let extended_begin = extend_backward(bytes, right_brace.start);
        let mut begin = i64::from(extended_begin);
        let mut end = i64::from(right_brace.start);

        let spans_newline =
            bytes[extended_begin as usize..right_brace.end as usize].contains(&b'\n');
        if spans_newline {
            let right_column = i64::from(ctx.line_col(right_brace.start).column);
            begin = end - (right_column - i64::from(column));
        }
        if inner.last() == Some(&b']') {
            end -= 1;
            let space_count = i64::from(last_line_space_count(inner));
            begin = end - (space_count - i64::from(column));
        }
        self.space(ctx, begin, end, MSG_RIGHT_DETECTED);
    }

    /// RuboCop's `no_space`: an offense only under `EnforcedStyle: space`.
    fn no_space(&self, ctx: &mut Context<'_>, begin: i64, end: i64, msg: &'static str) {
        if self.style == Style::Space {
            report(ctx, begin, end, msg);
        }
    }

    /// RuboCop's `space`: an offense only under `EnforcedStyle: no_space`.
    fn space(&self, ctx: &mut Context<'_>, begin: i64, end: i64, msg: &'static str) {
        if self.style == Style::NoSpace {
            report(ctx, begin, end, msg);
        }
    }
}

/// RuboCop's `offense`: `return if begin_pos > end_pos`, then reports with
/// an autocorrection keyed on the reported range's own source text.
fn report(ctx: &mut Context<'_>, begin: i64, end: i64, msg: &'static str) {
    if begin < 0 || begin > end {
        return;
    }
    let (Ok(begin), Ok(end)) = (u32::try_from(begin), u32::try_from(end)) else { return };
    let span = Span::new(begin, end);
    let fix = build_fix(ctx, span);
    ctx.report_with_fix(&SpaceInsideBlockBraces::META, span, msg, fix);
}

/// RuboCop's autocorrection `case range.source`.
fn build_fix(ctx: &Context<'_>, span: Span) -> Fix {
    let text = ctx.text(span);
    let edits = if text.iter().any(u8::is_ascii_whitespace) {
        vec![Edit::delete(span)]
    } else if text == b"{}" {
        vec![Edit::replace(span, b"{ }".as_slice())]
    } else if text == b"{|" {
        vec![Edit::replace(span, b"{ |".as_slice())]
    } else {
        vec![Edit::insert(span.start, b" ".as_slice())]
    };
    Fix { applicability: Applicability::Safe, edits }
}

/// RuboCop's `RangeHelp#final_pos` stepping forward with
/// `newlines: true, whitespace: false, continuations: false`: consumes a
/// run of spaces/tabs, then a run of bare `\n`s.
fn extend_forward(bytes: &[u8], mut pos: u32) -> u32 {
    let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    while pos < len && matches!(bytes[pos as usize], b' ' | b'\t') {
        pos += 1;
    }
    while pos < len && bytes[pos as usize] == b'\n' {
        pos += 1;
    }
    pos
}

/// The backward-stepping mirror of [`extend_forward`].
fn extend_backward(bytes: &[u8], mut pos: u32) -> u32 {
    while pos > 0 && matches!(bytes[(pos - 1) as usize], b' ' | b'\t') {
        pos -= 1;
    }
    while pos > 0 && bytes[(pos - 1) as usize] == b'\n' {
        pos -= 1;
    }
    pos
}

/// RuboCop's `inner_last_space_count`: the number of `' '` bytes on the last
/// `"\n"`-delimited line of `inner`.
#[allow(clippy::naive_bytecount)]
fn last_line_space_count(inner: &[u8]) -> u32 {
    let last_line = inner.rsplit(|&b| b == b'\n').next().unwrap_or(inner);
    u32::try_from(last_line.iter().filter(|&&b| b == b' ').count()).unwrap_or(u32::MAX)
}
