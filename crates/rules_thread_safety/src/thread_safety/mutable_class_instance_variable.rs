//! `ThreadSafety/MutableClassInstanceVariable`, ported from rubocop-thread_safety's
//! `lib/rubocop/cop/thread_safety/mutable_class_instance_variable.rb`.
//!
//! whitequark wraps a block's whole call (receiver and arguments included) in
//! the `block` node, so `ancestors` of a node inside those parts contain the
//! block. Prism's `BlockNode` is only the call's `block()`; to stay literal the
//! rule keeps its own stack of "whitequark ancestors" (`Frame`), pushed when
//! entering the whole call and popped when leaving it.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{ext, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Freeze mutable objects assigned to class instance variables.";

/// An ancestor in whitequark's sense.
#[derive(Debug, Clone)]
enum Frame {
    /// `class` with its superclass' `const_name`.
    Class(Option<String>),
    Module,
    Def,
    /// A `block` (call with a literal block, or lambda): method name and
    /// whether its send has a nil receiver.
    Block { name: Vec<u8>, nil_receiver: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnforcedStyle {
    Literals,
    Strict,
}

/// Do not assign mutable objects to class instance variables.
#[derive(Debug, Clone)]
pub struct MutableClassInstanceVariable {
    style: EnforcedStyle,
    target_ruby: f32,
    frames: Vec<Frame>,
}

/// A `BlockNode` that whitequark types as `block` (not `numblock`/`itblock`).
fn is_plain_block(block: &Node<'_>, target_ruby: f32) -> bool {
    let Some(block) = block.as_block_node() else { return false };
    !block.parameters().is_some_and(|params| numbered_or_it(&params, target_ruby))
}

fn numbered_or_it(params: &Node<'_>, target_ruby: f32) -> bool {
    match params.kind() {
        NodeKind::NumberedParametersNode => true,
        NodeKind::ItParametersNode => target_ruby >= 3.4,
        _ => false,
    }
}

/// The literal block of `call` that whitequark would wrap as `block`.
fn plain_block<'pr>(call: &CallNode<'pr>, target_ruby: f32) -> Option<Node<'pr>> {
    call.block().filter(|b| is_plain_block(b, target_ruby))
}

/// How a call looks to a `NodePattern`: `send` (no block), `block` (plain
/// block) or neither (csend, numblock, ...), plus its argument count.
struct View<'pr> {
    call: CallNode<'pr>,
    has_block: bool,
    nargs: usize,
}

fn view<'pr>(node: &Node<'pr>, target_ruby: f32) -> Option<View<'pr>> {
    let call = node.as_call_node()?;
    if call.is_safe_navigation() {
        return None;
    }
    let (has_block, block_arg) = match call.block() {
        None => (false, false),
        Some(b) if b.as_block_argument_node().is_some() => (false, true),
        Some(b) if is_plain_block(&b, target_ruby) => (true, false),
        Some(_) => return None,
    };
    let nargs = call.arguments().map_or(0, |a| a.arguments().len()) + usize::from(block_arg);
    Some(View { call, has_block, nargs })
}

fn is_named_const(node: &Node<'_>, expected: &[u8]) -> bool {
    if !ext::is_bare_or_toplevel_const(node) {
        return false;
    }
    match node.kind() {
        NodeKind::ConstantReadNode => {
            node.as_constant_read_node().is_some_and(|n| n.name().as_slice() == expected)
        }
        _ => node
            .as_constant_path_node()
            .and_then(|path| path.name())
            .is_some_and(|id| id.as_slice() == expected),
    }
}

fn receiver_is(call: &CallNode<'_>, name: &[u8]) -> bool {
    call.receiver().is_some_and(|r| is_named_const(&r, name))
}

fn is_number(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::IntegerNode | NodeKind::FloatNode)
}

fn first_argument<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    call.arguments()?.arguments().iter().next()
}

/// `ENV[_]` (`(send (const {nil? cbase} :ENV) :[] _)`).
fn is_env_index(node: &Node<'_>, target_ruby: f32) -> bool {
    view(node, target_ruby).is_some_and(|v| {
        !v.has_block && v.call.name().as_slice() == b"[]" && v.nargs == 1 && receiver_is(&v.call, b"ENV")
    })
}

