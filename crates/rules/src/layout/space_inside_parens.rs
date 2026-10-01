//! `Layout/SpaceInsideParens`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_inside_parens.rb`.
//!
//! RuboCop's `on_new_investigation` walks `processed_source.sorted_tokens`
//! pairwise (`each_cons(2)`) over the *whole file*, asking of each adjacent
//! token pair whether either side is a `tLPAREN`/`tLPAREN2`/`tRPAREN`. Prism
//! hands us no token stream, only node locations, so this rule instead
//! visits every node whose own opening/closing location can literally be a
//! single `(`/`)` byte -- `CallNode` (`f(...)`), `DefNode`/`DefinedNode`/
//! `SuperNode`/`YieldNode` (`def foo(...)`/`defined?(...)`/`super(...)`/
//! `yield(...)`), `ParenthesesNode` (`(...)`), `MultiWriteNode`/
//! `MultiTargetNode` (mlhs destructuring groups: `(a, b) = x`, `|(a, b)|`),
//! `PinnedExpressionNode` (pattern-match `^(expr)`), `BlockParametersNode`
//! (stabby-lambda params `->(x) {}`), and the constant-prefixed pattern
//! variants of `ArrayPatternNode`/`FindPatternNode`/`HashPatternNode`
//! (`Point(x, y)`) -- and scans the raw bytes strictly *between* that node's
//! own open and close, exactly like `SpaceInsideHashLiteralBraces` does for
//! `{`/`}`.
//!
//! This is a faithful stand-in for the token-pair walk, not an
//! approximation: since parens (like braces) always nest, "the token
//! immediately following this open paren" can only ever be the first real
//! byte inside *this same node's own* interior -- a deeper node's own
//! opening paren, if one starts there, or ordinary content otherwise -- and
//! symmetrically for "the token immediately preceding this close paren".
//! Checking each node's own content once therefore reproduces the exact
//! same offense set as the global pairwise walk, including every
//! `compact`-style "consecutive parens" case, without needing any
//! cross-node adjacency search.
//!
//! Filtering every candidate node to locations whose *text* is the literal
//! single byte `(`/`)` (rather than trusting the field name alone) is what
//! excludes non-paren delimiters that happen to share a struct field:
//! `CallNode::opening_loc`/`closing_loc` is also used for `a[1]`'s `[`/`]`;
//! `ArrayPatternNode`/`FindPatternNode`/`HashPatternNode::opening_loc` is
//! `[`/`{` for the bracketed forms of those patterns; `BlockParametersNode`
//! is `|`/`|` for an ordinary `do |x| end` block. None of these ever have a
//! one-byte `(`/`)` text, so the filter drops them for free.
//!
//! Ruby's `\s` is modelled with [`is_ruby_whitespace`], which (unlike
//! `u8::is_ascii_whitespace`) also treats a lone vertical tab as blank,
//! matching MRI's own byte-based lexer.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{is_ruby_whitespace, Span};

/// RuboCop's `MSG`: reported whenever a space is present but forbidden.
const MSG: &str = "Space inside parentheses detected.";
/// RuboCop's `MSG_SPACE`: reported whenever a space is required but absent.
const MSG_SPACE: &str = "No space inside parentheses detected.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    NoSpace,
    Space,
    Compact,
}

/// True when `bytes` is exactly the single expected byte -- the filter that
/// keeps a candidate node's location from being some other single-byte
/// delimiter (only `(`/`)` ever reach this rule's checks).
fn is_single(bytes: &[u8], expected: u8) -> bool {
    matches!(bytes, [b] if *b == expected)
}

