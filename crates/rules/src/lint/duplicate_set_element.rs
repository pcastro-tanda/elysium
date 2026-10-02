//! `Lint/DuplicateSetElement`, ported from RuboCop's
//! `lib/rubocop/cop/lint/duplicate_set_element.rb`.
//!
//! Upstream dedupes elements by rubocop-ast `Node#==` (structural AST
//! equality); since every element this cop considers is a literal, a
//! constant, or a variable read (anything else is skipped before the
//! dedup check), comparing each element's own source text is equivalent
//! here and avoids reimplementing generic node equality.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt, NodeKind};
use ruby_source::Span;

/// Checks for duplicate literal, constant, or variable elements in `Set` and `SortedSet`.
#[derive(Debug, Clone)]
pub struct DuplicateSetElement;

impl Rule for DuplicateSetElement {
    const META: RuleMeta = RuleMeta {
        name: "Lint/DuplicateSetElement",
        department: Department::Lint,
        summary: "Checks for duplicate elements in Set.",
        explanation: "\
Checks for duplicate literal, constant, or variable elements in `Set` and `SortedSet`.

```ruby
# bad
Set[:foo, :bar, :foo]

# good
Set[:foo, :bar]

# bad
Set.new([:foo, :bar, :foo])

# good
Set.new([:foo, :bar])

# bad
[:foo, :bar, :foo].to_set

# good
[:foo, :bar].to_set

# bad
SortedSet[:foo, :bar, :foo]

# good
SortedSet[:foo, :bar]

# bad
SortedSet.new([:foo, :bar, :foo])

# good
SortedSet.new([:foo, :bar])
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let name = call.name().as_slice();
        if !matches!(name, b"[]" | b"new" | b"to_set") {
            return;
        }
        let Some(elements) = set_init_elements(&call) else { return };

        let class_name = call
            .receiver()
            .filter(|r| {
                r.kind() == NodeKind::ConstantReadNode || r.kind() == NodeKind::ConstantPathNode
            })
            .and_then(|r| ruby_ast::ext::const_name(&r))
            .unwrap_or_else(|| "Set".to_string());

        let mut seen: Vec<&[u8]> = Vec::new();
        for (index, element) in elements.iter().enumerate() {
            if !is_literal(element.kind())
                && !is_const(element.kind())
                && !is_variable(element.kind())
            {
                continue;
            }
            let text = ctx.text(element.span());
            if seen.contains(&text) {
                let prev = &elements[index - 1];
                register_offense(ctx, &class_name, prev.span(), element.span());
            } else {
                seen.push(text);
            }
        }
    }
}

fn register_offense(ctx: &mut Context<'_>, class_name: &str, prev_span: Span, element_span: Span) {
    let msg = format!("Remove the duplicate element in {class_name}.");
    let removal = Span::new(prev_span.end, element_span.end);
    let fix = Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(removal)] };
    ctx.report_with_fix(&DuplicateSetElement::META, element_span, msg, fix);
}

/// RuboCop's `set_init_elements` node matcher: `Set[...]`/`SortedSet[...]`,
/// `Set.new([...])`/`SortedSet.new([...])`, or `[...].to_set` (the latter
/// via `call`, i.e. matching `&.to_set` too).
fn set_init_elements<'pr>(call: &CallNode<'pr>) -> Option<Vec<Node<'pr>>> {
    match call.name().as_slice() {
        b"[]" => {
            if call.is_safe_navigation() {
                return None;
            }
            let receiver = call.receiver()?;
            set_class_name(&receiver)?;
            Some(call.arguments()?.arguments().iter().collect())
        }
        b"new" => {
            if call.is_safe_navigation() {
                return None;
            }
            let receiver = call.receiver()?;
            set_class_name(&receiver)?;
            let args = call.arguments()?.arguments();
            if args.len() != 1 {
                return None;
            }
            let array = args.iter().next()?.as_array_node()?;
            Some(array.elements().iter().collect())
        }
        b"to_set" => {
            if call.arguments().is_some() {
                return None;
            }
            let array = call.receiver()?.as_array_node()?;
            Some(array.elements().iter().collect())
        }
        _ => None,
    }
}

/// `{nil? cbase}` applied to `{:Set :SortedSet}`: a bare or top-level
/// constant named `Set` or `SortedSet`.
fn set_class_name(receiver: &Node<'_>) -> Option<()> {
    if !ruby_ast::ext::is_bare_or_toplevel_const(receiver) {
        return None;
    }
    let name = ruby_ast::ext::const_name(receiver)?;
    (name == "Set" || name == "SortedSet").then_some(())
}

/// rubocop-ast's `LITERALS` (`Node#literal?`).
fn is_literal(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::StringNode
            | NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::SymbolNode
            | NodeKind::TrueNode
            | NodeKind::FalseNode
            | NodeKind::NilNode
            | NodeKind::ImaginaryNode
            | NodeKind::RationalNode
            | NodeKind::InterpolatedStringNode
            | NodeKind::XStringNode
            | NodeKind::InterpolatedXStringNode
            | NodeKind::InterpolatedSymbolNode
            | NodeKind::ArrayNode
            | NodeKind::HashNode
            | NodeKind::RegularExpressionNode
            | NodeKind::InterpolatedRegularExpressionNode
            | NodeKind::RangeNode
    )
}

/// rubocop-ast's `Node#const_type?`.
fn is_const(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::ConstantReadNode | NodeKind::ConstantPathNode)
}

/// rubocop-ast's `Node#variable?`.
fn is_variable(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::LocalVariableReadNode
            | NodeKind::InstanceVariableReadNode
            | NodeKind::ClassVariableReadNode
            | NodeKind::GlobalVariableReadNode
    )
}
