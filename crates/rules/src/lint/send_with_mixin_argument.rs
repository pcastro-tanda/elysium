//! `Lint/SendWithMixinArgument`, ported from RuboCop's
//! `lib/rubocop/cop/lint/send_with_mixin_argument.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Checks for `send`, `public_send`, and `__send__` methods when using
/// mix-in.
#[derive(Debug, Clone)]
pub struct SendWithMixinArgument;

impl Rule for SendWithMixinArgument {
    const META: RuleMeta = RuleMeta {
        name: "Lint/SendWithMixinArgument",
        department: Department::Lint,
        summary: "Checks for `send` method when using mixin.",
        explanation: "`include` and `prepend` methods were private methods until Ruby 2.0, \
            they were mixed-in via `send` method. This cop uses Ruby 2.1 or higher style that \
            can be called by public methods. And `extend` method that was originally a public \
            method is also targeted for style unification.",
        enabled_by_default: true,
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

        if !matches!(call.name().as_slice(), b"send" | b"public_send" | b"__send__") {
            return;
        }

        let Some(receiver) = call.receiver() else { return };
        if !is_const(&receiver) {
            return;
        }

        let Some(arguments) = call.arguments() else { return };
        let args = arguments.arguments();
        if args.len() < 2 {
            return;
        }

        let Some(first) = args.first() else { return };
        let Some(method) = mixin_method_name(&first) else { return };

        let module_names: Vec<Node<'_>> = args.iter().skip(1).collect();
        if !module_names.iter().all(is_const) {
            return;
        }

        let Some(message_loc) = call.message_loc() else { return };
        let bad_span = Span::new(message_loc.span().start, node.span().end);
        let bad_method = String::from_utf8_lossy(ctx.text(bad_span)).into_owned();

        let module_names_source = module_names
            .iter()
            .map(|m| String::from_utf8_lossy(ctx.text(m.span())).into_owned())
            .collect::<Vec<_>>()
            .join(", ");

        let message = format!("Use `{method} {module_names_source}` instead of `{bad_method}`.");

        let replacement = format!("{method} {module_names_source}");
        ctx.report_with_fix(
            &Self::META,
            bad_span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(bad_span, replacement.into_bytes())],
            },
        );
    }
}

/// RuboCop's `(const _ _)` pattern: a bare constant or a namespaced
/// constant path (`Foo` / `A::Foo` / `::Foo`), never any other receiver.
fn is_const(node: &Node<'_>) -> bool {
    node.as_constant_read_node().is_some() || node.as_constant_path_node().is_some()
}

/// RuboCop's `#mixin_method?` node matcher applied to the first `send`
/// argument: a `sym` or `str` literal whose value is `include`, `prepend`,
/// or `extend`. Returns the mixin method name to use in its place.
fn mixin_method_name(node: &Node<'_>) -> Option<&'static str> {
    if let Some(sym) = node.as_symbol_node() {
        return match sym.unescaped() {
            b"include" => Some("include"),
            b"prepend" => Some("prepend"),
            b"extend" => Some("extend"),
            _ => None,
        };
    }

    if let Some(s) = node.as_string_node() {
        return match s.unescaped() {
            b"include" => Some("include"),
            b"prepend" => Some("prepend"),
            b"extend" => Some("extend"),
            _ => None,
        };
    }

    None
}
