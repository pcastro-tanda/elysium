//! `Style/Lambda`, ported from RuboCop's `lib/rubocop/cop/style/lambda.rb`
//! plus the `LambdaLiteralToMethodCorrector` (`lib/rubocop/cop/correctors/
//! lambda_literal_to_method_corrector.rb`) it uses for one autocorrect
//! direction.
//!
//! # Two independent shapes
//!
//! Whitequork parses *both* `-> { }`/`-> do end` and `lambda { }`/
//! `lambda do end` as the same `block`-type node (in legacy mode, an arrow
//! literal's pseudo `send` node has method name `:lambda` and a source range
//! covering just the two `->` characters), so upstream's single `on_block`
//! handler tells them apart with `send_node.source == '->'`. Prism gives the
//! arrow literal its own dedicated [`NodeKind::LambdaNode`] -- never a
//! `CallNode` -- so this port has two independent entry points instead:
//! [`Lambda::check_literal_form`] for `NodeKind::LambdaNode` (the `->` side)
//! and [`Lambda::check_method_form`] for a `NodeKind::CallNode` named
//! `lambda` whose `block()` is a real `BlockNode` (a `BlockArgumentNode`,
//! Prism's shape for a `&block` pass, is not `block_type?` upstream either
//! and is excluded the same way).
//!
//! # Implicit `it`/numbered parameters
//!
//! Upstream's `on_numblock`/`on_itblock` aliases receive the same node type
//! whitequork uses for a declared-argument block (`BlockNode#arguments`
//! simply returns `[]` for them), so the corrector's `node.block_type?` guard
//! is what actually excludes them from the argument-rewriting steps. Prism
//! instead reuses one `LambdaNode`/`BlockNode` kind for every parameter
//! style, populating `parameters()` with a real `BlockParametersNode` for
//! explicit params, a synthetic `NumberedParametersNode`/`ItParametersNode`
//! for implicit ones, or `None` for a bare `-> do end`/`lambda do end` with
//! no parameters at all. [`Params::classify`] recovers upstream's split:
//! `Implicit` (numbered/`it`) skips every argument-rewriting step exactly
//! like `block_type?` being false does upstream; `None` and `Explicit` both
//! go through them, matching `block_type?` being true regardless of whether
//! any parameter was actually declared.
//!
//! # `do`/`end` -> `{ }` when converting an unparenthesized call argument
//!
//! `LambdaLiteralToMethodCorrector#replace_delimiters` only fires when
//! converting `->` to `lambda` (never the reverse) and the arrow literal
//! (or, through one `pair`-node hop, the keyword-argument hash that owns it)
//! is a non-receiver argument of an unparenthesized call -- otherwise
//! `has_many :kittens, lambda do ... end, source: cats` would parse with
//! `do`/`end` binding to the outermost call. Its
//! `current_node.sibling_index > 1` check is always true once that shape is
//! confirmed (a whitequork send node's children are
//! `[receiver, method_name, *args]`, so any real argument sits at index >= 2);
//! the only way to fail it is for the literal to be the call's *receiver*
//! instead of an argument. Prism has no equivalent of whitequork's flat `send`
//! children -- arguments always sit inside an extra `ArgumentsNode` layer the
//! receiver never does -- so [`arg_to_unparenthesized_call`] recovers the same
//! "argument, not receiver" distinction structurally: finding a `CallNode`
//! immediately below an `ArgumentsNode` (after optionally hopping over one
//! `AssocNode`/`HashNode`-or-`KeywordHashNode` pair-wrapper, mirroring the
//! single `pair_type?` hop upstream) proves it is an argument, since only
//! `ArgumentsNode` members can reach one; landing on a `CallNode` with no
//! intervening `ArgumentsNode` means the literal is that call's receiver
//! instead. Whether that `CallNode` itself is parenthesized is read from
//! [`Lambda::parenthesized_calls`], a per-file `Span -> bool` cache filled as
//! every `CallNode` in the file is entered (mirroring `Style/HashSyntax`'s
//! `facts` cache), since `Context::ancestors` carries only kind and span, not
//! a live node to query `parenthesized_call?` on directly.

