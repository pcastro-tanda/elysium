//! `Style/OpenStructUse`, ported from RuboCop's
//! `lib/rubocop/cop/style/open_struct_use.rb`.
//!
//! whitequark's unified `:const` node type (covering both a bare
//! `ConstantReadNode` and a namespaced/cbase-rooted `ConstantPathNode`) is
//! handled by matching both kinds. `custom_class_or_module_definition?`
//! (`parent&.type?(:class, :module) && node.left_siblings.empty?`, i.e. is
//! this constant the class/module's *own name*, not its superclass) has no
//! direct `left_siblings` equivalent in Prism; a `ClassNode`'s superclass
//! can only follow its `constant_path` after the `<` inheritance operator,
//! so a constant reference under a `ClassNode`/`ModuleNode` parent is the
//! definition's own name (exempt) iff there is no `<` between the class
//! keyword and the reference -- a `ModuleNode` never has a superclass, so
//! any reference directly under one is always its own name.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str =
    "Avoid using `OpenStruct`; use `Struct`, `Hash`, a class or test doubles instead.";

/// Avoid using `OpenStruct`. As of Ruby 3.0, use is officially discouraged due to performance, version compatibility, and potential security issues.
#[derive(Debug, Clone)]
pub struct OpenStructUse;

impl Rule for OpenStructUse {
    const META: RuleMeta = RuleMeta {
        name: "Style/OpenStructUse",
        department: Department::Style,
        summary: "Avoid using OpenStruct. As of Ruby 3.0, use is officially discouraged due to performance, version compatibility, and potential security issues.",
        explanation: "\
# @safety
Note that this cop may flag false positives; for instance, the following
legal use of a hand-rolled `OpenStruct` type would be considered an
offense:

```ruby
module MyNamespace
  class OpenStruct # not the OpenStruct we're looking for
  end

  def new_struct
    OpenStruct.new # resolves to MyNamespace::OpenStruct
  end
end
```

```ruby
# bad
point = OpenStruct.new(x: 0, y: 1)

# good
Point = Struct.new(:x, :y)
point = Point.new(0, 1)

# also good
point = { x: 0, y: 1 }
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::ConstantReadNode, NodeKind::ConstantPathNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let is_open_struct = if let Some(read) = node.as_constant_read_node() {
            read.name().as_slice() == b"OpenStruct"
        } else if let Some(path) = node.as_constant_path_node() {
            // `{nil? (cbase)}`: a bare name or a `::`-rooted path, never a
            // namespaced `Foo::OpenStruct` (whose own `parent` is `Some`).
            path.parent().is_none()
                && path.name().is_some_and(|name| name.as_slice() == b"OpenStruct")
        } else {
            false
        };
        if !is_open_struct {
            return;
        }

        if let Some(parent) = ctx.parent() {
            let is_class = parent.kind == NodeKind::ClassNode;
            let is_module = parent.kind == NodeKind::ModuleNode;
            if is_module {
                // A module never has a superclass, so any constant
                // reference directly under one is its own name.
                return;
            }
            if is_class {
                // The reference is the class's own name (exempt) unless an
                // inheritance operator (`<`) appears before it, in which
                // case it is the superclass reference.
                let between = ctx.text(Span::new(parent.span.start, node.span().start));
                if !between.contains(&b'<') {
                    return;
                }
            }
        }

        ctx.report(&Self::META, node.span(), MSG);
    }
}
