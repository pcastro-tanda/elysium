//! `Sorbet/ForbidUntypedStructProps`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/forbid_untyped_struct_props.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use std::collections::HashSet;

use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Struct props cannot be T.untyped";

/// Disallows use of `T.untyped` or `T.nilable(T.untyped)` as a prop type for `T::Struct` subclasses.
#[derive(Debug, Clone)]
pub struct ForbidUntypedStructProps {
    /// RuboCop drops a second offense on an identical range (nested structs).
    reported: HashSet<Span>,
}

impl Rule for ForbidUntypedStructProps {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/ForbidUntypedStructProps",
        department: Department::Sorbet,
        summary: "Disallows use of `T.untyped` or `T.nilable(T.untyped)` as a prop type for `T::Struct` subclasses.",
        explanation: "",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::ClassNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { reported: HashSet::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(class) = node.as_class_node() else { return };
        // `(class (const ...) #t_struct ...)`
        let path = class.constant_path();
        if path.as_constant_read_node().is_none() && path.as_constant_path_node().is_none() {
            return;
        }
        if !class.superclass().is_some_and(|sup| t_struct(&sup)) {
            return;
        }
        let mut found = Vec::new();
        // `def_node_search` also tests the searched node itself.
        let mut visit = |child: &Node<'_>| {
            if let Some(call) = child.as_call_node() {
                if let Some(span) = untyped_prop(&call) {
                    found.push(span);
                }
            }
        };
        visit(node);
        ruby_ast::each_descendant(node, &mut visit);
        for span in found {
            if self.reported.insert(span) {
                ctx.report(&Self::META, span, MSG);
            }
        }
    }
}

/// `(const (const nil? :T) {:Struct :ImmutableStruct})`
fn t_struct(node: &Node<'_>) -> bool {
    let Some(path) = node.as_constant_path_node() else { return false };
    let Some(parent) = path.parent() else { return false };
    let Some(t) = parent.as_constant_read_node() else { return false };
    t.name().as_slice() == b"T"
        && path.name().is_some_and(|name| matches!(name.as_slice(), b"Struct" | b"ImmutableStruct"))
}

fn is_plain_t(node: &Node<'_>) -> bool {
    node.as_constant_read_node().is_some_and(|c| c.name().as_slice() == b"T")
}

/// A `send` (not `csend`, not wrapped in a `block`) on `T` named `name`.
fn t_call<'pr>(node: &Node<'pr>, name: &[u8]) -> Option<CallNode<'pr>> {
    let call = node.as_call_node()?;
    if call.is_safe_navigation()
        || call.name().as_slice() != name
        || !call.receiver().is_some_and(|r| is_plain_t(&r))
        || call.block().is_some()
    {
        return None;
    }
    Some(call)
}

/// `(send (const nil? :T) :untyped)`
fn t_untyped(node: &Node<'_>) -> bool {
    t_call(node, b"untyped").is_some_and(|call| call.arguments().is_none())
}

/// `(send (const nil? :T) :nilable {#t_untyped #t_nilable_untyped})`
fn t_nilable_untyped(node: &Node<'_>) -> bool {
    let Some(call) = t_call(node, b"nilable") else { return false };
    let Some(arguments) = call.arguments() else { return false };
    let list = arguments.arguments();
    let mut iter = list.iter();
    let (Some(arg), None) = (iter.next(), iter.next()) else { return false };
    t_untyped(&arg) || t_nilable_untyped(&arg)
}

/// `(send nil? {:prop :const} _ ${#t_untyped #t_nilable_untyped} ...)`: the
/// span of the captured type.
fn untyped_prop(call: &CallNode<'_>) -> Option<Span> {
    if call.receiver().is_some()
        || call.is_safe_navigation()
        || !matches!(call.name().as_slice(), b"prop" | b"const")
    {
        return None;
    }
    let arguments = call.arguments()?;
    let ty = arguments.arguments().iter().nth(1)?;
    (t_untyped(&ty) || t_nilable_untyped(&ty)).then(|| ty.span())
}
