//! `Layout/MultilineMethodCallBraceLayout`, ported from RuboCop's
//! `lib/rubocop/cop/layout/multiline_method_call_brace_layout.rb` plus the
//! `MultilineLiteralBraceLayout` mixin and `MultilineLiteralBraceCorrector`
//! it uses, shared with the other three cops in this family in
//! [`super::multiline_literal_brace_layout`].
//!
//! This cop's own `on_send`/`on_csend` overrides `children(node)` to
//! `node.arguments` and `ignored_literal?` to add
//! `single_line_ignoring_receiver?`: a call's own parens can sit entirely on
//! one line even while the call's full span (receiver included) does not,
//! e.g. `[\n].join(" ")`. [`BraceLiteral::whole_span`] is the *full* call
//! span for the shared `single_line?` check, but [`enter`] checks
//! `single_line_ignoring_receiver?` itself first and returns early, since it
//! is not expressible through that one shared field.
//!
//! `use_heredoc_argument_method_chain?` (moving a trailing `.method`/`&.method`
//! chain after the closing paren once it is relocated) needs to know, while
//! visiting a call's receiver, whether that receiver is itself a call whose
//! first argument is a heredoc -- a fact about the *parent* call the
//! engine's single top-down pass has not yet examined when it reaches the
//! receiver. [`record_heredoc_chain`] runs while visiting the outer
//! (chained) call, before the engine descends into its receiver, and
//! records the already-resolved chain-removal span/text keyed by the
//! receiver's own span, exactly like [`record_call_relations`]'s
//! `chained?`/`argument?` bookkeeping.

use std::collections::{HashMap, HashSet};

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use super::multiline_literal_brace_layout::{
    check_brace_layout, record_call_relations, resolve_style, BraceLiteral, Messages, Style,
};

const MESSAGES: Messages = Messages {
    same: "Closing method call brace must be on the same line as the last argument when \
                opening brace is on the same line as the first argument.",
    new: "Closing method call brace must be on the line after the last argument when \
               opening brace is on a separate line from the first argument.",
    always_new: "Closing method call brace must be on the line after the last argument.",
    always_same: "Closing method call brace must be on the same line as the last argument.",
};

/// Checks that the closing brace in a method call is either on the same line as the last method argument, or a new line.
#[derive(Debug, Clone)]
pub struct MultilineMethodCallBraceLayout {
    style: Style,
    /// RuboCop's `node.chained?`: spans of nodes that are some call's receiver.
    chained: HashSet<Span>,
    /// RuboCop's `node.argument?`: spans of nodes that are some (non-safe-nav) call's argument.
    arguments: HashSet<Span>,
    /// RuboCop's `use_heredoc_argument_method_chain?`, precomputed by receiver span: the
    /// chained method's own span (to delete) and source text (to reinsert).
    heredoc_chains: HashMap<Span, (Span, Box<[u8]>)>,
}

