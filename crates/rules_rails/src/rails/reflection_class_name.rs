//! `Rails/ReflectionClassName`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/reflection_class_name.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{for_each_child, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Use a string value for `class_name`.";

/// Checks if the value of the option `class_name`, in the definition of a
/// reflection is a string.
#[derive(Debug, Clone)]
pub struct ReflectionClassName;

impl Rule for ReflectionClassName {
    const META: RuleMeta = RuleMeta {
        name: "Rails/ReflectionClassName",
        department: Department::Rails,
        summary: "Use a string for `class_name` option value in the definition of a reflection.",
        explanation: "Checks if the value of the option `class_name`, in the definition of a \
                      reflection is a string.\n\nThis cop is unsafe because it cannot be \
                      determined whether constant or method return value specified to \
                      `class_name` is a string.\n\n```ruby\n# bad\nhas_many :accounts, \
                      class_name: Account\nhas_many :accounts, class_name: Account.name\n\n\
                      # good\nhas_many :accounts, class_name: 'Account'\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.receiver().is_some()
            || !matches!(call.name().as_slice(), b"has_many" | b"has_one" | b"belongs_to")
            || call.block().is_some_and(|block| block.as_block_argument_node().is_some())
        {
            return;
        }
        // `_ _ ? (hash ...)`: name, optional scope, then the options hash.
        let Some(arguments) = call.arguments() else { return };
        let arguments: Vec<Node<'_>> = arguments.arguments().iter().collect();
        if !(2..=3).contains(&arguments.len()) {
            return;
        }
        let Some(last) = arguments.last() else { return };
        let elements = if let Some(hash) = last.as_hash_node() {
            hash.elements()
        } else if let Some(hash) = last.as_keyword_hash_node() {
            hash.elements()
        } else {
            return;
        };
        let Some((pair, value)) = elements.iter().find_map(|element| {
            let assoc = element.as_assoc_node()?;
            let key = assoc.key();
            let symbol = key.as_symbol_node()?;
            if symbol.unescaped() != b"class_name" {
                return None;
            }
            // Shorthand `class_name:` wraps its value in an `ImplicitNode`.
            let value = assoc.value();
            let value = match value.as_implicit_node() {
                Some(implicit) => implicit.value(),
                None => value,
            };
            reflection_class_value(&value).then(|| (element.span(), value))
        }) else {
            return;
        };

        // `value.send_type? && !value.receiver&.const_type?`.
        if let Some(value_call) = send_call(&value) {
            if !value_call.receiver().is_some_and(|r| is_const(&r)) {
                return;
            }
        }
        if let Some(local) = value.as_local_variable_read_node() {
            if str_assigned(&ctx.parsed().root(), pair, local.name().as_slice()) {
                return;
            }
        }

        let fix = autocorrect(&value, ctx).map(|edit| Fix {
            applicability: Applicability::Unsafe,
            edits: vec![edit],
        });
        match fix {
            Some(fix) => ctx.report_with_fix(&Self::META, pair, MSG, fix),
            None => ctx.report(&Self::META, pair, MSG),
        }
    }
}

fn is_const(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode)
}

/// The node as whitequark's `send` (not `csend`, not a `block`).
fn send_call<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    let call = node.as_call_node()?;
    let is_block = call.block().is_some_and(|block| block.as_block_node().is_some());
    (!call.is_safe_navigation() && !is_block).then_some(call)
}

/// `str`, `dstr` or `sym` (`ALLOWED_REFLECTION_CLASS_TYPES`).
fn is_allowed_type(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::StringNode
            | NodeKind::InterpolatedStringNode
            | NodeKind::SymbolNode
            | NodeKind::SourceFileNode
    )
}

/// `reflection_class_value?`.
fn reflection_class_value(value: &Node<'_>) -> bool {
    match send_call(value) {
        Some(call) => {
            call.name().as_slice() != b"to_s" || call.receiver().is_some_and(|r| is_const(&r))
        }
        None => !is_allowed_type(value),
    }
}

/// `const_or_string`: a bare constant, or `Const.name` / `Const.to_s`.
fn const_or_string(value: &Node<'_>) -> Option<Span> {
    if value.kind() == NodeKind::ConstantReadNode {
        return Some(value.span());
    }
    let call = send_call(value)?;
    if call.arguments().is_some() || !matches!(call.name().as_slice(), b"name" | b"to_s") {
        return None;
    }
    let receiver = call.receiver()?;
    (receiver.kind() == NodeKind::ConstantReadNode).then(|| receiver.span())
}

fn autocorrect(value: &Node<'_>, ctx: &Context<'_>) -> Option<Edit> {
    let replacement = const_or_string(value)?;
    let source = String::from_utf8_lossy(ctx.text(replacement));
    Some(Edit::replace(value.span(), format!("\"{source}\"").into_bytes()))
}

/// whitequark-style children of `node` (single-statement bodies and
/// parentheses do not introduce a `begin` of their own).
fn effective_children<'pr>(node: &Node<'pr>) -> Vec<Node<'pr>> {
    let mut out = Vec::new();
    if let Some(parens) = node.as_parentheses_node() {
        if let Some(body) = parens.body() {
            match body.as_statements_node() {
                Some(statements) => out.extend(statements.body().iter()),
                None => out.push(body),
            }
        }
        return out;
    }
    for_each_child(node, |child| match child.as_statements_node() {
        Some(statements) if statements.body().len() == 1 => out.extend(statements.body().iter()),
        _ => out.push(child.clone()),
    });
    out
}

/// `str_assigned?`: some ancestor of the pair at `target` has an `lvasgn`
/// child assigning a string or symbol to `lvar`.
fn str_assigned(ancestor: &Node<'_>, target: Span, lvar: &[u8]) -> bool {
    let is_ruby_node = ancestor.kind() != NodeKind::ProgramNode
        && ancestor.as_statements_node().is_none_or(|statements| statements.body().len() != 1);
    if is_ruby_node
        && effective_children(ancestor).iter().any(|child| {
            child.as_local_variable_write_node().is_some_and(|write| {
                write.name().as_slice() == lvar && is_allowed_type(&write.value())
            })
        })
    {
        return true;
    }
    let mut next: Option<Node<'_>> = None;
    for_each_child(ancestor, |child| {
        let span = child.span();
        let is_target = span == target && child.kind() == NodeKind::AssocNode;
        if next.is_none() && !is_target && span.start <= target.start && target.end <= span.end {
            next = Some(child.clone());
        }
    });
    next.is_some_and(|child| str_assigned(&child, target, lvar))
}