/// `operation_produces_immutable_object?`.
fn produces_immutable_object(node: &Node<'_>, target_ruby: f32) -> bool {
    match node.kind() {
        NodeKind::ConstantReadNode | NodeKind::ConstantPathNode => return true,
        NodeKind::OrNode => {
            let or = node.as_or_node().expect("kind matched");
            return is_env_index(&or.left(), target_ruby);
        }
        _ => {}
    }
    let Some(v) = view(node, target_ruby) else { return false };
    let name = v.call.name();
    let name = name.as_slice();
    if matches!(name, b"count" | b"length" | b"size") {
        return true;
    }
    if name == b"new" && receiver_is(&v.call, b"Struct") {
        return true;
    }
    if v.has_block {
        return false;
    }
    match name {
        b"freeze" if v.nargs == 0 => true,
        b"+" | b"-" | b"*" | b"**" | b"/" | b"%" | b"<<" => {
            let numeric_receiver = v.call.receiver().is_some_and(|r| is_number(&r));
            let numeric_arg = name != b"<<"
                && v.nargs == 1
                && first_argument(&v.call).is_some_and(|a| is_number(&a));
            (numeric_receiver && v.nargs == 1) || numeric_arg
        }
        b"==" | b"===" | b"!=" | b"<=" | b">=" | b"<" | b">" => v.nargs == 1,
        b"[]" => v.nargs == 1 && receiver_is(&v.call, b"ENV"),
        _ => false,
    }
}

/// Root name and the segment names after it of a constant chain.
fn const_chain(node: &Node<'_>) -> Option<(Vec<u8>, Vec<Vec<u8>>)> {
    let mut segments = Vec::new();
    let mut current = *node;
    loop {
        match current.kind() {
            NodeKind::ConstantReadNode => {
                let name = current.as_constant_read_node()?.name().as_slice().to_vec();
                segments.reverse();
                return Some((name, segments));
            }
            NodeKind::ConstantPathNode => {
                let path = current.as_constant_path_node()?;
                let name = path.name()?.as_slice().to_vec();
                if let Some(parent) = path.parent() {
                    segments.push(name);
                    current = parent;
                } else {
                    segments.reverse();
                    return Some((name, segments));
                }
            }
            _ => return None,
        }
    }
}

/// `operation_produces_threadsafe_object?` (`OperationWithThreadsafeResult`).
fn produces_threadsafe_object(node: &Node<'_>, target_ruby: f32) -> bool {
    let Some(v) = view(node, target_ruby) else { return false };
    if v.call.name().as_slice() != b"new" {
        return false;
    }
    let Some(receiver) = v.call.receiver() else { return false };
    let Some((root, segments)) = const_chain(&receiver) else { return false };
    match root.as_slice() {
        b"Queue" => !v.has_block && segments.is_empty(),
        b"ThreadSafe" => segments.len() == 1 && matches!(segments[0].as_slice(), b"Hash" | b"Array"),
        b"Concurrent" => (1..=3).contains(&segments.len()),
        _ => false,
    }
}

/// `immutable_literal?` for non-nil nodes.
fn is_immutable_literal(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::SymbolNode
            | NodeKind::InterpolatedSymbolNode
            | NodeKind::TrueNode
            | NodeKind::FalseNode
            | NodeKind::NilNode
            | NodeKind::ImaginaryNode
            | NodeKind::RationalNode
            | NodeKind::SourceLineNode
    )
}

/// `mutable_literal?(node) || range_type?`: `str dstr xstr array hash regexp
/// irange erange`.
fn is_mutable_literal(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::StringNode
            | NodeKind::SourceFileNode
            | NodeKind::InterpolatedStringNode
            | NodeKind::XStringNode
            | NodeKind::InterpolatedXStringNode
            | NodeKind::ArrayNode
            | NodeKind::HashNode
            | NodeKind::RegularExpressionNode
            | NodeKind::InterpolatedRegularExpressionNode
            | NodeKind::RangeNode
    )
}

/// `range_enclosed_in_parentheses?`: `(begin (range _ _))`.
fn is_range_enclosed_in_parentheses(node: &Node<'_>) -> bool {
    let Some(parens) = node.as_parentheses_node() else { return false };
    let Some(body) = parens.body() else { return false };
    let Some(stmts) = body.as_statements_node() else { return false };
    let list = stmts.body();
    list.len() == 1 && list.iter().next().is_some_and(|s| s.kind() == NodeKind::RangeNode)
}

/// `(array (splat $_))`.
fn splat_value<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    let elements = node.as_array_node()?.elements();
    if elements.len() != 1 {
        return None;
    }
    elements.iter().next()?.as_splat_node()?.expression()
}

