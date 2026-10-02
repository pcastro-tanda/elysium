//! `Style/FetchEnvVar`, ported from RuboCop's
//! `lib/rubocop/cop/style/fetch_env_var.rb`.
//!
//! Upstream's exclusions (`used_as_flag?`, `used_if_condition_in_body?`,
//! `used_in_condition?`, `or_lhs?`) all compare `rubocop-ast` `Node`
//! objects for structural equality (`==`, which is a recursive
//! type-and-children comparison ignoring source location) or read a
//! `send` node's `child_nodes` (its receiver and arguments, skipping the
//! non-node method-name symbol). Since every case these checks actually
//! fire on compares two sub-expressions that are byte-for-byte identical
//! source text (the same `ENV['X']`, or a key argument reappearing
//! verbatim), this port approximates "structurally equal" as "same source
//! text" ([`text_eq`]) rather than reimplementing a general AST-equality
//! walk.
//!
//! `||=`/`&&=`/plain `=` on an indexed receiver (`ENV['X'] ||= y`) never
//! reaches this cop as a `[]`-named `CallNode` at all in Prism -- it is a
//! distinct `IndexOrWriteNode`/`IndexAndWriteNode`/`IndexWriteNode` -- so
//! upstream's `assigned?` exclusion (guarding against exactly that shape,
//! which whitequark represents as a plain `(send ... :[] ...)` reused as
//! an assignment target) is structurally unreachable here and is not
//! ported.
//!
//! `used_in_condition?`'s fallback `condition.child_nodes.any?(node)` (an
//! `if`/`unless` condition that isn't itself a `send`, e.g. `if hosts =
//! ENV['X']`) only ever matters for *simple* variable/constant
//! assignments here: Prism gives each of those ([`write_value`]'s kinds)
//! a flat node exposing `.value()` directly, unlike whitequark's nested
//! `lvasgn`/`ivasgn`/etc., but the upstream `child_nodes` check can only
//! ever match the *value* slot regardless -- a compound form's other slot
//! (whitequark's valueless inner target node, no Prism equivalent at all)
//! could never equal the `ENV[]` call being checked, so tracking just the
//! value span reproduces the fallback exactly for every write kind.
//!
//! Every `CallNode`/`IfNode`/`UnlessNode`/`AndNode`/`OrNode`/write-node
//! visited is recorded into per-kind fact maps keyed by span as the
//! pre-order walk reaches it, so that by the time a `ENV[key]` leaf is
//! checked, every enclosing node's shape is already available without
//! re-walking from the root.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    NodeInfo, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::const_name;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;
use std::collections::HashMap;

const MSG_WITH_NIL: &str = "Use `ENV.fetch(%<key>s, nil)` instead of `ENV[%<key>s]`.";
const MSG_WITHOUT_NIL: &str = "Use `ENV.fetch(%<key>s)` instead of `ENV[%<key>s]`.";

#[derive(Debug, Clone, Default)]
struct CallFact {
    method: Vec<u8>,
    receiver: Option<Span>,
    args: Vec<Span>,
    operator: Option<Span>,
    safe_nav: bool,
}