impl Rule for MultilineMethodCallBraceLayout {
    const META: RuleMeta = RuleMeta {
        name: "Layout/MultilineMethodCallBraceLayout",
        department: Department::Layout,
        summary: "Checks that the closing brace in a method call is either on the same line as the last method argument, or a new line.",
        explanation: "\
Checks that the closing brace in a method call is either on the same line
as the last method argument, or a new line.

When using the `symmetrical` (default) style:

If a method call's opening brace is on the same line as the first argument
of the call, then the closing brace should be on the same line as the last
argument of the call.

If a method call's opening brace is on the line above the first argument of
the call, then the closing brace should be on the line below the last
argument of the call.

When using the `new_line` style, the closing brace of a multi-line method
call must be on the line after the last argument of the call.

When using the `same_line` style, the closing brace of a multi-line method
call must be on the same line as the last argument of the call.

```ruby
# EnforcedStyle: symmetrical (default)

# bad
foo(a,
  b
)

# bad
foo(
  a,
  b)

# good
foo(a,
  b)

# good
foo(
  a,
  b
)
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("symmetrical"),
            allowed: &["symmetrical", "new_line", "same_line"],
            doc: "Whether the closing brace mirrors the opening brace's own \
                  line (`symmetrical`), always sits on the line after the \
                  last argument (`new_line`), or always sits on the same \
                  line as the last argument (`same_line`).",
        }],
        blind_spots: "\
If the closing delimiter's own physical line also holds a trailing chained
`.method`/`&.method` call *and* the last argument's own line carries a
trailing comment, both `content_if_comment_present`'s whole-line capture and
`use_heredoc_argument_method_chain?`'s chain relocation would need to
cooperate on the same stretch of text; RuboCop's own corrector has this same
gap (its two `TreeRewriter` actions are simply queued independently), and no
such case is reachable from this cop's own preconditions (the chain move
requires the call's own *first* argument to be a heredoc, and RuboCop's own
heredoc-safety guard already forbids autocorrecting a literal whose last
child's line carries a trailing comment together with a heredoc argument in
the same call).",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            style: resolve_style(options)?,
            chained: HashSet::new(),
            arguments: HashSet::new(),
            heredoc_chains: HashMap::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.chained.clear();
        self.arguments.clear();
        self.heredoc_chains.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Node::CallNode { .. } = node else { return };
        let call = node.as_call_node().expect("kind matched");
        record_call_relations(&call, &mut self.chained, &mut self.arguments);
        record_heredoc_chain(&call, ctx, &mut self.heredoc_chains);

        // `implicit_literal?`/`empty_literal?`: no parens, or parens with no arguments at
        // all. Whitequark's `Send` map never sets `loc.begin`/`loc.end` for an index call
        // (`a[b]`/`a[b] = c`) at all -- only for an actual `(...)` parenthesized call --
        // unlike Prism, whose `opening_loc`/`closing_loc` cover `[`/`]` just the same;
        // requiring the opening delimiter's own text reproduces that whitequark gap.
        let Some(opening) = call.opening_loc() else { return };
        if ctx.text(opening.span()) != b"(" {
            return;
        }
        let Some(closing) = call.closing_loc() else { return };
        let Some(arguments) = call.arguments() else { return };

        // `single_line_ignoring_receiver?`: the call's own parens can be single-line even
        // while the full call (receiver included) is not.
        if ctx.same_line(opening.span(), closing.span()) {
            return;
        }

        let children: Vec<Node<'_>> = arguments.arguments().iter().collect();
        let span = node.span();
        let literal = BraceLiteral {
            opening: Some(opening.span()),
            closing: closing.span(),
            children,
            whole_span: span,
            chained_or_argument: self.chained.contains(&span) || self.arguments.contains(&span),
            heredoc_chain: self.heredoc_chains.get(&span).cloned(),
        };
        check_brace_layout(ctx, &Self::META, self.style, &MESSAGES, &literal);
    }
}

/// RuboCop's `use_heredoc_argument_method_chain?`, precomputed while visiting the *outer*
/// chained call (`parent` in RuboCop's terms) -- see the module doc comment.
fn record_heredoc_chain(
    outer: &CallNode<'_>,
    ctx: &Context<'_>,
    chains: &mut HashMap<Span, (Span, Box<[u8]>)>,
) {
    let Some(receiver) = outer.receiver() else { return };
    let Some(operator) = outer.call_operator_loc() else { return };
    let Some(inner) = receiver.as_call_node() else { return };
    let Some(first_argument) = inner.arguments().and_then(|args| args.arguments().iter().next())
    else {
        return;
    };
    if !is_plain_heredoc_str(&first_argument, ctx) {
        return;
    }
    let chain_span = Span::new(operator.span().start, outer.location().span().end);
    chains.insert(receiver.span(), (chain_span, ctx.text(chain_span).into()));
}

/// RuboCop-AST's `str_type? && heredoc?`: a plain (non-interpolated) heredoc string, excluding
/// `dstr`/`xstr` -- Prism's [`Node::StringNode`] specifically.
fn is_plain_heredoc_str(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    let Node::StringNode { .. } = node else { return false };
    let str_node = node.as_string_node().expect("kind matched");
    let Some(opening) = str_node.opening_loc() else { return false };
    ctx.text(opening.span()).starts_with(b"<<")
}