/// Resolves the open/close spans of a candidate node, restricted to a
/// literal single-byte `(`/`)` pair. See the module docs for why every
/// other shape (`[`, `{`, `|`, a percent-literal's multi-byte opener, ...)
/// is excluded here for free.
fn parens(node: &Node<'_>, ctx: &Context<'_>) -> Option<(Span, Span)> {
    let (open, close) = match node {
        Node::CallNode { .. } => {
            let n = node.as_call_node()?;
            (n.opening_loc()?.span(), n.closing_loc()?.span())
        }
        Node::DefNode { .. } => {
            let n = node.as_def_node()?;
            (n.lparen_loc()?.span(), n.rparen_loc()?.span())
        }
        Node::DefinedNode { .. } => {
            let n = node.as_defined_node()?;
            (n.lparen_loc()?.span(), n.rparen_loc()?.span())
        }
        Node::SuperNode { .. } => {
            let n = node.as_super_node()?;
            (n.lparen_loc()?.span(), n.rparen_loc()?.span())
        }
        Node::YieldNode { .. } => {
            let n = node.as_yield_node()?;
            (n.lparen_loc()?.span(), n.rparen_loc()?.span())
        }
        Node::ParenthesesNode { .. } => {
            let n = node.as_parentheses_node()?;
            (n.opening_loc().span(), n.closing_loc().span())
        }
        Node::MultiWriteNode { .. } => {
            let n = node.as_multi_write_node()?;
            (n.lparen_loc()?.span(), n.rparen_loc()?.span())
        }
        Node::MultiTargetNode { .. } => {
            let n = node.as_multi_target_node()?;
            (n.lparen_loc()?.span(), n.rparen_loc()?.span())
        }
        Node::PinnedExpressionNode { .. } => {
            let n = node.as_pinned_expression_node()?;
            (n.lparen_loc().span(), n.rparen_loc().span())
        }
        Node::BlockParametersNode { .. } => {
            let n = node.as_block_parameters_node()?;
            (n.opening_loc()?.span(), n.closing_loc()?.span())
        }
        Node::ArrayPatternNode { .. } => {
            let n = node.as_array_pattern_node()?;
            (n.opening_loc()?.span(), n.closing_loc()?.span())
        }
        Node::FindPatternNode { .. } => {
            let n = node.as_find_pattern_node()?;
            (n.opening_loc()?.span(), n.closing_loc()?.span())
        }
        Node::HashPatternNode { .. } => {
            let n = node.as_hash_pattern_node()?;
            (n.opening_loc()?.span(), n.closing_loc()?.span())
        }
        _ => return None,
    };
    if !is_single(ctx.text(open), b'(') || !is_single(ctx.text(close), b')') {
        return None;
    }
    Some((open, close))
}

/// Checks that parentheses have or don't have surrounding space depending on
/// configuration.
#[derive(Debug, Clone)]
pub struct SpaceInsideParens {
    style: Style,
    /// Opening-paren byte offsets already identified (while visiting their
    /// owning `CallNode`/`YieldNode`/`SuperNode`/`DefinedNode`, which is
    /// always entered first in this rule's pre-order traversal) as RuboCop's
    /// `tLPAREN_ARG` -- a paren-less command's receiver (`not (expr)`) or
    /// first argument (`foo (expr)`, `yield (expr)`, `super (expr)`,
    /// `defined? (expr)`), directly preceded by nothing but horizontal
    /// whitespace. `left_parens?`/`right_parens?` upstream only ever match
    /// `tLPAREN`/`tLPAREN2`/`tRPAREN`, never `tLPAREN_ARG`, so this specific
    /// open paren's own left-hand space is never checked (its matching
    /// close paren is an ordinary `tRPAREN` and is unaffected).
    exempt_opens: std::collections::HashSet<u32>,
}

