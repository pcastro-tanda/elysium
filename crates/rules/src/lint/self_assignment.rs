//! `Lint/SelfAssignment`, ported from RuboCop's
//! `lib/rubocop/cop/lint/self_assignment.rb`.
//!
//! # Node shapes
//!
//! Whitequark's `or_asgn`/`and_asgn` fire for *any* writable LHS (a plain
//! variable, an attribute, an index, a constant), but upstream's
//! `rhs_matches_lhs?` looks the LHS's node type up in
//! `ASSIGNMENT_TYPE_TO_RHS_TYPE`, which only has entries for
//! `lvasgn`/`ivasgn`/`cvasgn`/`gvasgn` -- so `foo.bar ||= foo.bar`,
//! `Foo ||= Foo`, and `foo[0] ||= foo[0]` can never be flagged (the lookup
//! misses and `rhs_matches_lhs?` returns `false`). Prism mirrors this split
//! at the node-kind level: only `Local`/`Instance`/`Class`/`Global`
//! `Or`/`AndWriteNode` exist as their own kinds (an attribute or index
//! `||=`/`&&=` is a distinct `CallOperatorWriteNode`/`IndexOrWriteNode`/...,
//! and a constant one is `ConstantOrWriteNode`), so subscribing to just
//! those eight kinds reproduces the same exemption for free -- no
//! `ASSIGNMENT_TYPE_TO_RHS_TYPE`-style table is needed here.
//!
//! Plain attribute/index assignment (`foo.bar = foo.bar`, `foo[k] = foo[k]`)
//! is, in Prism, an ordinary `CallNode` named `bar=`/`[]=` whose sole
//! argument (for `bar=`) or last argument (for `[]=`, the keys coming
//! first) *is* the RHS value -- matching whitequark's `send`-node shape
//! closely enough that [`attribute_assignment_is_self`]/
//! [`key_assignment_is_self`] port `handle_attribute_assignment`/
//! `handle_key_assignment` almost line for line.
//!
//! `node.receiver == value_node.receiver` and `node_arguments ==
//! value_node.arguments` are RuboCop's generic `Parser::AST::Node#==`
//! (type-and-children structural equality); this port approximates both
//! with exact source-text comparison instead, matching this codebase's
//! established approximation for the same problem in
//! `Style/RedundantCondition`. Every fixture receiver/key is a bare
//! identifier or literal, so this is exact for everything tested; a
//! genuinely equal expression written with different incidental
//! formatting (extra parens, different whitespace) would be treated as
//! unequal.
//!
//! # `AllowRBSInlineAnnotation`
//!
//! Upstream checks a *specific* node per assignment shape (the RHS for
//! `lvasgn`/etc. and `casgn`, the first `mlhs` target for `masgn`, the LHS
//! target for `or_asgn`/`and_asgn`, the receiver for the `send`-based
//! shapes) for a trailing `#: Type` comment, via `processed_source
//! .ast_with_comments`. That association algorithm (`parser` gem's
//! `Comment::Associator`) is a single left-to-right scan that assigns each
//! same-line trailing comment to the *first* leaf node it reaches walking
//! into the first child of the first child of ... -- which, for every
//! shape above, is provably exactly the specific node upstream checks.
//! Since every fixture (and every real use of this annotation style) is a
//! single-line statement, every node in it shares one line number, so this
//! port skips reconstructing that per-shape node and just asks "is there a
//! `#:`-comment on this whole statement's own line" -- equivalent for a
//! single-line statement, and a documented blind spot for a hypothetical
//! multi-line one.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CallNode, MultiWriteNode};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Self-assignment detected.";

/// Checks for self-assignments.
#[derive(Debug, Clone)]
pub struct SelfAssignment {
    /// `AllowRBSInlineAnnotation`.
    allow_rbs_inline_annotation: bool,
}

impl SelfAssignment {
    /// See the module doc: a `#:`-comment on `span`'s own line.
    fn has_rbs_annotation(&self, ctx: &Context<'_>, span: Span) -> bool {
        if !self.allow_rbs_inline_annotation {
            return false;
        }
        let line = ctx.line_col(span.end).line;
        ctx.comments()
            .iter()
            .any(|comment| comment.line == line && ctx.text(comment.span).starts_with(b"#:"))
    }

