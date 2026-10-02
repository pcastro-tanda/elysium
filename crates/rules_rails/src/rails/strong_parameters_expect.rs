//! `Rails/StrongParametersExpect`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/strong_parameters_expect.rb`.
//!
//! whitequark wraps a `send` that carries a literal block in a `block` node,
//! and an `&block` argument is a trailing `block_pass` argument; Prism keeps
//! both on the `CallNode`. [`wq_arguments`] and the [`CallFrame`] block span
//! bridge the two shapes.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MINIMUM_TARGET_RAILS_VERSION: f64 = 8.0;

/// Method calls on `params[:key]` that should not be rewritten with
/// `expect(:key)`.
const IGNORED_METHODS: &[&[u8]] = &[
    b"!",
    b"blank?",
    b"compact",
    b"compact!",
    b"compact_blank",
    b"compact_blank!",
    b"deep_merge",
    b"deep_merge!",
    b"delete",
    b"delete_if",
    b"dig",
    b"each",
    b"except",
    b"exclude?",
    b"extract!",
    b"fetch",
    b"has_key?",
    b"has_value?",
    b"include?",
    b"inspect",
    b"instance_of?",
    b"is_a?",
    b"keep_if",
    b"key?",
    b"keys",
    b"kind_of?",
    b"member?",
    b"merge",
    b"merge!",
    b"nil?",
    b"presence",
    b"present?",
    b"reverse_merge",
    b"reverse_merge!",
    b"slice",
    b"stringify_keys",
    b"to_a",
    b"to_f",
    b"to_h",
    b"to_hash",
    b"to_i",
    b"to_s",
    b"to_unsafe_h",
    b"to_unsafe_hash",
    b"transform_keys",
    b"transform_keys!",
    b"transform_values",
    b"transform_values!",
    b"try",
    b"try!",
    b"value?",
    b"values",
    b"values_at",
    b"with_defaults",
    b"with_defaults!",
    b"without",
];

const RAISING_FINDER_METHODS: &[&[u8]] = &[b"find", b"find_by!", b"find_sole_by"];

/// `Node::COMPARISON_OPERATORS`.
const COMPARISON_OPERATORS: &[&[u8]] = &[b"==", b"===", b"!=", b"<=", b">=", b">", b"<"];

/// What the checks need to know about an enclosing call.
#[derive(Debug, Clone)]
struct CallFrame {
    /// The span of a literal `{}`/`do...end` block, whose body whitequark
    /// does not nest under the `send`.
    block_span: Option<Span>,
    receiver_span: Option<Span>,
    name: Vec<u8>,
    safe_navigation: bool,
    has_block_pass: bool,
}

impl CallFrame {
    fn new(call: &CallNode<'_>) -> Self {
        let block = call.block();
        Self {
            block_span: block.as_ref().filter(|b| b.as_block_node().is_some()).map(Node::span),
            receiver_span: call.receiver().map(|r| r.span()),
            name: call.name().as_slice().to_vec(),
            safe_navigation: call.is_safe_navigation(),
            has_block_pass: block.is_some_and(|b| b.as_block_argument_node().is_some()),
        }
    }

    fn raising_finder(&self) -> bool {
        RAISING_FINDER_METHODS.contains(&self.name.as_slice())
    }
}

/// Enforces `ActionController::Parameters#expect` for strong parameters.
#[derive(Debug, Clone)]
pub struct StrongParametersExpect {
    supported: bool,
    frames: Vec<CallFrame>,
    ignored: Vec<Span>,
}