/// The span RuboCop reports: a heredoc's range is only its opening token.
fn value_span(value: &Node<'_>, ctx: &Context<'_>) -> Span {
    let opening = match value.kind() {
        NodeKind::StringNode => value.as_string_node().expect("kind matched").opening_loc(),
        NodeKind::InterpolatedStringNode => {
            value.as_interpolated_string_node().expect("kind matched").opening_loc()
        }
        _ => None,
    };
    if let Some(opening) = opening {
        let span = opening.span();
        if ctx.text(span).starts_with(b"<<") {
            return span;
        }
    }
    value.span()
}

impl MutableClassInstanceVariable {
    /// `frozen_string_literal?`.
    fn frozen_string_literal(&self, node: &Node<'_>, ctx: &Context<'_>) -> bool {
        let literal = if self.target_ruby >= 3.0 {
            matches!(node.kind(), NodeKind::StringNode | NodeKind::SourceFileNode)
        } else {
            matches!(
                node.kind(),
                NodeKind::StringNode
                    | NodeKind::SourceFileNode
                    | NodeKind::InterpolatedStringNode
            )
        };
        literal && ctx.parsed().frozen_string_literals()
    }

    /// `in_class?`: the nearest container is a class or module.
    fn in_class(&self) -> bool {
        for frame in self.frames.iter().rev() {
            match frame {
                Frame::Class(_) | Frame::Module => return true,
                Frame::Def => return false,
                Frame::Block { name, nil_receiver: true }
                    if name == b"define_method" || name == b"define_singleton_method" =>
                {
                    return false;
                }
                Frame::Block { .. } => {}
            }
        }
        false
    }

    /// `within_dsl_with_threadsafe_semantics?` (the value is never nil here).
    fn within_dsl_with_threadsafe_semantics(&self) -> bool {
        let Some(index) = self.frames.iter().rposition(|f| matches!(f, Frame::Block { .. }))
        else {
            return false;
        };
        let Frame::Block { name, .. } = &self.frames[index] else { return false };
        if name != b"setup" && name != b"teardown" {
            return false;
        }
        self.frames[..index].iter().rev().find_map(|f| match f {
            Frame::Class(superclass) => Some(superclass.as_deref() == Some("ActiveSupport::TestCase")),
            _ => None,
        }) == Some(true)
    }

    fn on_assignment(&self, value: &Node<'_>, ctx: &mut Context<'_>) {
        if self.within_dsl_with_threadsafe_semantics() {
            return;
        }
        let offending = if self.style == EnforcedStyle::Strict {
            !(is_immutable_literal(value)
                || produces_immutable_object(value, self.target_ruby)
                || produces_threadsafe_object(value, self.target_ruby)
                || self.frozen_string_literal(value, ctx))
        } else {
            (is_mutable_literal(value) || is_range_enclosed_in_parentheses(value))
                && !self.frozen_string_literal(value, ctx)
        };
        if !offending {
            return;
        }
        let span = value_span(value, ctx);
        let fix = Fix { applicability: Applicability::Unsafe, edits: self.autocorrect(value, ctx) };
        ctx.report_with_fix(&Self::META, span, MSG, fix);
    }

    fn requires_parentheses(&self, node: &Node<'_>) -> bool {
        if node.kind() == NodeKind::RangeNode {
            return true;
        }
        let Some(call) = node.as_call_node() else { return false };
        if call.is_safe_navigation() {
            return false;
        }
        match call.block() {
            Some(b) if is_plain_block(&b, self.target_ruby) => return false,
            Some(b) if b.as_block_node().is_some() => return false,
            _ => {}
        }
        let has_args = call.arguments().is_some() || call.block().is_some();
        call.call_operator_loc().is_none() || (has_args && call.opening_loc().is_none())
    }

    fn autocorrect(&self, node: &Node<'_>, ctx: &Context<'_>) -> Vec<Edit> {
        let expr = value_span(node, ctx);
        let mut edits = Vec::with_capacity(3);
        if let Some(splat) = splat_value(node) {
            let source = String::from_utf8_lossy(ctx.text(splat.span()));
            let replacement = if is_range_enclosed_in_parentheses(&splat) {
                format!("{source}.to_a")
            } else {
                format!("({source}).to_a")
            };
            edits.push(Edit::replace(expr, replacement.into_bytes()));
        } else if node.as_array_node().is_some_and(|a| a.opening_loc().is_none()) {
            edits.push(Edit::insert(expr.start, b"[".to_vec()));
            edits.push(Edit::insert(expr.end, b"]".to_vec()));
        } else if self.requires_parentheses(node) {
            edits.push(Edit::insert(expr.start, b"(".to_vec()));
            edits.push(Edit::insert(expr.end, b")".to_vec()));
        }
        edits.push(Edit::insert(expr.end, b".freeze".to_vec()));
        edits
    }