    /// RuboCop's `on_lvasgn`/`on_ivasgn`/`on_cvasgn`/`on_gvasgn` and the
    /// `or_asgn`/`and_asgn` variants sharing the same shape: `value` must be
    /// a same-named read of the kind `expected` matches.
    fn check_simple(
        &self,
        ctx: &mut Context<'_>,
        node: &Node<'_>,
        name: &[u8],
        value: Node<'_>,
        expected: NodeKind,
    ) {
        if self.has_rbs_annotation(ctx, node.span()) {
            return;
        }
        if same_variable_read(&value, expected, name) {
            ctx.report(&Self::META, node.span(), MSG);
        }
    }

    /// RuboCop's `on_masgn`.
    fn check_masgn(&self, ctx: &mut Context<'_>, node: &MultiWriteNode<'_>) {
        let span = node.as_node().span();
        if self.has_rbs_annotation(ctx, span) {
            return;
        }
        if masgn_is_self_assignment(node) {
            ctx.report(&Self::META, span, MSG);
        }
    }

    /// RuboCop's `on_send`: routes to `handle_key_assignment` for `[]=` or
    /// `handle_attribute_assignment` for any other assignment method.
    fn check_call(&self, ctx: &mut Context<'_>, call: &CallNode<'_>) {
        let span = call.as_node().span();
        if self.has_rbs_annotation(ctx, span) {
            return;
        }
        let name = call.name();
        let name_bytes = name.as_slice();
        let is_self = if name_bytes == b"[]=" {
            key_assignment_is_self(call, ctx)
        } else if is_assignment_method(name_bytes) {
            attribute_assignment_is_self(call, ctx)
        } else {
            false
        };
        if is_self {
            ctx.report(&Self::META, span, MSG);
        }
    }
}

/// RuboCop's `ASSIGNMENT_TYPE_TO_RHS_TYPE` lookup applied to a read node:
/// `value` must have kind `expected` and the same variable name as `name`.
fn same_variable_read(value: &Node<'_>, expected: NodeKind, name: &[u8]) -> bool {
    if value.kind() != expected {
        return false;
    }
    let value_name = match expected {
        NodeKind::LocalVariableReadNode => value.as_local_variable_read_node().map(|n| n.name()),
        NodeKind::InstanceVariableReadNode => {
            value.as_instance_variable_read_node().map(|n| n.name())
        }
        NodeKind::ClassVariableReadNode => value.as_class_variable_read_node().map(|n| n.name()),
        NodeKind::GlobalVariableReadNode => value.as_global_variable_read_node().map(|n| n.name()),
        _ => None,
    };
    value_name.is_some_and(|found| found.as_slice() == name)
}

/// RuboCop's `rhs_matches_lhs?`, applied to one `masgn` target/element pair.
fn target_matches_value(target: &Node<'_>, value: &Node<'_>) -> bool {
    match target.kind() {
        NodeKind::LocalVariableTargetNode => {
            target.as_local_variable_target_node().is_some_and(|t| {
                same_variable_read(value, NodeKind::LocalVariableReadNode, t.name().as_slice())
            })
        }
        NodeKind::InstanceVariableTargetNode => {
            target.as_instance_variable_target_node().is_some_and(|t| {
                same_variable_read(value, NodeKind::InstanceVariableReadNode, t.name().as_slice())
            })
        }
        NodeKind::ClassVariableTargetNode => {
            target.as_class_variable_target_node().is_some_and(|t| {
                same_variable_read(value, NodeKind::ClassVariableReadNode, t.name().as_slice())
            })
        }
        NodeKind::GlobalVariableTargetNode => {
            target.as_global_variable_target_node().is_some_and(|t| {
                same_variable_read(value, NodeKind::GlobalVariableReadNode, t.name().as_slice())
            })
        }
        _ => false,
    }
}

/// RuboCop's `multiple_self_assignment?`: the RHS must be an array literal
/// (bracketed or a bare comma list; never a single non-array value or a
/// splat) with exactly as many elements as the LHS has targets, each
/// pairwise matching.
fn masgn_is_self_assignment(node: &MultiWriteNode<'_>) -> bool {
    let value = node.value();
    let Some(array) = value.as_array_node() else { return false };
    let rhs: Vec<Node<'_>> = array.elements().iter().collect();

    let mut targets: Vec<Node<'_>> = node.lefts().iter().collect();
    if let Some(rest) = node.rest() {
        targets.push(rest);
    }
    targets.extend(node.rights().iter());

    targets.len() == rhs.len()
        && targets.iter().zip(rhs.iter()).all(|(t, r)| target_matches_value(t, r))
}