impl Rule for StrongParametersExpect {
    const META: RuleMeta = RuleMeta {
        name: "Rails/StrongParametersExpect",
        department: Department::Rails,
        summary: "Enforces the use of `ActionController::Parameters#expect` as a method for \
                  strong parameter handling.",
        explanation: "Enforces the use of `ActionController::Parameters#expect` as a method for \
                      strong parameter handling.\n\nIn the following cases, `params[:key]` is \
                      treated as a key that is expected to be passed from the HTTP client, and \
                      the cop detects it using the `expect` method.\n\n- Method calls on \
                      `params[:key]` without comparison methods, methods that are safe to call \
                      on `nil` (such as `to_i`, `to_s`, or `is_a?`), key-check methods such as \
                      `key?`, collection methods such as `keys`, `merge`, or `slice`, or \
                      block-style calls such as `params[:key].each { ... }` or \
                      `params[:key].map(&:to_s)`\n- Passing `params[:key]` as an argument to \
                      finder methods that raise on missing records\n- Strong parameter methods \
                      using `require` or `permit`\n\nOther cases are not detected, as they are \
                      cases where `params[:key]` may not be passed from the HTTP client.\n\nThe \
                      autocorrection is unsafe: the HTTP status may change from 500 to 400, \
                      `expect` is stricter about the structure of nested attributes, and \
                      `find` accepts an array of IDs while `expect(:id)` requires a \
                      scalar.\n\n```ruby\n# bad\nparams[:key].do_something\n\n# good\n\
                      params.expect(:key).do_something\n\n# bad\nModel.find(params[:id])\n\n\
                      # good\nModel.find(params.expect(:id))\n\n# bad\n\
                      params.require(:user).permit(:name, :age)\n\
                      params.permit(user: [:name, :age]).require(:user)\n\n# good\n\
                      params.expect(user: [:name, :age])\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            supported: options.target_rails_version() >= MINIMUM_TARGET_RAILS_VERSION,
            frames: Vec::new(),
            ignored: Vec::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if self.supported {
            self.check(node, &call, ctx);
        }
        self.frames.push(CallFrame::new(&call));
    }

    fn leave(&mut self, _node: &Node<'_>, _ctx: &mut Context<'_>) {
        self.frames.pop();
    }
}

impl StrongParametersExpect {
    fn check(&mut self, node: &Node<'_>, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        let name = call.name();
        if !matches!(name.as_slice(), b"[]" | b"require" | b"permit") {
            return;
        }
        let span = call_span_excluding_block(call);
        if self.ignored.iter().any(|ignored| ignored.start <= span.start && span.end <= ignored.end)
        {
            return;
        }
        if name.as_slice() == b"[]" {
            self.check_bracket_access(node, call, ctx);
            return;
        }
        let matched = if name.as_slice() == b"permit" {
            params_require_permit(call).map(|require| {
                // (permit_method, require_method)
                let key = require_key(&wq_arguments(&require)[0], ctx);
                let permit_args = wq_arguments(call);
                let prefer = format!("expect({key}[{}])", join_sources(&permit_args, ctx));
                (*call, require, prefer, true)
            })
        } else {
            params_permit_require(call, ctx).map(|permit| {
                let prefer = format!("expect({})", join_sources(&wq_arguments(&permit), ctx));
                (permit, *call, prefer, false)
            })
        };
        let Some((permit_method, require_method, prefer, replace_argument)) = matched else {
            return;
        };

        // `offense_range(require_method, node)` / `offense_range(permit_method, node)`.
        let anchor = if replace_argument { &require_method } else { &permit_method };
        let (Some(anchor_selector), Some(permit_selector)) =
            (anchor.message_loc(), permit_method.message_loc())
        else {
            return;
        };
        let range = Span::new(anchor_selector.span().start, span.end);

        let Some(require_receiver) = require_method.receiver() else { return };
        let mut edits = vec![
            Edit::delete(Span::new(
                require_receiver.span().end,
                call_span_excluding_block(&require_method).end,
            )),
            Edit::replace(permit_selector.span(), b"expect".to_vec()),
        ];
        if replace_argument {
            let permit_args = wq_arguments(&permit_method);
            let (Some(first), Some(last)) = (permit_args.first(), permit_args.last()) else {
                return;
            };
            let key = require_key(&wq_arguments(&require_method)[0], ctx);
            edits.push(Edit::insert(first.span().start, format!("{key}[").into_bytes()));
            edits.push(Edit::insert(last.span().end, b"]".to_vec()));
        }
        ctx.report_with_fix(
            &Self::META,
            range,
            format!("Use `{prefer}` instead."),
            Fix { applicability: Applicability::Unsafe, edits },
        );
        self.ignored.push(span);
    }