/// A simple variable/constant assignment's own value (plain `=` and the
/// compound `||=`/`&&=`/op-assign forms) -- see the module doc's note on
/// why only the value slot matters.
fn write_value<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    match node.kind() {
        NodeKind::LocalVariableWriteNode => Some(node.as_local_variable_write_node()?.value()),
        NodeKind::LocalVariableOrWriteNode => Some(node.as_local_variable_or_write_node()?.value()),
        NodeKind::LocalVariableAndWriteNode => {
            Some(node.as_local_variable_and_write_node()?.value())
        }
        NodeKind::LocalVariableOperatorWriteNode => {
            Some(node.as_local_variable_operator_write_node()?.value())
        }
        NodeKind::InstanceVariableWriteNode => {
            Some(node.as_instance_variable_write_node()?.value())
        }
        NodeKind::InstanceVariableOrWriteNode => {
            Some(node.as_instance_variable_or_write_node()?.value())
        }
        NodeKind::InstanceVariableAndWriteNode => {
            Some(node.as_instance_variable_and_write_node()?.value())
        }
        NodeKind::InstanceVariableOperatorWriteNode => {
            Some(node.as_instance_variable_operator_write_node()?.value())
        }
        NodeKind::ClassVariableWriteNode => Some(node.as_class_variable_write_node()?.value()),
        NodeKind::ClassVariableOrWriteNode => Some(node.as_class_variable_or_write_node()?.value()),
        NodeKind::ClassVariableAndWriteNode => {
            Some(node.as_class_variable_and_write_node()?.value())
        }
        NodeKind::ClassVariableOperatorWriteNode => {
            Some(node.as_class_variable_operator_write_node()?.value())
        }
        NodeKind::GlobalVariableWriteNode => Some(node.as_global_variable_write_node()?.value()),
        NodeKind::GlobalVariableOrWriteNode => {
            Some(node.as_global_variable_or_write_node()?.value())
        }
        NodeKind::GlobalVariableAndWriteNode => {
            Some(node.as_global_variable_and_write_node()?.value())
        }
        NodeKind::GlobalVariableOperatorWriteNode => {
            Some(node.as_global_variable_operator_write_node()?.value())
        }
        NodeKind::ConstantWriteNode => Some(node.as_constant_write_node()?.value()),
        NodeKind::ConstantOrWriteNode => Some(node.as_constant_or_write_node()?.value()),
        NodeKind::ConstantAndWriteNode => Some(node.as_constant_and_write_node()?.value()),
        NodeKind::ConstantOperatorWriteNode => {
            Some(node.as_constant_operator_write_node()?.value())
        }
        NodeKind::ConstantPathWriteNode => Some(node.as_constant_path_write_node()?.value()),
        NodeKind::ConstantPathOrWriteNode => Some(node.as_constant_path_or_write_node()?.value()),
        NodeKind::ConstantPathAndWriteNode => Some(node.as_constant_path_and_write_node()?.value()),
        NodeKind::ConstantPathOperatorWriteNode => {
            Some(node.as_constant_path_operator_write_node()?.value())
        }
        _ => None,
    }
}

/// Suggests `ENV.fetch` for the replacement of `ENV[]`.
#[derive(Debug, Clone, Default)]
pub struct FetchEnvVar {
    allowed_variables: Vec<String>,
    default_to_nil: bool,
    calls: HashMap<Span, CallFact>,
    /// `if`/`unless` node span -> predicate span.
    conditions: HashMap<Span, Span>,
    /// `and`/`or` node span -> (left, right).
    and_children: HashMap<Span, (Span, Span)>,
    or_children: HashMap<Span, (Span, Span)>,
    /// A simple variable/constant assignment's own span -> its value span.
    writes: HashMap<Span, Span>,
}

impl Rule for FetchEnvVar {
    const META: RuleMeta = RuleMeta {
        name: "Style/FetchEnvVar",
        department: Department::Style,
        summary: "Suggests `ENV.fetch` for the replacement of `ENV[]`.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::CallNode,
            NodeKind::IfNode,
            NodeKind::UnlessNode,
            NodeKind::AndNode,
            NodeKind::OrNode,
            NodeKind::LocalVariableWriteNode,
            NodeKind::LocalVariableOrWriteNode,
            NodeKind::LocalVariableAndWriteNode,
            NodeKind::LocalVariableOperatorWriteNode,
            NodeKind::InstanceVariableWriteNode,
            NodeKind::InstanceVariableOrWriteNode,
            NodeKind::InstanceVariableAndWriteNode,
            NodeKind::InstanceVariableOperatorWriteNode,
            NodeKind::ClassVariableWriteNode,
            NodeKind::ClassVariableOrWriteNode,
            NodeKind::ClassVariableAndWriteNode,
            NodeKind::ClassVariableOperatorWriteNode,
            NodeKind::GlobalVariableWriteNode,
            NodeKind::GlobalVariableOrWriteNode,
            NodeKind::GlobalVariableAndWriteNode,
            NodeKind::GlobalVariableOperatorWriteNode,
            NodeKind::ConstantWriteNode,
            NodeKind::ConstantOrWriteNode,
            NodeKind::ConstantAndWriteNode,
            NodeKind::ConstantOperatorWriteNode,
            NodeKind::ConstantPathWriteNode,
            NodeKind::ConstantPathOrWriteNode,
            NodeKind::ConstantPathAndWriteNode,
            NodeKind::ConstantPathOperatorWriteNode,
        ],
        config: &[
            ConfigOption {
                name: "AllowedVariables",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Environment variables to be excluded from the inspection.",
            },
            ConfigOption {
                name: "DefaultToNil",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "When `true`, autocorrects `ENV[\"key\"]` to `ENV.fetch(\"key\", nil)`. \
                      When `false`, autocorrects to `ENV.fetch(\"key\")`.",
            },
        ],
        blind_spots: "Structural-equality comparisons (`used_if_condition_in_body?`'s \
                      `child_nodes` checks) are approximated as same-source-text rather than a \
                      true AST-equality walk; a semantically identical but differently-written \
                      duplicate (e.g. different whitespace) would not be recognized as the same \
                      expression.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            allowed_variables: options.str_list("AllowedVariables"),
            default_to_nil: options.bool("DefaultToNil"),
            ..Self::default()
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::IfNode => {
                let n = node.as_if_node().expect("kind matched");
                self.conditions.insert(node.span(), n.predicate().span());
            }
            NodeKind::UnlessNode => {
                let n = node.as_unless_node().expect("kind matched");
                self.conditions.insert(node.span(), n.predicate().span());
            }
            NodeKind::AndNode => {
                let n = node.as_and_node().expect("kind matched");
                self.and_children.insert(node.span(), (n.left().span(), n.right().span()));
            }
            NodeKind::OrNode => {
                let n = node.as_or_node().expect("kind matched");
                self.or_children.insert(node.span(), (n.left().span(), n.right().span()));
            }
            NodeKind::CallNode => self.check_call(node, ctx),
            _ => {
                if let Some(value) = write_value(node) {
                    self.writes.insert(node.span(), value.span());
                }
            }
        }
    }
}