/// RuboCop-AST's `assignment_method?`, restricted to what can actually reach
/// [`SelfAssignment::check_call`] (a name ending in `=`; `[]=` is routed to
/// `handle_key_assignment` before this is ever consulted).
fn is_assignment_method(name: &[u8]) -> bool {
    name.ends_with(b"=") && !matches!(name, b"==" | b"===" | b"!=" | b"<=" | b">=")
}

/// RuboCop's `handle_key_assignment`.
fn key_assignment_is_self(call: &CallNode<'_>, ctx: &Context<'_>) -> bool {
    let Some(args) = call.arguments() else { return false };
    let arg_list = args.arguments();
    let Some(value_node) = arg_list.last() else { return false };
    let Some(value_call) = value_node.as_call_node() else { return false };
    if value_call.name().as_slice() != b"[]" {
        return false;
    }
    let (Some(receiver), Some(value_receiver)) = (call.receiver(), value_call.receiver()) else {
        return false;
    };
    if ctx.text(receiver.span()) != ctx.text(value_receiver.span()) {
        return false;
    }

    let key_count = arg_list.len() - 1;
    let keys: Vec<Node<'_>> = arg_list.iter().take(key_count).collect();
    if keys.iter().any(|key| key.kind() == NodeKind::CallNode) {
        return false;
    }

    let value_args: Vec<Node<'_>> =
        value_call.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
    keys.len() == value_args.len()
        && keys.iter().zip(value_args.iter()).all(|(k, v)| ctx.text(k.span()) == ctx.text(v.span()))
}

/// RuboCop's `handle_attribute_assignment`.
fn attribute_assignment_is_self(call: &CallNode<'_>, ctx: &Context<'_>) -> bool {
    let Some(args) = call.arguments() else { return false };
    let arg_list = args.arguments();
    if arg_list.len() != 1 {
        return false;
    }
    let Some(getter) = arg_list.first().and_then(|first| first.as_call_node()) else {
        return false;
    };
    if getter.arguments().is_some() {
        return false;
    }
    let (Some(receiver), Some(getter_receiver)) = (call.receiver(), getter.receiver()) else {
        return false;
    };
    if ctx.text(receiver.span()) != ctx.text(getter_receiver.span()) {
        return false;
    }

    let setter = call.name();
    let Some(expected) = setter.as_slice().strip_suffix(b"=") else { return false };
    getter.name().as_slice() == expected
}

/// One level of a constant node's own namespace: `None` for a bare `Foo`
/// (whitequark's `nil` namespace), `Cbase` for a leading `::` (Prism's
/// `ConstantPathNode` with no `parent`, whitequark's `(cbase)` -- a real
/// namespace value, distinct from no namespace at all despite both being
/// "parent-less" in Prism's own representation), or the actual namespace
/// expression for `Foo::Bar`/`::Foo::Bar`.
enum Namespace<'pr> {
    None,
    Cbase,
    Node(Node<'pr>),
}

/// A constant write target's or constant read value's `(namespace,
/// short_name)`, mirroring rubocop-ast's `ConstantNode` mixin applied
/// generically to `casgn` targets by `Node#const_name`'s siblings
/// `namespace`/`short_name`.
fn constant_parts<'pr>(node: &Node<'pr>) -> Option<(Namespace<'pr>, &'pr [u8])> {
    match node.kind() {
        NodeKind::ConstantWriteNode => {
            let write = node.as_constant_write_node()?;
            Some((Namespace::None, write.name().as_slice()))
        }
        NodeKind::ConstantPathWriteNode => {
            let write = node.as_constant_path_write_node()?;
            let target = write.target();
            let namespace = target.parent().map_or(Namespace::Cbase, Namespace::Node);
            Some((namespace, target.name()?.as_slice()))
        }
        NodeKind::ConstantReadNode => {
            let read = node.as_constant_read_node()?;
            Some((Namespace::None, read.name().as_slice()))
        }
        NodeKind::ConstantPathNode => {
            let path = node.as_constant_path_node()?;
            let namespace = path.parent().map_or(Namespace::Cbase, Namespace::Node);
            Some((namespace, path.name()?.as_slice()))
        }
        _ => None,
    }
}

