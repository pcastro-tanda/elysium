//! `Style/ModuleFunction`, ported from RuboCop's
//! `lib/rubocop/cop/style/module_function.rb`.
//!
//! Upstream's `on_module` inspects the module's direct body statements
//! (`node.body.begin_type? ? node.body.children : [node.body]`). Prism
//! always wraps a non-empty `ModuleNode` body in a `StatementsNode` (see
//! `constant_definition_in_block`'s note on the same pattern), so this port
//! simply reads `ModuleNode::body`'s `StatementsNode` children directly,
//! falling back to treating the body itself as the sole statement on the
//! (unreachable in practice, but harmless) chance it is not a
//! `StatementsNode`.
//!
//! The three node matchers (`extend_self_node?`, `module_function_node?`,
//! `private_directive?`) all pattern-match `(send nil? ...)`, i.e. a
//! receiver-less, non-safe-navigation call; `extend self` additionally
//! requires exactly one argument that is a bare `self`, and
//! `module_function` (with no argument) requires no arguments at all --
//! `module_function :foo` is a different, unflagged call.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{node::CallNode, Node, NodeExt as _, NodeKind};

const MODULE_FUNCTION_MSG: &str = "Use `module_function` instead of `extend self`.";
const EXTEND_SELF_MSG: &str = "Use `extend self` instead of `module_function`.";
const FORBIDDEN_MSG: &str = "Do not use `module_function` or `extend self`.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    ModuleFunction,
    ExtendSelf,
    Forbidden,
}

/// Checks for use of `extend self` or `module_function` in a module.
#[derive(Debug, Clone)]
pub struct ModuleFunction {
    style: Style,
}

impl Rule for ModuleFunction {
    const META: RuleMeta = RuleMeta {
        name: "Style/ModuleFunction",
        department: Department::Style,
        summary: "Checks for use of `extend self` or `module_function` in a module.",
        explanation: "\
Checks for use of `extend self` or `module_function` in a module.

Supported styles are: `module_function` (default), `extend_self` and `forbidden`.

A couple of things to keep in mind:

* `forbidden` style prohibits the usage of both styles
* in default mode (`module_function`), the cop won't be activated when the
  module contains any private methods

```ruby
# EnforcedStyle: module_function (default)
# bad
module Test
  extend self
  # ...
end

# good
module Test
  module_function
  # ...
end

# good
module Test
  extend self
  # ...
  private
  # ...
end

# EnforcedStyle: extend_self
# bad
module Test
  module_function
  # ...
end

# good
module Test
  extend self
  # ...
end

# EnforcedStyle: forbidden
# bad
module Test
  module_function
  # ...
end

# bad
module Test
  extend self
  # ...
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ModuleNode],
        config: &[linter::ConfigOption {
            name: "EnforcedStyle",
            default: linter::ConfigDefault::Str("module_function"),
            allowed: &["module_function", "extend_self", "forbidden"],
            doc: "Which of `extend self`/`module_function` to enforce (or forbid both).",
        }],
        blind_spots: "\
Autocorrection is unsafe (and is disabled by default) because `extend self`
and `module_function` do not behave exactly the same.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "extend_self" => Style::ExtendSelf,
            "forbidden" => Style::Forbidden,
            _ => Style::ModuleFunction,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if node.kind() != NodeKind::ModuleNode {
            return;
        }
        let module = node.as_module_node().expect("kind matched");
        let Some(body) = module.body() else { return };
        let items: Vec<Node<'_>> = match body.as_statements_node() {
            Some(stmts) => stmts.body().iter().collect(),
            None => vec![body],
        };

        match self.style {
            Style::ModuleFunction => {
                if items.iter().any(is_private_directive) {
                    return;
                }
                for item in &items {
                    if let Some(call) = as_extend_self(item) {
                        self.report(&call, ctx);
                    }
                }
            }
            Style::ExtendSelf => {
                for item in &items {
                    if let Some(call) = as_module_function(item) {
                        self.report(&call, ctx);
                    }
                }
            }
            Style::Forbidden => {
                for item in &items {
                    let flagged = as_extend_self(item).or_else(|| as_module_function(item));
                    if let Some(call) = flagged {
                        self.report(&call, ctx);
                    }
                }
            }
        }
    }
}

impl ModuleFunction {
    /// RuboCop's `add_offense` block: replaces `extend self` with
    /// `module_function` or vice versa, except in `forbidden` style, whose
    /// corrector block returns immediately (i.e. never fixes).
    fn report(&self, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        let span = call.as_node().span();
        match self.style {
            Style::Forbidden => ctx.report(&Self::META, span, FORBIDDEN_MSG),
            Style::ModuleFunction => ctx.report_with_fix(
                &Self::META,
                span,
                MODULE_FUNCTION_MSG,
                Fix {
                    applicability: Applicability::Unsafe,
                    edits: vec![Edit::replace(span, b"module_function".as_slice())],
                },
            ),
            Style::ExtendSelf => ctx.report_with_fix(
                &Self::META,
                span,
                EXTEND_SELF_MSG,
                Fix {
                    applicability: Applicability::Unsafe,
                    edits: vec![Edit::replace(span, b"extend self".as_slice())],
                },
            ),
        }
    }
}

/// RuboCop's `extend_self_node?`: `(send nil? :extend self)`.
fn as_extend_self<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    let call = node.as_call_node()?;
    if call.receiver().is_some() || call.is_safe_navigation() {
        return None;
    }
    if call.name().as_slice() != b"extend" {
        return None;
    }
    let args: Vec<Node<'pr>> = call.arguments()?.arguments().iter().collect();
    let [arg] = args.as_slice() else { return None };
    arg.as_self_node()?;
    Some(call)
}

/// RuboCop's `module_function_node?`: `(send nil? :module_function)`, i.e.
/// no arguments at all (`module_function :foo` is untouched).
fn as_module_function<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    let call = node.as_call_node()?;
    if call.receiver().is_some() || call.is_safe_navigation() {
        return None;
    }
    if call.name().as_slice() != b"module_function" {
        return None;
    }
    if call.arguments().is_some() {
        return None;
    }
    Some(call)
}

/// RuboCop's `private_directive?`: `(send nil? :private ...)`, matching a
/// bare `private` or `private :foo`/`private def foo; end` alike.
fn is_private_directive(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    call.receiver().is_none() && !call.is_safe_navigation() && call.name().as_slice() == b"private"
}
