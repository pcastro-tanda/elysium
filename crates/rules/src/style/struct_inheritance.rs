//! `Style/StructInheritance`, ported from RuboCop's
//! `lib/rubocop/cop/style/struct_inheritance.rb`.
//!
//! Upstream's `parent_class` is a superclass expression that, in Prism, is
//! always a [`NodeKind::CallNode`]: whitequark wraps a `Struct.new do ... end`
//! superclass in a separate `block` node, but Prism attaches the block
//! directly to the `new` call's own [`CallNode::block`] field instead, so
//! there is no separate block-shaped alternative to match against --
//! [`is_struct_new_call`] alone covers both of upstream's
//! `struct_constructor?` pattern alternatives.
//!
//! `defines_constants?` walks the class body (upstream's
//! `each_node(:casgn, :class, :module)`) for a constant assignment or a
//! nested class/module -- [`ConstantWriteNode`]/[`ConstantPathWriteNode`]
//! stand in for whitequark's `casgn`.
//!
//! The autocorrection mirrors `correct_parent`'s four branches verbatim:
//! an already-attached `do...end` block loses its own (now redundant)
//! `end` keyword to the class's trailing `end`; an empty class body is
//! deleted outright (the whole `; end`/on its own line(s)); an
//! unparenthesized call gains parentheses and a trailing `do`; anything
//! else just gets `do` appended after it.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::node::{CallNode, ClassNode};
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

/// RuboCop's `MSG`.
const MSG: &str =
    "Don't extend an instance initialized by `Struct.new`. Use a block to customize the struct.";

/// Checks for inheritance from `Struct.new`.
#[derive(Debug, Clone)]
pub struct StructInheritance;