/// RuboCop's lexer-level `tLPAREN_ARG` distinction (MRI's `IS_SPCARG`):
/// resolves to `Some` open-paren span when `node` is a paren-less
/// `CallNode`/`YieldNode`/`SuperNode`/`DefinedNode` whose receiver (`not
/// (expr)`, a `CallNode` named `!` whose own source is the `not` keyword) or
/// literal first argument/value is a `ParenthesesNode` directly preceded --
/// skipping only horizontal whitespace, and at least one byte of it -- by
/// the command's own message/keyword end. A later/non-first argument, or a
/// command that already owns its own call-parens, never matches (Ruby's
/// lexer only emits `tLPAREN_ARG` for the first token of a paren-less
/// command's argument list).
fn tlparen_arg_exemption(node: &Node<'_>, ctx: &Context<'_>) -> Option<Span> {
    let (preceding_end, candidate) = match node {
        Node::CallNode { .. } => {
            let call = node.as_call_node()?;
            let message = call.message_loc()?;
            if call.name().as_slice() == b"!" && ctx.text(message.span()) == b"not" {
                (message.span().end, call.receiver()?)
            } else {
                if call.opening_loc().is_some() {
                    return None;
                }
                (message.span().end, call.arguments()?.arguments().first()?)
            }
        }
        Node::YieldNode { .. } => {
            let y = node.as_yield_node()?;
            if y.lparen_loc().is_some() {
                return None;
            }
            (y.keyword_loc().span().end, y.arguments()?.arguments().first()?)
        }
        Node::SuperNode { .. } => {
            let s = node.as_super_node()?;
            if s.lparen_loc().is_some() {
                return None;
            }
            (s.keyword_loc().span().end, s.arguments()?.arguments().first()?)
        }
        Node::DefinedNode { .. } => {
            let d = node.as_defined_node()?;
            if d.lparen_loc().is_some() {
                return None;
            }
            (d.keyword_loc().span().end, d.value())
        }
        _ => return None,
    };
    // `candidate` need not be the `ParenthesesNode` itself: a receiver-chain
    // built on it (`foo ( 1 )[0]`'s index call, `foo ( 1 ).bar`'s method
    // call, ...) shares the exact same start offset as its own deepest
    // receiver, since Ruby's grammar can only begin such an expression with
    // a literal `(` byte by it being this `ParenthesesNode`'s own opening
    // paren -- so checking the leading byte directly (rather than
    // `candidate.as_parentheses_node()`, which only matches when the
    // argument/receiver *is* the paren, not merely starts with it) covers
    // both shapes uniformly.
    let start = candidate.span().start;
    if ctx.text(Span::new(start, start + 1)) != b"(" {
        return None;
    }
    let gap = ctx.text(Span::new(preceding_end, start));
    if gap.is_empty() || gap.iter().any(|&b| b != b' ' && b != b'\t') {
        return None;
    }
    Some(Span::new(start, start + 1))
}
impl SpaceInsideParens {
    /// Reports an offense removing `span` (RuboCop's `corrector.remove`).
    fn remove(ctx: &mut Context<'_>, span: Span, message: &'static str) {
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(span)] },
        );
    }

    /// Reports an offense inserting a single space at `at` (RuboCop's
    /// `corrector.insert_before(range, ' ')`), with the offense itself
    /// pointing at `range`.
    fn insert(ctx: &mut Context<'_>, range: Span, at: u32, message: &'static str) {
        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::insert(at, *b" ")] },
        );
    }

    /// RuboCop's `on_new_investigation`, restricted to the single `(...)`
    /// pair `open`/`close` belong to. `content` is the raw bytes strictly
    /// between them -- exactly the region a token-stream walk would ever
    /// need to inspect to decide this pair's own offenses (see module
    /// docs).
    fn check(&self, ctx: &mut Context<'_>, open: Span, close: Span, open_is_tlparen_arg: bool) {
        let content = ctx.text(Span::new(open.end, close.start));

        if content.is_empty() {
            // Already touching (`()`): every style leaves this alone.
            return;
        }

        if content.iter().all(|&b| is_ruby_whitespace(b)) {
            // `correct_extraneous_space_in_empty_parens`: unconditional for
            // `space`/`compact`, but (like every other check of this open
            // paren) skipped when `open` is an exempt `tLPAREN_ARG` -- it
            // requires `token1.left_parens?`, which a `tLPAREN_ARG` never
            // satisfies. For `no_space` this is just an ordinary
            // extraneous-space pair driven by `token2.right_parens?` alone,
            // so it still applies regardless (and still needs
            // `same_line?`, i.e. `correct_extraneous_space`'s own guard).
            let offense = match self.style {
                Style::NoSpace => !content.contains(&b'\n'),
                Style::Space | Style::Compact => !open_is_tlparen_arg,
            };
            if offense {
                Self::remove(ctx, Span::new(open.end, close.start), MSG);
            }
            return;
        }

        // `first`/`last` bound the real (non-whitespace) content; the runs
        // before `first` and after `last` are this pair's own left/right
        // gaps, each checked independently (RuboCop checks each side of a
        // pair separately too).
        let first = content.iter().position(|&b| !is_ruby_whitespace(b)).expect("not all blank");
        let last = content.iter().rposition(|&b| !is_ruby_whitespace(b)).expect("not all blank");

        if content[first] == b'#' {
            // A comment sits directly inside: RuboCop's `token2.comment?`
            // skip. Never reached on the trailing side: a comment always
            // runs to end of line, so it can never be the byte immediately
            // preceding a close paren on the same line.
        } else if !open_is_tlparen_arg && !content[..first].contains(&b'\n') {
            let consecutive_open = content[first] == b'(';
            let has_space = first > 0;
            match self.style {
                Style::NoSpace => {
                    if has_space {
                        let span =
                            Span::new(open.end, open.end + u32::try_from(first).unwrap_or(0));
                        Self::remove(ctx, span, MSG);
                    }
                }
                Style::Space => {
                    if !has_space {
                        Self::insert(ctx, Span::new(open.end, open.end + 1), open.end, MSG_SPACE);
                    }
                }
                Style::Compact => {
                    if consecutive_open {
                        // `correct_extraneous_space_between_consecutive_parens`:
                        // only an exact single-space gap is collapsed.
                        if first == 1 && content[0] == b' ' {
                            Self::remove(ctx, Span::new(open.end, open.end + 1), MSG);
                        }
                    } else if !has_space {
                        Self::insert(ctx, Span::new(open.end, open.end + 1), open.end, MSG_SPACE);
                    }
                }
            }
        }

        let trailing_len = content.len() - last - 1;
        if !content[last + 1..].contains(&b'\n') {
            let consecutive_close = content[last] == b')';
            let has_space = trailing_len > 0;
            let trailing_len_u32 = u32::try_from(trailing_len).unwrap_or(0);
            match self.style {
                Style::NoSpace => {
                    if has_space {
                        let span = Span::new(close.start - trailing_len_u32, close.start);
                        Self::remove(ctx, span, MSG);
                    }
                }
                Style::Space => {
                    if !has_space {
                        Self::insert(ctx, close, close.start, MSG_SPACE);
                    }
                }
                Style::Compact => {
                    if consecutive_close {
                        if trailing_len == 1 && content[content.len() - 1] == b' ' {
                            Self::remove(ctx, Span::new(close.start - 1, close.start), MSG);
                        }
                    } else if !has_space {
                        Self::insert(ctx, close, close.start, MSG_SPACE);
                    }
                }
            }
        }
    }
}