    /// `params_bracket_access` + `register_bracket_access_offense`.
    fn check_bracket_access(&self, node: &Node<'_>, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        // `(send (send nil? :params) :[] $_)`.
        if call.is_safe_navigation() {
            return;
        }
        let Some(receiver) = call.receiver().and_then(|r| r.as_call_node()) else { return };
        if receiver.name().as_slice() != b"params"
            || receiver.receiver().is_some()
            || receiver.arguments().is_some()
            || receiver.is_safe_navigation()
            || receiver.block().is_some()
        {
            return;
        }
        let arguments = wq_arguments(call);
        let [key] = arguments.as_slice() else { return };
        if !self.offensive_bracket_access(node, ctx) {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        let range = Span::new(selector.span().start, node.span().end);
        let prefer = format!("expect({})", String::from_utf8_lossy(ctx.text(key.span())));
        let replacement = format!(".{prefer}").into_bytes();
        ctx.report_with_fix(
            &Self::META,
            range,
            format!("Use `{prefer}` instead."),
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(range, replacement)],
            },
        );
    }

    fn offensive_bracket_access(&self, node: &Node<'_>, ctx: &Context<'_>) -> bool {
        let node_span = node.span();
        let Some(parent) = self.parent_of(ctx) else { return false };
        if matches!(parent, Parent::Or) {
            return false;
        }
        let parent_call = match &parent {
            Parent::Call(frame) => Some(frame),
            _ => None,
        };
        if let Some(frame) = parent_call {
            if frame.safe_navigation && frame.receiver_span == Some(node_span) {
                return false;
            }
        }
        // `parent.each_ancestor(:call).any? { raising_finder_method? }`,
        // which includes `parent` itself.
        let inside_block = |frame: &CallFrame| {
            frame.block_span.is_some_and(|b| b.start <= node_span.start && node_span.end <= b.end)
        };
        if self.frames.iter().any(|frame| frame.raising_finder() && !inside_block(frame)) {
            return true;
        }
        let Some(frame) = parent_call else { return false };
        if frame.receiver_span != Some(node_span) {
            // `raising_finder_method?(parent)`, already covered above.
            return false;
        }
        if COMPARISON_OPERATORS.contains(&frame.name.as_slice()) || frame.name == b"[]" {
            return false;
        }
        if frame.block_span.is_some() || frame.has_block_pass {
            return false;
        }
        !IGNORED_METHODS.contains(&frame.name.as_slice())
    }

    /// The whitequark parent of the node being entered, as far as the checks
    /// need it.
    fn parent_of(&self, ctx: &Context<'_>) -> Option<Parent<'_>> {
        let ancestors = ctx.ancestors();
        let parent = ancestors.last()?;
        Some(match parent.kind {
            NodeKind::OrNode => Parent::Or,
            NodeKind::CallNode => self.frames.last().map_or(Parent::Other, Parent::Call),
            NodeKind::ArgumentsNode
                if ancestors
                    .len()
                    .checked_sub(2)
                    .is_some_and(|i| ancestors[i].kind == NodeKind::CallNode) =>
            {
                self.frames.last().map_or(Parent::Other, Parent::Call)
            }
            _ => Parent::Other,
        })
    }
}

enum Parent<'a> {
    Or,
    Call(&'a CallFrame),
    Other,
}

/// A call's whitequark arguments: positional ones plus a trailing
/// `block_pass` for `&block`.
fn wq_arguments<'a>(call: &CallNode<'a>) -> Vec<Node<'a>> {
    let mut arguments: Vec<Node<'a>> =
        call.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
    if let Some(block) = call.block() {
        if block.as_block_argument_node().is_some() {
            arguments.push(block);
        }
    }
    arguments
}

