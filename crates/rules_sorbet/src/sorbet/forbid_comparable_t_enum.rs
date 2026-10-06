//! `Sorbet/ForbidComparableTEnum`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/forbid_comparable_t_enum.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::ClassNode;
use ruby_ast::{Node, NodeKind};

const MSG: &str =
    "Do not use `T::Enum` as a comparable object because of significant performance overhead.";

/// Disallows including the `Comparable` module in a `T::Enum`.
#[derive(Debug, Clone)]
pub struct ForbidComparableTEnum {
    /// `TEnum`'s `@scopes`: whether each enclosing class is a `T::Enum`.
    scopes: Vec<bool>,
}

impl Rule for ForbidComparableTEnum {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/ForbidComparableTEnum",
        department: Department::Sorbet,
        summary: "Disallows including the `Comparable` module in a `T::Enum`.",
        explanation: "",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::ClassNode, NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { scopes: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(class) = node.as_class_node() {
            self.scopes.push(t_enum(&class));
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if self.scopes.last() != Some(&true) || !mix_in_comparable(&call) {
            return;
        }
        ctx.report(&Self::META, ruby_ast::ext::call_span_excluding_block(&call), MSG);
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if node.as_class_node().is_some() {
            self.scopes.pop();
        }
    }
}

/// `(send nil? {:include | :prepend} (const nil? :Comparable))`.
fn mix_in_comparable(call: &ruby_ast::node::CallNode<'_>) -> bool {
    if call.receiver().is_some()
        || call.is_safe_navigation()
        || !matches!(call.name().as_slice(), b"include" | b"prepend")
        || call.block().is_some_and(|block| block.as_block_argument_node().is_some())
    {
        return false;
    }
    let Some(arguments) = call.arguments() else { return false };
    let list = arguments.arguments();
    let mut iter = list.iter();
    let (Some(arg), None) = (iter.next(), iter.next()) else { return false };
    arg.as_constant_read_node().is_some_and(|c| c.name().as_slice() == b"Comparable")
}

/// `TEnum#t_enum?`: `(class (const ...) (const (const nil? :T) :Enum) ...)`.
fn t_enum(class: &ClassNode<'_>) -> bool {
    let path = class.constant_path();
    if path.as_constant_read_node().is_none() && path.as_constant_path_node().is_none() {
        return false;
    }
    class.superclass().is_some_and(|sup| is_t_const(&sup, &[b"Enum"]))
}

/// `(const (const nil? :T) {names})`.
fn is_t_const(node: &Node<'_>, names: &[&[u8]]) -> bool {
    let Some(path) = node.as_constant_path_node() else { return false };
    let Some(parent) = path.parent() else { return false };
    let Some(t) = parent.as_constant_read_node() else { return false };
    t.name().as_slice() == b"T" && path.name().is_some_and(|name| names.contains(&name.as_slice()))
}