    fn frame_for(&self, node: &Node<'_>) -> Option<Frame> {
        match node.kind() {
            NodeKind::ClassNode => {
                let class = node.as_class_node().expect("kind matched");
                Some(Frame::Class(class.superclass().and_then(|s| ext::const_name(&s))))
            }
            NodeKind::ModuleNode => Some(Frame::Module),
            NodeKind::DefNode => Some(Frame::Def),
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                plain_block(&call, self.target_ruby)?;
                Some(Frame::Block {
                    name: call.name().as_slice().to_vec(),
                    nil_receiver: call.receiver().is_none(),
                })
            }
            NodeKind::LambdaNode => {
                let lambda = node.as_lambda_node().expect("kind matched");
                if lambda.parameters().is_some_and(|p| numbered_or_it(&p, self.target_ruby)) {
                    return None;
                }
                Some(Frame::Block { name: b"lambda".to_vec(), nil_receiver: true })
            }
            _ => None,
        }
    }
}

impl Rule for MutableClassInstanceVariable {
    const META: RuleMeta = RuleMeta {
        name: "ThreadSafety/MutableClassInstanceVariable",
        department: Department::ThreadSafety,
        summary: "Do not assign mutable objects to class instance variables.",
        explanation: "\
Checks whether some class instance variable isn't a mutable literal (e.g.
array or hash).

It is based on Style/MutableConstant from RuboCop.

Class instance variables are a risk to threaded code as they are shared
between threads. A mutable object such as an array or hash may be updated via
an attr_reader so would not be detected by the
ThreadSafety/ClassAndModuleAttributes cop.

Strict mode can be used to freeze all class instance variables, rather than
just literals. Strict mode is considered an experimental feature.

```ruby
# bad
class Model
  @list = [1, 2, 3]
end

# good
class Model
  @list = [1, 2, 3].freeze
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[
            NodeKind::ClassNode,
            NodeKind::ModuleNode,
            NodeKind::DefNode,
            NodeKind::CallNode,
            NodeKind::LambdaNode,
            NodeKind::InstanceVariableWriteNode,
            NodeKind::InstanceVariableOrWriteNode,
            NodeKind::MultiWriteNode,
        ],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("literals"),
            allowed: &["literals", "strict"],
            doc: "`literals` freezes literals assigned to class instance variables; \
`strict` freezes every assigned value.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "strict" => EnforcedStyle::Strict,
            _ => EnforcedStyle::Literals,
        };
        Ok(Self { style, target_ruby: options.target_ruby_version(), frames: Vec::new() })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.frames.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(frame) = self.frame_for(node) {
            self.frames.push(frame);
            return;
        }
        match node.kind() {
            NodeKind::InstanceVariableWriteNode => {
                if self.in_class() {
                    let value = node.as_instance_variable_write_node().expect("kind matched").value();
                    self.on_assignment(&value, ctx);
                }
            }
            NodeKind::InstanceVariableOrWriteNode => {
                if self.in_class() {
                    let value =
                        node.as_instance_variable_or_write_node().expect("kind matched").value();
                    self.on_assignment(&value, ctx);
                }
            }
            NodeKind::MultiWriteNode => {
                if !self.in_class() {
                    return;
                }
                let multi = node.as_multi_write_node().expect("kind matched");
                let Some(values) = multi.value().as_array_node() else { return };
                let mut targets: Vec<Node<'_>> = multi.lefts().iter().collect();
                targets.extend(multi.rest().filter(|r| r.kind() == NodeKind::SplatNode));
                targets.extend(multi.rights().iter());
                let mut elements = values.elements().iter();
                for target in targets {
                    let value = elements.next();
                    if target.kind() != NodeKind::InstanceVariableTargetNode {
                        continue;
                    }
                    if let Some(value) = value {
                        self.on_assignment(&value, ctx);
                    }
                }
            }
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if self.frame_for(node).is_some() {
            self.frames.pop();
        }
    }
}
