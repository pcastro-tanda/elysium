//! `Lint/RaiseException`, ported from RuboCop's
//! `lib/rubocop/cop/lint/raise_exception.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Use `StandardError` over `Exception`.";

/// Checks for `raise` or `fail` statements which are raising `Exception` class.
#[derive(Debug, Clone)]
pub struct RaiseException {
    allowed_implicit_namespaces: Vec<String>,
    /// Names of enclosing `module` nodes, innermost last -- RuboCop's
    /// `implicit_namespace?` walks `node.parent` looking for the nearest
    /// `module_type?` ancestor (skipping classes/defs/blocks along the way);
    /// tracking only `ModuleNode` enter/leave as a stack gives the same
    /// "nearest enclosing module" at the top without needing full ancestor
    /// node lookups.
    module_stack: Vec<String>,
}

impl Rule for RaiseException {
    const META: RuleMeta = RuleMeta {
        name: "Lint/RaiseException",
        department: Department::Lint,
        summary: "Checks for `raise` or `fail` statements which are raising `Exception` class.",
        explanation: "\
Checks for `raise` or `fail` statements which raise `Exception` or
`Exception.new`. Use `StandardError` or a specific exception class instead.

If you have defined your own namespaced `Exception` class, it is possible
to configure the cop to allow it by setting `AllowedImplicitNamespaces` to
an array with the names of the namespaces to allow. By default, this is set to
`['Gem']`, which allows `Gem::Exception` to be raised without an explicit
namespace. If not allowed, a false positive may be registered if
`raise Exception` is called within the namespace.

Alternatively, use a fully qualified name with `raise`/`fail`
(eg. `raise Namespace::Exception`).

```ruby
# bad
raise Exception, 'Error message here'
raise Exception.new('Error message here')

# good
raise StandardError, 'Error message here'
raise MyError.new, 'Error message here'
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::ModuleNode],
        config: &[ConfigOption {
            name: "AllowedImplicitNamespaces",
            default: ConfigDefault::StrList(&["Gem"]),
            allowed: &[],
            doc: "Allows `Exception` to be raised bare inside these module namespaces.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            allowed_implicit_namespaces: options.str_list("AllowedImplicitNamespaces"),
            module_stack: Vec::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(module) = node.as_module_node() {
            self.module_stack.push(String::from_utf8_lossy(module.name().as_slice()).into_owned());
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if call.receiver().is_some() {
            return;
        }
        let name = call.name().as_slice();
        if name != b"raise" && name != b"fail" {
            return;
        }
        let Some(first) = call.arguments().and_then(|args| args.arguments().first()) else {
            return;
        };

        let exception = exception_const(&first).or_else(|| {
            let inner = first.as_call_node()?;
            if inner.name().as_slice() != b"new" {
                return None;
            }
            exception_const(&inner.receiver()?)
        });
        let Some(exception) = exception else { return };

        if !exception.has_cbase && self.implicit_namespace_allowed() {
            return;
        }

        let replacement: &[u8] =
            if exception.has_cbase { b"::StandardError" } else { b"StandardError" };
        let fix = Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::replace(exception.span, replacement.to_vec())],
        };
        ctx.report_with_fix(&Self::META, exception.span, MSG, fix);
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if node.as_module_node().is_some() {
            self.module_stack.pop();
        }
    }
}

impl RaiseException {
    /// RuboCop's `implicit_namespace?`: is the nearest enclosing `module`
    /// (if any) one of `AllowedImplicitNamespaces`?
    fn implicit_namespace_allowed(&self) -> bool {
        self.module_stack
            .last()
            .is_some_and(|ns| self.allowed_implicit_namespaces.iter().any(|allowed| allowed == ns))
    }
}

/// The span and "has an explicit `::` prefix" flag of a `(const {cbase nil?}
/// :Exception)` node -- RuboCop's `exception?`/`exception_new_with_message?`
/// pattern's `$(const ${cbase nil?} :Exception)` capture. Matches a bare
/// `Exception` (`ConstantReadNode`) or a top-level-qualified `::Exception`
/// (`ConstantPathNode` with no `parent`), but not a namespaced
/// `Foo::Exception`.
struct ExceptionConst {
    span: Span,
    has_cbase: bool,
}

fn exception_const(node: &Node<'_>) -> Option<ExceptionConst> {
    match node.kind() {
        NodeKind::ConstantReadNode => {
            let c = node.as_constant_read_node()?;
            (c.name().as_slice() == b"Exception")
                .then_some(ExceptionConst { span: node.span(), has_cbase: false })
        }
        NodeKind::ConstantPathNode => {
            let path = node.as_constant_path_node()?;
            (path.parent().is_none() && path.name().is_some_and(|n| n.as_slice() == b"Exception"))
                .then_some(ExceptionConst { span: node.span(), has_cbase: true })
        }
        _ => None,
    }
}
