//! `Style/MixinGrouping`, ported from RuboCop's
//! `lib/rubocop/cop/style/mixin_grouping.rb`.
//!
//! Upstream finds the class/module's body via `node.child_nodes.find(&:begin_type?)`,
//! falling back to the class/module node itself when the body holds zero or
//! one statement (whitequark elides the wrapping `begin` node in that case).
//! Prism always wraps a class/module body in a [`NodeKind::StatementsNode`]
//! -- one statement or several alike -- so this port reaches the same direct
//! children uniformly through [`ClassNode::body`]/[`ModuleNode::body`], with
//! no fallback branch needed. A mixin call is only ever a direct child of
//! that body (a receiver-less [`NodeKind::CallNode`] naming `extend`,
//! `include` or `prepend`); Prism has no separate `csend` node for `&.`
//! macro calls, and `&.prepend Foo` (with no explicit receiver) is not valid
//! Ruby syntax, so no safe-navigation guard is needed.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{ClassNode, ModuleNode};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Separated,
    Grouped,
}

/// One direct-child mixin macro call (`extend`/`include`/`prepend` with no
/// receiver) in a class/module body.
struct Macro {
    /// The whole call's own span (RuboCop's `send_node.source_range`).
    span: Span,
    /// `extend`/`include`/`prepend`, matching the spelling in
    /// `MIXIN_METHODS`.
    method: &'static str,
    /// Each argument's own span, in source order.
    args: Vec<Span>,
}

/// Checks for grouping of mixins in `class` and `module` bodies.
#[derive(Debug, Clone)]
pub struct MixinGrouping {
    style: Style,
}

impl Rule for MixinGrouping {
    const META: RuleMeta = RuleMeta {
        name: "Style/MixinGrouping",
        department: Department::Style,
        summary: "Checks for grouping of mixins in `class` and `module` bodies.",
        explanation: "\
By default it enforces mixins to be placed in separate declarations, but it
can be configured to enforce grouping them in one declaration.

```ruby
# EnforcedStyle: separated (default)
# bad
class Foo
  include Bar, Qox
end

# good
class Foo
  include Qox
  include Bar
end
```

```ruby
# EnforcedStyle: grouped
# bad
class Foo
  extend Bar
  extend Qox
end

# good
class Foo
  extend Qox, Bar
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ClassNode, NodeKind::ModuleNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("separated"),
            allowed: &["separated", "grouped"],
            doc: "Whether mixins of the same kind must be spread across \
                  separate statements, or merged into a single statement.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "grouped" => Style::Grouped,
            _ => Style::Separated,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let body = match node {
            Node::ClassNode { .. } => node.as_class_node().and_then(|c: ClassNode<'_>| c.body()),
            Node::ModuleNode { .. } => node.as_module_node().and_then(|m: ModuleNode<'_>| m.body()),
            _ => return,
        };
        let Some(statements) = body.and_then(|b| b.as_statements_node()) else { return };

        let macros: Vec<Macro> = statements
            .body()
            .iter()
            .filter_map(|child| {
                let call = child.as_call_node()?;
                if call.receiver().is_some() {
                    return None;
                }
                let method = match call.name().as_slice() {
                    b"extend" => "extend",
                    b"include" => "include",
                    b"prepend" => "prepend",
                    _ => return None,
                };
                let args = call
                    .arguments()
                    .map(|a| a.arguments().iter().map(|n| n.span()).collect())
                    .unwrap_or_default();
                Some(Macro { span: child.span(), method, args })
            })
            .collect();

        for (index, macro_) in macros.iter().enumerate() {
            if macro_.args.is_empty() {
                continue;
            }
            match self.style {
                Style::Separated => Self::check_separated(ctx, macro_),
                Style::Grouped => Self::check_grouped(ctx, &macros, index),
            }
        }
    }
}

impl MixinGrouping {
    /// RuboCop's `check_separated_style`.
    fn check_separated(ctx: &mut Context<'_>, macro_: &Macro) {
        if macro_.args.len() == 1 {
            return;
        }
        let message = format!("Put `{}` mixins in separate statements.", macro_.method);
        let replacement = separate_mixins(ctx, macro_);
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(macro_.span, replacement)],
        };
        ctx.report_with_fix(&Self::META, macro_.span, message, fix);
    }

    /// RuboCop's `check_grouped_style`.
    fn check_grouped(ctx: &mut Context<'_>, macros: &[Macro], index: usize) {
        let current = &macros[index];
        let siblings: Vec<&Macro> = macros.iter().filter(|m| m.method == current.method).collect();
        if siblings.len() == 1 {
            return;
        }
        let message = format!("Put `{}` mixins in a single statement.", current.method);

        let sibling_index =
            siblings.iter().position(|m| std::ptr::eq(*m, current)).unwrap_or_default();
        let (range, replacement) = if sibling_index == 0 {
            (current.span, group_mixins(ctx, current.method, &siblings))
        } else {
            let prev = siblings[sibling_index - 1];
            (range_to_remove(ctx, prev.span, current.span), Vec::new())
        };

        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(range, replacement)],
        };
        ctx.report_with_fix(&Self::META, current.span, message, fix);
    }
}

/// RuboCop's `group_mixins`: every sibling's arguments, siblings taken in
/// reverse source order (so a call above the first sibling ends up with the
/// mixins of calls below it prepended), joined into one call.
fn group_mixins(ctx: &Context<'_>, method: &str, siblings: &[&Macro]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(method.as_bytes());
    out.push(b' ');
    let mut first = true;
    for sibling in siblings.iter().rev() {
        for arg in &sibling.args {
            if !first {
                out.extend_from_slice(b", ");
            }
            first = false;
            out.extend_from_slice(ctx.text(*arg));
        }
    }
    out
}

/// RuboCop's `separate_mixins`: the call's own arguments, reversed, one per
/// line indented to the call's own column.
fn separate_mixins(ctx: &Context<'_>, macro_: &Macro) -> Vec<u8> {
    let indent_width = ctx.line_col(macro_.span.start).column as usize;
    let mut out = Vec::new();
    for (i, arg) in macro_.args.iter().rev().enumerate() {
        if i > 0 {
            out.push(b'\n');
            out.extend(std::iter::repeat_n(b' ', indent_width));
        }
        out.extend_from_slice(macro_.method.as_bytes());
        out.push(b' ');
        out.extend_from_slice(ctx.text(*arg));
    }
    out
}

/// RuboCop's `range_to_remove_for_subsequent_mixin`: the removed macro's own
/// span, extended backwards through the gap since the previous same-named
/// sibling when that gap is whitespace-only (so no blank line is left
/// behind); otherwise just the macro's own span (the gap holds other code,
/// which must stay put).
fn range_to_remove(ctx: &Context<'_>, prev_span: Span, node_span: Span) -> Span {
    let between = Span::new(prev_span.end, node_span.start);
    if ctx.text(between).iter().any(|b| !b.is_ascii_whitespace()) {
        node_span
    } else {
        Span::new(prev_span.end, node_span.end)
    }
}