use std::collections::HashMap;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    NodeInfo, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{BlockParametersNode, CallNode, LambdaNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    LineCountDependent,
    Lambda,
    Literal,
}

/// The shape of a lambda/block's declared parameters, recovered from
/// Prism's unified `parameters()` field. See the module docs.
enum Params<'pr> {
    /// No parameters at all: `-> do end`/`lambda do end`.
    None,
    /// A real, possibly-empty parameter list: `->(x)`, `-> x`, `->()`,
    /// `lambda { |x; y| }`.
    Explicit(BlockParametersNode<'pr>),
    /// An implicit numbered (`_1`) or `it` parameter -- never rewritten.
    Implicit,
}

impl<'pr> Params<'pr> {
    fn classify(node: Option<Node<'pr>>) -> Self {
        match node {
            None => Self::None,
            Some(n) => match n.as_block_parameters_node() {
                Some(p) => Self::Explicit(p),
                None => Self::Implicit,
            },
        }
    }
}

/// RuboCop's `message_line_modifier`.
fn modifier_text(style: Style, multiline: bool) -> &'static str {
    match style {
        Style::LineCountDependent => {
            if multiline {
                "multiline"
            } else {
                "single line"
            }
        }
        Style::Lambda | Style::Literal => "all",
    }
}

/// RuboCop's `arguments?`/`empty_and_without_delimiters?`, restricted to a
/// real `BlockParametersNode`: no declared regular or shadow parameters.
fn params_is_empty(params: &BlockParametersNode<'_>) -> bool {
    params.parameters().is_none() && params.locals().is_empty()
}

/// RuboCop's `lambda_arg_string`: regular parameters joined by `, `, plus
/// `; `-joined shadow (block-local) arguments when any are declared --
/// Prism keeps the two apart natively (`BlockParametersNode::parameters`
/// vs `::locals`), where whitequork's single `args` node mixes
/// `shadowarg`-typed children in among the rest.
fn lambda_arg_string(params: &BlockParametersNode<'_>, ctx: &Context<'_>) -> String {
    let mut regular = Vec::new();
    if let Some(parameters) = params.parameters() {
        ruby_ast::for_each_child(&parameters.as_node(), |child| {
            regular.push(String::from_utf8_lossy(ctx.text(child.span())).into_owned());
        });
    }
    let shadow: Vec<String> = params
        .locals()
        .iter()
        .map(|n| String::from_utf8_lossy(ctx.text(n.span())).into_owned())
        .collect();
    let mut result = regular.join(", ");
    if !shadow.is_empty() {
        result.push_str("; ");
        result.push_str(&shadow.join(", "));
    }
    result
}

/// One step of [`arg_to_unparenthesized_call`]'s upward walk: `parent_idx`
/// is an `ArgumentsNode` and the entry directly below it is a `CallNode`.
fn call_below_arguments(ancestors: &[NodeInfo], parent_idx: usize) -> Option<usize> {
    if ancestors[parent_idx].kind != NodeKind::ArgumentsNode || parent_idx == 0 {
        return None;
    }
    let call_idx = parent_idx - 1;
    (ancestors[call_idx].kind == NodeKind::CallNode).then_some(call_idx)
}