/// Structural equality restricted to constant-path namespace chains
/// (RuboCop's generic `Node#==`, applied only to `namespace`s here): `None`
/// only matches `None`, `Cbase` only matches `Cbase`, and two `Node`s match
/// when their own parts (recursively) match -- unlike
/// [`ruby_ast::ext::const_name`], which flattens `None`/`Cbase` to the same
/// string and is not precise enough for this comparison.
fn same_constant_namespace(a: Namespace<'_>, b: Namespace<'_>) -> bool {
    match (a, b) {
        (Namespace::None, Namespace::None) | (Namespace::Cbase, Namespace::Cbase) => true,
        (Namespace::Node(a), Namespace::Node(b)) => {
            match (constant_parts(&a), constant_parts(&b)) {
                (Some((ns_a, name_a)), Some((ns_b, name_b))) => {
                    name_a == name_b && same_constant_namespace(ns_a, ns_b)
                }
                _ => false,
            }
        }
        _ => false,
    }
}

/// RuboCop's `on_casgn`.
fn casgn_is_self_assignment(node: &Node<'_>) -> bool {
    let value = match node.kind() {
        NodeKind::ConstantWriteNode => node.as_constant_write_node().map(|w| w.value()),
        NodeKind::ConstantPathWriteNode => node.as_constant_path_write_node().map(|w| w.value()),
        _ => None,
    };
    let Some(value) = value else { return false };
    if !matches!(value.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode) {
        return false;
    }
    let (Some((lhs_ns, lhs_name)), Some((rhs_ns, rhs_name))) =
        (constant_parts(node), constant_parts(&value))
    else {
        return false;
    };
    lhs_name == rhs_name && same_constant_namespace(lhs_ns, rhs_ns)
}