fn join_sources(nodes: &[Node<'_>], ctx: &Context<'_>) -> String {
    nodes
        .iter()
        .map(|n| String::from_utf8_lossy(ctx.text(n.span())).into_owned())
        .collect::<Vec<_>>()
        .join(", ")
}

/// `(send nil? :params)`.
fn is_params(node: &Node<'_>) -> bool {
    node.as_call_node().is_some_and(|c| {
        c.name().as_slice() == b"params"
            && c.receiver().is_none()
            && c.arguments().is_none()
            && c.block().is_none()
            && !c.is_safe_navigation()
    })
}

/// Whitequark's `call` (a `send`/`csend`; one with a literal block is a
/// `block`).
fn is_call(node: &Node<'_>) -> bool {
    node.as_call_node().is_some_and(|c| c.block().is_none_or(|b| b.as_block_node().is_none()))
}

/// `params_require_permit` for a `permit` call `node`: returns the inner
/// `require` call.
fn params_require_permit<'a>(node: &CallNode<'a>) -> Option<CallNode<'a>> {
    let permit_args = wq_arguments(node);
    if permit_args.is_empty() {
        return None;
    }
    let require = node.receiver()?.as_call_node()?;
    if require.block().is_some_and(|b| b.as_block_node().is_some())
        || require.name().as_slice() != b"require"
        || !require.receiver().is_some_and(|r| is_params(&r))
    {
        return None;
    }
    let require_args = wq_arguments(&require);
    let [key] = require_args.as_slice() else { return None };
    if key.as_array_node().is_some() {
        return None;
    }
    // `!(call _ :permit {call lvar ivar cvar gvar const})`.
    if let [only] = permit_args.as_slice() {
        if is_call(only)
            || matches!(
                only.kind(),
                NodeKind::LocalVariableReadNode
                    | NodeKind::InstanceVariableReadNode
                    | NodeKind::ClassVariableReadNode
                    | NodeKind::GlobalVariableReadNode
                    | NodeKind::ConstantReadNode
                    | NodeKind::ConstantPathNode
            )
        {
            return None;
        }
    }
    Some(require)
}

/// `params_permit_require` for a `require` call `node`: returns the inner
/// `permit` call.
fn params_permit_require<'a>(node: &CallNode<'a>, ctx: &Context<'_>) -> Option<CallNode<'a>> {
    let require_args = wq_arguments(node);
    let [require_key_node] = require_args.as_slice() else { return None };
    let permit = node.receiver()?.as_call_node()?;
    if permit.block().is_some_and(|b| b.as_block_node().is_some())
        || permit.name().as_slice() != b"permit"
        || !permit.receiver().is_some_and(|r| is_params(&r))
    {
        return None;
    }
    let permit_args = wq_arguments(&permit);
    let [hash] = permit_args.as_slice() else { return None };
    let elements: Vec<Node<'_>> = match hash.kind() {
        NodeKind::HashNode => hash.as_hash_node()?.elements().iter().collect(),
        NodeKind::KeywordHashNode => hash.as_keyword_hash_node()?.elements().iter().collect(),
        _ => return None,
    };
    let [element] = elements.as_slice() else { return None };
    let pair = element.as_assoc_node()?;
    same_node(&pair.key(), require_key_node, ctx).then_some(permit)
}

/// Structural node equality, as far as keys go: symbols and strings by
/// value, anything else by kind and source.
fn same_node(a: &Node<'_>, b: &Node<'_>, ctx: &Context<'_>) -> bool {
    if a.kind() != b.kind() {
        return false;
    }
    match a.kind() {
        NodeKind::SymbolNode => match (a.as_symbol_node(), b.as_symbol_node()) {
            (Some(x), Some(y)) => x.unescaped() == y.unescaped(),
            _ => false,
        },
        NodeKind::StringNode => match (a.as_string_node(), b.as_string_node()) {
            (Some(x), Some(y)) => x.unescaped() == y.unescaped(),
            _ => false,
        },
        _ => ctx.text(a.span()) == ctx.text(b.span()),
    }
}

/// `require_key`: `user: ` for a literal key (`respond_to?(:value)`), else
/// `source => `.
fn require_key(first_argument: &Node<'_>, ctx: &Context<'_>) -> String {
    match first_argument.kind() {
        NodeKind::SymbolNode => first_argument.as_symbol_node().map(|s| s.unescaped().to_vec()),
        NodeKind::StringNode => first_argument.as_string_node().map(|s| s.unescaped().to_vec()),
        NodeKind::IntegerNode | NodeKind::FloatNode => {
            Some(ctx.text(first_argument.span()).to_vec())
        }
        _ => None,
    }
    .map_or_else(
        || format!("{} => ", String::from_utf8_lossy(ctx.text(first_argument.span()))),
        |value| format!("{}: ", String::from_utf8_lossy(&value)),
    )
}