/// RuboCop's `LambdaLiteralToMethodCorrector#arg_to_unparenthesized_call?`.
/// See the module docs for the structural translation from whitequork's
/// flat `send` children to Prism's `ArgumentsNode`-wrapped ones.
fn arg_to_unparenthesized_call(
    ancestors: &[NodeInfo],
    parenthesized_calls: &HashMap<Span, bool>,
) -> bool {
    let Some(&immediate) = ancestors.last() else { return false };
    let call_idx = if immediate.kind == NodeKind::AssocNode {
        if ancestors.len() < 2 {
            return false;
        }
        let hash_idx = ancestors.len() - 2;
        if !matches!(ancestors[hash_idx].kind, NodeKind::HashNode | NodeKind::KeywordHashNode)
            || hash_idx == 0
        {
            return false;
        }
        match call_below_arguments(ancestors, hash_idx - 1) {
            Some(i) => i,
            None => return false,
        }
    } else {
        match call_below_arguments(ancestors, ancestors.len() - 1) {
            Some(i) => i,
            None => return false,
        }
    };
    parenthesized_calls.get(&ancestors[call_idx].span).is_some_and(|&parenthesized| !parenthesized)
}

/// Use the new lambda literal syntax for single-line blocks, and the
/// `lambda` method for multiline ones.
#[derive(Debug, Clone)]
pub struct Lambda {
    style: Style,
    /// `Span -> is this ``CallNode`` parenthesized?`, filled as every call in
    /// the file is entered. See the module docs.
    parenthesized_calls: HashMap<Span, bool>,
}

impl Lambda {
    /// RuboCop's `on_block` (`send_node.source == '->'` branch) plus
    /// `LambdaLiteralToMethodCorrector#call`.
    fn check_literal_form(&self, lambda: &LambdaNode<'_>, ctx: &mut Context<'_>) {
        let operator = lambda.operator_loc().span();
        let opening = lambda.opening_loc().span();
        let closing = lambda.closing_loc().span();
        let multiline = !ctx.same_line(opening, closing);
        let offending = match self.style {
            Style::Lambda => true,
            Style::Literal => false,
            Style::LineCountDependent => multiline,
        };
        if !offending {
            return;
        }
        let modifier = modifier_text(self.style, multiline);
        let message = format!("Use the `lambda` method for {modifier} lambdas.");

        let mut edits = vec![Edit::replace(operator, b"lambda".to_vec())];
        match Params::classify(lambda.parameters()) {
            Params::Implicit => {}
            Params::None => {
                if opening.start == operator.end {
                    edits.push(Edit::insert(opening.start, b" ".to_vec()));
                }
            }
            Params::Explicit(params) => {
                let params_span = params.location().span();
                let parens = params.opening_loc().map(|l| l.span());
                let parens_close = params.closing_loc().map(|l| l.span());
                if parens.is_none() {
                    // `remove_unparenthesized_whitespace`: trims around a
                    // bare (non-parenthesized) identifier list, leaving
                    // exactly one separating character before the opening
                    // delimiter.
                    edits.push(Edit::delete(Span::new(operator.end, params_span.start)));
                    if opening.start > params_span.end + 1 {
                        edits.push(Edit::delete(Span::new(params_span.end + 1, opening.start)));
                    }
                }
                let needs_space = matches!(
                    (parens, parens_close),
                    (Some(open), Some(close))
                        if close.end == opening.start && operator.end == open.start
                );
                if needs_space {
                    edits.push(Edit::insert(opening.start, b" ".to_vec()));
                }
                edits.push(Edit::delete(params_span));
                if !params_is_empty(&params) {
                    let arg_str = format!(" |{}|", lambda_arg_string(&params, ctx));
                    edits.push(Edit::insert(opening.end, arg_str.into_bytes()));
                }
            }
        }

        // `replace_delimiters`: only for a `do`/`end` literal being spliced
        // into an unparenthesized call as a non-receiver argument.
        if ctx.text(opening) != b"{"
            && arg_to_unparenthesized_call(ctx.ancestors(), &self.parenthesized_calls)
        {
            let separating_space = ctx
                .text(Span::new(opening.end, opening.end + 1))
                .first()
                .is_some_and(u8::is_ascii_whitespace);
            if !separating_space {
                edits.push(Edit::insert(opening.end, b" ".to_vec()));
            }
            edits.push(Edit::replace(opening, b"{".to_vec()));
            edits.push(Edit::replace(closing, b"}".to_vec()));
        }

        let fix = Fix { applicability: Applicability::Safe, edits };
        ctx.report_with_fix(&Self::META, operator, message, fix);
    }