impl Rule for SelfAssignment {
    const META: RuleMeta = RuleMeta {
        name: "Lint/SelfAssignment",
        department: Department::Lint,
        summary: "Checks for self-assignments.",
        explanation: "\
Checks for self-assignments.

```ruby
# bad
foo = foo
foo, bar = foo, bar
Foo = Foo
hash['foo'] = hash['foo']
obj.attr = obj.attr

# good
foo = bar
foo, bar = bar, foo
Foo = Bar
hash['foo'] = hash['bar']
obj.attr = obj.attr2

# good (method calls possibly can return different results)
hash[foo] = hash[foo]
```

With `AllowRBSInlineAnnotation: true` (default: `false`):

```ruby
# good
foo = foo #: Integer
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::LocalVariableWriteNode,
            NodeKind::InstanceVariableWriteNode,
            NodeKind::ClassVariableWriteNode,
            NodeKind::GlobalVariableWriteNode,
            NodeKind::LocalVariableOrWriteNode,
            NodeKind::LocalVariableAndWriteNode,
            NodeKind::InstanceVariableOrWriteNode,
            NodeKind::InstanceVariableAndWriteNode,
            NodeKind::ClassVariableOrWriteNode,
            NodeKind::ClassVariableAndWriteNode,
            NodeKind::GlobalVariableOrWriteNode,
            NodeKind::GlobalVariableAndWriteNode,
            NodeKind::ConstantWriteNode,
            NodeKind::ConstantPathWriteNode,
            NodeKind::MultiWriteNode,
            NodeKind::CallNode,
        ],
        config: &[ConfigOption {
            name: "AllowRBSInlineAnnotation",
            default: ConfigDefault::Bool(false),
            allowed: &[],
            doc: "Whether to allow a trailing `#: Type` RBS inline annotation to exempt an \
                  otherwise-flagged self-assignment.",
        }],
        blind_spots: "\
`node.receiver == value_node.receiver`/`node_arguments == value_node.arguments` (the key/attribute
assignment shapes) are approximated by exact source-text comparison rather than RuboCop's true
structural `Node#==`; a genuinely equal expression written with different incidental formatting is
treated as unequal. `AllowRBSInlineAnnotation` only recognizes the annotation on a single-line
statement (see the module doc); a multi-line self-assignment with a trailing annotation is not
exempted.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allow_rbs_inline_annotation: options.bool("AllowRBSInlineAnnotation") })
    }

    #[allow(clippy::too_many_lines)]
    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::LocalVariableWriteNode => {
                if let Some(w) = node.as_local_variable_write_node() {
                    self.check_simple(
                        ctx,
                        node,
                        w.name().as_slice(),
                        w.value(),
                        NodeKind::LocalVariableReadNode,
                    );
                }
            }
            NodeKind::InstanceVariableWriteNode => {
                if let Some(w) = node.as_instance_variable_write_node() {
                    self.check_simple(
                        ctx,
                        node,
                        w.name().as_slice(),
                        w.value(),
                        NodeKind::InstanceVariableReadNode,
                    );
                }
            }
            NodeKind::ClassVariableWriteNode => {
                if let Some(w) = node.as_class_variable_write_node() {
                    self.check_simple(
                        ctx,
                        node,
                        w.name().as_slice(),
                        w.value(),
                        NodeKind::ClassVariableReadNode,
                    );
                }
            }
            NodeKind::GlobalVariableWriteNode => {
                if let Some(w) = node.as_global_variable_write_node() {
                    self.check_simple(
                        ctx,
                        node,
                        w.name().as_slice(),
                        w.value(),
                        NodeKind::GlobalVariableReadNode,
                    );
                }
            }
            NodeKind::LocalVariableOrWriteNode => {
                if let Some(w) = node.as_local_variable_or_write_node() {
                    self.check_simple(
                        ctx,
                        node,
                        w.name().as_slice(),
                        w.value(),
                        NodeKind::LocalVariableReadNode,
                    );
                }
            }
            NodeKind::LocalVariableAndWriteNode => {
                if let Some(w) = node.as_local_variable_and_write_node() {
                    self.check_simple(
                        ctx,
                        node,
                        w.name().as_slice(),
                        w.value(),
                        NodeKind::LocalVariableReadNode,
                    );
                }
            }
            NodeKind::InstanceVariableOrWriteNode => {
                if let Some(w) = node.as_instance_variable_or_write_node() {
                    self.check_simple(
                        ctx,
                        node,
                        w.name().as_slice(),
                        w.value(),
                        NodeKind::InstanceVariableReadNode,
                    );
                }
            }
            NodeKind::InstanceVariableAndWriteNode => {
                if let Some(w) = node.as_instance_variable_and_write_node() {
                    self.check_simple(
                        ctx,
                        node,
                        w.name().as_slice(),
                        w.value(),
                        NodeKind::InstanceVariableReadNode,
                    );
                }
            }
            NodeKind::ClassVariableOrWriteNode => {
                if let Some(w) = node.as_class_variable_or_write_node() {
                    self.check_simple(
                        ctx,
                        node,
                        w.name().as_slice(),
                        w.value(),
                        NodeKind::ClassVariableReadNode,
                    );
                }
            }
            NodeKind::ClassVariableAndWriteNode => {
                if let Some(w) = node.as_class_variable_and_write_node() {
                    self.check_simple(
                        ctx,
                        node,
                        w.name().as_slice(),
                        w.value(),
                        NodeKind::ClassVariableReadNode,
                    );
                }
            }
            NodeKind::GlobalVariableOrWriteNode => {
                if let Some(w) = node.as_global_variable_or_write_node() {
                    self.check_simple(
                        ctx,
                        node,
                        w.name().as_slice(),
                        w.value(),
                        NodeKind::GlobalVariableReadNode,
                    );
                }
            }
            NodeKind::GlobalVariableAndWriteNode => {
                if let Some(w) = node.as_global_variable_and_write_node() {
                    self.check_simple(
                        ctx,
                        node,
                        w.name().as_slice(),
                        w.value(),
                        NodeKind::GlobalVariableReadNode,
                    );
                }
            }
            NodeKind::ConstantWriteNode | NodeKind::ConstantPathWriteNode => {
                if self.has_rbs_annotation(ctx, node.span()) {
                    return;
                }
                if casgn_is_self_assignment(node) {
                    ctx.report(&Self::META, node.span(), MSG);
                }
            }
            NodeKind::MultiWriteNode => {
                if let Some(m) = node.as_multi_write_node() {
                    self.check_masgn(ctx, &m);
                }
            }
            NodeKind::CallNode => {
                if let Some(call) = node.as_call_node() {
                    self.check_call(ctx, &call);
                }
            }
            _ => {}
        }
    }
}
