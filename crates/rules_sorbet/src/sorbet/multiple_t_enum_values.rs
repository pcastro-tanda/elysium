//! `Sorbet/MultipleTEnumValues`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/multiple_t_enum_values.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::ClassNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "`T::Enum` should have at least two values.";

/// Ensures that all `T::Enum`s have multiple values.
#[derive(Debug, Clone)]
pub struct MultipleTEnumValues {
    /// `TEnum`'s `@scopes`: each enclosing class's span and whether it is a `T::Enum`.
    scopes: Vec<(ruby_source::Span, bool)>,
}

impl Rule for MultipleTEnumValues {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/MultipleTEnumValues",
        department: Department::Sorbet,
        summary: "Ensures that all `T::Enum`s have multiple values.",
        explanation: "",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::ClassNode, NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { scopes: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(class) = node.as_class_node() {
            let is_enum = t_enum(&class);
            self.scopes.push((node.span(), is_enum));
            if is_enum && class.body().is_none() {
                ctx.report(&Self::META, node.span(), MSG);
            }
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        let Some(&(scope, true)) = self.scopes.last() else { return };
        let Some(block) = enums_block(&call) else { return };
        let enough = match block.body() {
            None => false,
            Some(body) => casgn_count(&body) >= 2,
        };
        if !enough {
            ctx.report(&Self::META, scope, MSG);
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if node.as_class_node().is_some() {
            self.scopes.pop();
        }
    }
}

/// `(block (send nil? :enums) ...)` -- a plain `block`, not `numblock`/`itblock`.
fn enums_block<'pr>(
    call: &ruby_ast::node::CallNode<'pr>,
) -> Option<ruby_ast::node::BlockNode<'pr>> {
    if call.receiver().is_some()
        || call.is_safe_navigation()
        || call.name().as_slice() != b"enums"
        || call.arguments().is_some()
    {
        return None;
    }
    let block = call.block()?.as_block_node()?;
    let numbered = block.parameters().is_some_and(|p| {
        p.as_numbered_parameters_node().is_some() || p.as_it_parameters_node().is_some()
    });
    (!numbered).then_some(block)
}

fn is_casgn(node: &Node<'_>) -> bool {
    node.as_constant_write_node().is_some() || node.as_constant_path_write_node().is_some()
}

/// `(begin_node || node).children.count(&:casgn_type?)` for the block `node`,
/// where `begin_node` is the block's body when it is a `begin` (several
/// statements, or a parenthesized expression) and otherwise the block's
/// children are `[send, args, body]`.
fn casgn_count(body: &Node<'_>) -> usize {
    let Some(statements) = body.as_statements_node() else { return 0 };
    let list = statements.body();
    let mut iter = list.iter();
    let (Some(first), None) = (iter.next(), iter.next()) else {
        return list.iter().filter(|stmt| is_casgn(stmt)).count();
    };
    if let Some(parens) = first.as_parentheses_node() {
        // `(begin ...)`: the children are the parenthesized statements.
        return match parens.body().and_then(|b| b.as_statements_node().map(|s| s.body())) {
            Some(inner) => inner.iter().filter(|stmt| is_casgn(stmt)).count(),
            None => 0,
        };
    }
    usize::from(is_casgn(&first))
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