impl Rule for SpaceInsideParens {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceInsideParens",
        department: Department::Layout,
        summary: "Checks for spaces inside ordinary round parentheses.",
        explanation: "\
```ruby
# EnforcedStyle: no_space (default)
# The `no_space` style enforces that parentheses do not have spaces.

# bad
f( 3)
g = (a + 3 )
f( )

# good
f(3)
g = (a + 3)
f()
```

```ruby
# EnforcedStyle: space
# The `space` style enforces that parentheses have a space at the
# beginning and end.
# Note: Empty parentheses should not have spaces.

# bad
f(3)
g = (a + 3)
y( )

# good
f( 3 )
g = ( a + 3 )
y()
```

```ruby
# EnforcedStyle: compact
# The `compact` style enforces that parentheses have a space at the
# beginning with the exception that successive parentheses are allowed.
# Note: Empty parentheses should not have spaces.

# bad
f(3)
g = (a + 3)
y( )
g( f( x ) )
g( f( x( 3 ) ), 5 )
g( ( ( 3 + 5 ) * f) ** x, 5 )

# good
f( 3 )
g = ( a + 3 )
y()
g( f( x ))
g( f( x( 3 )), 5 )
g((( 3 + 5 ) * f ) ** x, 5 )
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::CallNode,
            NodeKind::DefNode,
            NodeKind::DefinedNode,
            NodeKind::SuperNode,
            NodeKind::YieldNode,
            NodeKind::ParenthesesNode,
            NodeKind::MultiWriteNode,
            NodeKind::MultiTargetNode,
            NodeKind::PinnedExpressionNode,
            NodeKind::BlockParametersNode,
            NodeKind::ArrayPatternNode,
            NodeKind::FindPatternNode,
            NodeKind::HashPatternNode,
        ],
        config: &[linter::ConfigOption {
            name: "EnforcedStyle",
            default: linter::ConfigDefault::Str("no_space"),
            allowed: &["space", "no_space", "compact"],
            doc: "Whether parentheses require, forbid, or (for `compact`) selectively collapse \
                  surrounding space.",
        }],
        blind_spots: "\
A `%`-literal whose closing delimiter is a lone `)` (`%(...)`, `%w(...)`,
`%i(...)`, `%r(...)`, `%q(...)`/`%Q(...)`, `%s(...)`, `%x(...)`) is not a
real paren token upstream (the whole literal, delimiters included, is one
lexer token), but if it sits immediately -- whitespace only, nothing else
between -- before a real close paren inside a checked node's own content
(e.g. `f(%(a) )`), this rule's `compact`-style consecutive-close-paren
check reads that literal's trailing `)` byte as a neighbouring real paren
and would collapse the single space between them. This mirrors a known gap
in `SpaceInsideArrayLiteralBrackets`'s own token-stream re-derivation and is
expected to be exercised only by deliberately adversarial input.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "space" => Style::Space,
            "compact" => Style::Compact,
            _ => Style::NoSpace,
        };
        Ok(Self { style, exempt_opens: std::collections::HashSet::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(open) = tlparen_arg_exemption(node, ctx) {
            self.exempt_opens.insert(open.start);
        }
        let Some((open, close)) = parens(node, ctx) else { return };
        let open_is_tlparen_arg = self.exempt_opens.remove(&open.start);
        self.check(ctx, open, close, open_is_tlparen_arg);
    }
}