impl Rule for StructInheritance {
    const META: RuleMeta = RuleMeta {
        name: "Style/StructInheritance",
        department: Department::Style,
        summary: "Checks for inheritance from `Struct.new`.",
        explanation: "\
Checks for inheritance from `Struct.new`. Inheriting from `Struct.new`
adds a superfluous level in inheritance tree.

Autocorrection is unsafe because it will change the inheritance tree
(e.g. return value of `Module#ancestors`) of the constant.

It is also unsafe because constants that the class body resolves through
its ancestors (e.g. one provided by an included module) fall out of
scope inside the block.

```ruby
# bad
class Person < Struct.new(:first_name, :last_name)
  def age
    42
  end
end

Person.ancestors
# => [Person, #<Class:0x000000010b4e14a0>, Struct, (...)]

# good
Person = Struct.new(:first_name, :last_name) do
  def age
    42
  end
end

Person.ancestors
# => [Person, Struct, (...)]
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ClassNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(class_node) = node.as_class_node() else { return };
        let Some(superclass) = class_node.superclass() else { return };
        let Some(call) = superclass.as_call_node() else { return };
        if !is_struct_new_call(&call) {
            return;
        }
        if defines_constants(class_node.body()) {
            return;
        }

        let span = superclass.span();
        let fix = build_fix(ctx, &class_node, &call, span);
        ctx.report_with_fix(&Self::META, span, MSG, fix);
    }
}

/// RuboCop's `struct_constructor?` node matcher, narrowed to what Prism can
/// produce as a superclass expression: a `Struct.new(...)` call (its own
/// attached `do...end`/`{...}` block, if any, is Prism's
/// [`CallNode::block`], not a wrapping node -- see the module doc comment).
/// `::Struct.new` matches too (`{nil? cbase}` upstream); `MyModule::Struct.new`
/// does not.
fn is_struct_new_call(call: &CallNode<'_>) -> bool {
    if call.name().as_slice() != b"new" {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    is_bare_or_toplevel_const(&receiver) && const_name(&receiver).as_deref() == Some("Struct")
}

/// The set of body node kinds `defines_constants` looks for: upstream's
/// `casgn`/`class`/`module`, i.e. a constant assignment or a nested
/// class/module definition.
fn is_constant_defining(n: &Node<'_>) -> bool {
    matches!(
        n.kind(),
        NodeKind::ConstantWriteNode
            | NodeKind::ConstantPathWriteNode
            | NodeKind::ClassNode
            | NodeKind::ModuleNode
    )
}

/// RuboCop's `defines_constants?`: the class body, including nested class
/// and module bodies, declares a constant (a `casgn`, matched here by
/// [`NodeKind::ConstantWriteNode`]/[`NodeKind::ConstantPathWriteNode`]) or
/// defines a nested class or module -- any of which would leak into the
/// enclosing namespace if rewritten as a `Struct.new do ... end` block.
fn defines_constants(body: Option<Node<'_>>) -> bool {
    let Some(body) = body else { return false };
    if is_constant_defining(&body) {
        return true;
    }
    let mut found = false;
    each_descendant(&body, &mut |n| {
        if is_constant_defining(n) {
            found = true;
        }
    });
    found
}

/// RuboCop's `correct_parent`, dispatching on the superclass call's shape.
fn build_fix(
    ctx: &Context<'_>,
    class_node: &ClassNode<'_>,
    call: &CallNode<'_>,
    span: Span,
) -> Fix {
    let mut edits = Vec::new();

    // `corrector.remove(range_with_surrounding_space(node.loc.keyword, side:
    // :right, newlines: false))`: drop the `class` keyword and the single
    // run of horizontal whitespace after it.
    let keyword_span = class_node.class_keyword_loc().span();
    edits.push(Edit::delete(ctx.with_surrounding_space(keyword_span, Side::Right, false, false)));

    // `corrector.replace(node.loc.operator, '=')`.
    let operator_span =
        class_node.inheritance_operator_loc().expect("superclass is present").span();
    edits.push(Edit::replace(operator_span, b"=".to_vec()));

    if let Some(block) = call.block() {
        // `parent.block_type?`: the call already carries its own `do...end`
        // (or `{...}`); its own closing keyword is redundant now that the
        // class's trailing `end` will close the block instead.
        let block_node = block.as_block_node().expect("CallNode::block is always a BlockNode");
        let closing_span = block_node.closing_loc().span();
        edits.push(Edit::delete(ctx.with_surrounding_space(
            closing_span,
            Side::Both,
            false,
            false,
        )));
    } else if class_node.body().is_none() {
        // `(class_node = parent.parent).body.nil?`: an empty class body
        // needs no block at all -- delete whatever would otherwise become
        // an empty `do ... end`.
        let class_span = class_node.location().span();
        if ctx.is_single_line(class_span) {
            edits.push(Edit::delete(Span::new(span.end, class_span.end)));
        } else {
            let end_span = class_node.end_keyword_loc().span();
            edits.push(Edit::delete(ctx.whole_lines(end_span)));
        }
    } else if is_unparenthesized_struct_new(call) {
        // `unparenthesized_struct_new?` / `wrap_unparenthesized_call_with_do`.
        let args_source = call
            .arguments()
            .map(|args| {
                args.arguments()
                    .iter()
                    .map(|arg| String::from_utf8_lossy(ctx.text(arg.span())).into_owned())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        let message_end = call.message_loc().expect("named call has a selector").span().end;
        let range = Span::new(message_end, span.end);
        edits.push(Edit::replace(range, format!("({args_source}) do").into_bytes()));
    } else {
        // `corrector.insert_after(parent, ' do')`.
        edits.push(Edit::insert(span.end, b" do".to_vec()));
    }

    Fix { applicability: Applicability::Unsafe, edits }
}

/// RuboCop's `unparenthesized_struct_new?`.
fn is_unparenthesized_struct_new(call: &CallNode<'_>) -> bool {
    call.arguments().is_some_and(|args| !args.arguments().is_empty())
        && call.opening_loc().is_none()
}