    /// RuboCop's `on_block` (`send_node.source == 'lambda'` branch) plus
    /// `#autocorrect_method_to_literal`.
    fn check_method_form(&self, call: &CallNode<'_>, node: &Node<'_>, ctx: &mut Context<'_>) {
        if call.name().as_slice() != b"lambda" {
            return;
        }
        let Some(block_generic) = call.block() else { return };
        let Some(block) = block_generic.as_block_node() else { return };
        let opening = block.opening_loc().span();
        let closing = block.closing_loc().span();
        let multiline = !ctx.same_line(opening, closing);
        let offending = match self.style {
            Style::Lambda => false,
            Style::Literal => true,
            Style::LineCountDependent => !multiline,
        };
        if !offending {
            return;
        }
        let modifier = modifier_text(self.style, multiline);
        let message =
            format!("Use the `-> {{ ... }}` lambda literal syntax for {modifier} lambdas.");
        let span = call.message_loc().map_or_else(|| node.span(), |l| l.span());

        let mut edits = vec![Edit::replace(span, b"->".to_vec())];
        if let Params::Explicit(params) = Params::classify(block.parameters()) {
            if !params_is_empty(&params) {
                let arg_str = format!("({})", lambda_arg_string(&params, ctx));
                edits.push(Edit::insert(span.end, arg_str.into_bytes()));
                let remove_span = Span::new(opening.end, params.location().span().end);
                edits.push(Edit::delete(remove_span));
            }
        }
        let fix = Fix { applicability: Applicability::Safe, edits };
        ctx.report_with_fix(&Self::META, span, message, fix);
    }
}

impl Rule for Lambda {
    const META: RuleMeta = RuleMeta {
        name: "Style/Lambda",
        department: Department::Style,
        summary: "Use the new lambda literal syntax for single-line blocks.",
        explanation: "\
Checks the usage of a `lambda` literal syntax for single-line blocks and
method calls for multiline blocks. It is configurable to enforce one of the
styles for both single line and multiline lambdas as well.

```ruby
# EnforcedStyle: line_count_dependent (default)
# bad
f = lambda { |x| x }
f = ->(x) do
      x
    end

# good
f = ->(x) { x }
f = lambda do |x|
      x
    end
```

```ruby
# EnforcedStyle: lambda
# bad
f = ->(x) { x }
f = ->(x) do
      x
    end

# good
f = lambda { |x| x }
f = lambda do |x|
      x
    end
```

```ruby
# EnforcedStyle: literal
# bad
f = lambda { |x| x }
f = lambda do |x|
      x
    end

# good
f = ->(x) { x }
f = ->(x) do
      x
    end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::LambdaNode, NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("line_count_dependent"),
            allowed: &["line_count_dependent", "lambda", "literal"],
            doc: "Which lambda syntax to enforce.",
        }],
        blind_spots: "\
`self.autocorrect_incompatible_with` (`Style::SymbolProc`, avoiding a
`->(x)(&:method)` double-correction clash when both cops run together) is
not ported: this port has no cross-rule autocorrect-conflict mechanism.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "lambda" => Style::Lambda,
            "literal" => Style::Literal,
            _ => Style::LineCountDependent,
        };
        Ok(Self { style, parenthesized_calls: HashMap::new() })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.parenthesized_calls.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                self.parenthesized_calls.insert(node.span(), call.opening_loc().is_some());
                self.check_method_form(&call, node, ctx);
            }
            NodeKind::LambdaNode => {
                let lambda = node.as_lambda_node().expect("kind matched");
                self.check_literal_form(&lambda, ctx);
            }
            _ => {}
        }
    }
}