impl FetchEnvVar {
    fn check_call(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node().expect("kind matched");
        let arg_nodes: Vec<Node<'_>> =
            call.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
        let args: Vec<Span> = arg_nodes.iter().map(Node::span).collect();
        self.calls.insert(
            node.span(),
            CallFact {
                method: call.name().as_slice().to_vec(),
                receiver: call.receiver().map(|r| r.span()),
                args: args.clone(),
                operator: call.call_operator_loc().map(|l| l.span()),
                safe_nav: call.is_safe_navigation(),
            },
        );

        if call.name().as_slice() != b"[]" || call.is_safe_navigation() {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        if const_name(&receiver).as_deref() != Some("ENV") {
            return;
        }
        let [key_node] = arg_nodes.as_slice() else { return };
        let key_span = key_node.span();

        if self.allowed_var(key_node) {
            return;
        }
        if self.allowable_use(node.span(), ctx) {
            return;
        }

        let key_source = String::from_utf8_lossy(ctx.text(key_span)).into_owned();
        let template = if self.default_to_nil { MSG_WITH_NIL } else { MSG_WITHOUT_NIL };
        let message = template.replacen("%<key>s", &key_source, 2);
        let replacement = if self.default_to_nil {
            format!("ENV.fetch({key_source}, nil)")
        } else {
            format!("ENV.fetch({key_source})")
        };

        ctx.report_with_fix(
            &Self::META,
            node.span(),
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(node.span(), replacement.into_bytes())],
            },
        );
    }

    fn allowed_var(&self, key_node: &Node<'_>) -> bool {
        let Some(string) = key_node.as_string_node() else { return false };
        let value = string.unescaped();
        self.allowed_variables.iter().any(|v| v.as_bytes() == value)
    }

    fn allowable_use(&self, node_span: Span, ctx: &Context<'_>) -> bool {
        self.used_as_flag(node_span, ctx)
            || self.message_chained_with_dot(node_span, ctx)
            || self.or_lhs(node_span, ctx)
    }

    fn used_as_flag(&self, node_span: Span, ctx: &Context<'_>) -> bool {
        if ctx.ancestors().is_empty() {
            return false;
        }
        if self.used_if_condition_in_body(node_span, ctx) {
            return true;
        }
        let Some(parent) = enclosing_call(ctx.ancestors()) else { return false };
        let Some(fact) = self.calls.get(&parent.span) else { return false };
        (fact.method == b"!" && fact.args.is_empty()) || is_comparison_method(&fact.method)
    }

    fn used_if_condition_in_body(&self, node_span: Span, ctx: &Context<'_>) -> bool {
        let Some(if_info) = ctx
            .ancestors()
            .iter()
            .rev()
            .find(|a| matches!(a.kind, NodeKind::IfNode | NodeKind::UnlessNode))
        else {
            return false;
        };
        let Some(&condition_span) = self.conditions.get(&if_info.span) else { return false };
        if self.same_child_nodes(condition_span, node_span, ctx) {
            return true;
        }
        self.used_in_condition(condition_span, node_span, ctx)
    }

    fn used_in_condition(&self, condition_span: Span, node_span: Span, ctx: &Context<'_>) -> bool {
        if let Some(fact) = self.calls.get(&condition_span) {
            if is_assignment_method(&fact.method)
                && self.partial_matched(condition_span, node_span, ctx)
            {
                return true;
            }
            if !is_comparison_method(&fact.method) && !fact.method.ends_with(b"?") {
                return false;
            }
        }
        self.children_contain(condition_span, node_span, ctx)
    }

    /// Every child of the `node_span` call appears somewhere among
    /// `condition_span`'s own children (upstream's `node.child_nodes ==
    /// node.child_nodes & condition.child_nodes`).
    fn partial_matched(&self, condition_span: Span, node_span: Span, ctx: &Context<'_>) -> bool {
        let Some(node_fact) = self.calls.get(&node_span) else { return false };
        let node_children = call_children(node_fact);
        let condition_children =
            self.calls.get(&condition_span).map(call_children).unwrap_or_default();
        node_children
            .iter()
            .all(|child| condition_children.iter().any(|c| text_eq(ctx, *c, *child)))
    }

    fn children_contain(&self, condition_span: Span, node_span: Span, ctx: &Context<'_>) -> bool {
        let children: Vec<Span> = if let Some(fact) = self.calls.get(&condition_span) {
            call_children(fact)
        } else if let Some((l, r)) = self.and_children.get(&condition_span) {
            vec![*l, *r]
        } else if let Some((l, r)) = self.or_children.get(&condition_span) {
            vec![*l, *r]
        } else if let Some(value) = self.writes.get(&condition_span) {
            vec![*value]
        } else {
            Vec::new()
        };
        children.iter().any(|c| text_eq(ctx, *c, node_span))
    }

    fn same_child_nodes(&self, condition_span: Span, node_span: Span, ctx: &Context<'_>) -> bool {
        let node_children = self.calls.get(&node_span).map(call_children).unwrap_or_default();
        let condition_children =
            self.calls.get(&condition_span).map(call_children).unwrap_or_default();
        if node_children.len() != condition_children.len() {
            return false;
        }
        node_children.iter().zip(condition_children.iter()).all(|(a, b)| text_eq(ctx, *a, *b))
    }

    fn message_chained_with_dot(&self, node_span: Span, ctx: &Context<'_>) -> bool {
        let Some(parent) = ctx.parent() else { return false };
        if parent.kind != NodeKind::CallNode {
            return false;
        }
        let Some(fact) = self.calls.get(&parent.span) else { return false };
        if fact.receiver != Some(node_span) {
            return false;
        }
        fact.safe_nav || fact.operator.is_some_and(|op| ctx.text(op) == b".")
    }

    fn or_lhs(&self, node_span: Span, ctx: &Context<'_>) -> bool {
        let Some(parent) = ctx.parent() else { return false };
        if parent.kind != NodeKind::OrNode {
            return false;
        }
        let Some((left, _)) = self.or_children.get(&parent.span) else { return false };
        if *left == node_span {
            return true;
        }
        let ancestors = ctx.ancestors();
        ancestors.len() >= 2 && ancestors[ancestors.len() - 2].kind == NodeKind::OrNode
    }
}

fn call_children(fact: &CallFact) -> Vec<Span> {
    let mut out = Vec::with_capacity(fact.args.len() + 1);
    if let Some(r) = fact.receiver {
        out.push(r);
    }
    out.extend(fact.args.iter().copied());
    out
}

/// The `send`-equivalent call enclosing the node whose ancestor path is
/// `ancestors`: Prism wraps a call's argument list in its own
/// `ArgumentsNode` (unlike whitequark's flat `send` node, where receiver
/// and arguments alike are direct children), so a node in argument
/// position has `ArgumentsNode` as its immediate parent and the real call
/// as its grandparent, while a node in receiver position has the call
/// directly as its parent.
fn enclosing_call(ancestors: &[NodeInfo]) -> Option<NodeInfo> {
    let parent = *ancestors.last()?;
    if parent.kind == NodeKind::CallNode {
        return Some(parent);
    }
    if parent.kind == NodeKind::ArgumentsNode {
        let grandparent = *ancestors.get(ancestors.len() - 2)?;
        if grandparent.kind == NodeKind::CallNode {
            return Some(grandparent);
        }
    }
    None
}

fn text_eq(ctx: &Context<'_>, a: Span, b: Span) -> bool {
    ctx.text(a) == ctx.text(b)
}

/// RuboCop-AST's `assignment_method?`: a method name ending in `=` that is
/// not one of the comparison operators.
fn is_assignment_method(name: &[u8]) -> bool {
    name.ends_with(b"=") && !is_comparison_method(name)
}

/// RuboCop-AST's `Node::COMPARISON_OPERATORS`.
fn is_comparison_method(name: &[u8]) -> bool {
    matches!(name, b"==" | b"===" | b"!=" | b"<=" | b">=" | b">" | b"<")
}
