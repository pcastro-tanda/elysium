//! `Style/DateTime`, ported from RuboCop's
//! `lib/rubocop/cop/style/date_time.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// RuboCop's `CLASS_MSG`.
const CLASS_MSG: &str = "Prefer `Time` over `DateTime`.";
/// RuboCop's `COERCION_MSG`.
const COERCION_MSG: &str = "Do not use `#to_datetime`.";

/// Use `Time` over `DateTime`.
#[derive(Debug, Clone)]
pub struct DateTime {
    /// `AllowCoercion`.
    allow_coercion: bool,
}

impl Rule for DateTime {
    const META: RuleMeta = RuleMeta {
        name: "Style/DateTime",
        department: Department::Style,
        summary: "Use `Time` over `DateTime`.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "AllowCoercion",
            default: ConfigDefault::Bool(false),
            allowed: &[],
            doc: "Allows `#to_datetime` coercion.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allow_coercion: options.bool("AllowCoercion") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };

        let coercion = is_to_datetime(&call);
        if !is_date_time_const(&call) && (!coercion || self.allow_coercion) {
            return;
        }
        if is_historic_date(&call) {
            return;
        }

        let message = if coercion { COERCION_MSG } else { CLASS_MSG };
        let edits = if coercion { Vec::new() } else { autocorrect(&call) };

        ctx.report_with_fix(
            &Self::META,
            node.span(),
            message,
            Fix { applicability: Applicability::Unsafe, edits },
        );
    }
}

/// `date_time?`: a call on a bare or top-level-qualified `DateTime`
/// constant.
fn is_date_time_const(call: &CallNode<'_>) -> bool {
    let Some(receiver) = call.receiver() else { return false };
    is_bare_or_toplevel_const(&receiver, b"DateTime")
}

/// `to_datetime?`: `!nil? :to_datetime` -- any call (with a receiver) named
/// `to_datetime`.
fn is_to_datetime(call: &CallNode<'_>) -> bool {
    call.receiver().is_some() && call.name().as_slice() == b"to_datetime"
}

/// `historic_date?`: `(call _ _ _ (const (const {nil? (cbase)} :Date) _))`
/// -- a call with exactly 2 positional arguments whose last is a
/// `Date::SOMETHING` constant path.
fn is_historic_date(call: &CallNode<'_>) -> bool {
    let Some(args) = call.arguments() else { return false };
    let args = args.arguments();
    if args.len() != 2 {
        return false;
    }
    let Some(last) = args.iter().last() else { return false };
    let Some(path) = last.as_constant_path_node() else { return false };
    let Some(parent) = path.parent() else { return false };
    is_bare_or_toplevel_const(&parent, b"Date")
}

/// `(const {nil? (cbase)} :Name)`: a bare or `::`-qualified constant read
/// named `name`.
fn is_bare_or_toplevel_const(node: &Node<'_>, name: &[u8]) -> bool {
    if let Some(read) = node.as_constant_read_node() {
        return read.name().as_slice() == name;
    }
    if let Some(path) = node.as_constant_path_node() {
        return path.parent().is_none() && path.name().is_some_and(|n| n.as_slice() == name);
    }
    false
}

/// `autocorrect`: replaces the receiver's constant name with `Time`
/// (`corrector.replace(node.receiver.loc.name, 'Time')`).
fn autocorrect(call: &CallNode<'_>) -> Vec<Edit> {
    let Some(receiver) = call.receiver() else { return Vec::new() };
    let name_span = if receiver.as_constant_read_node().is_some() {
        receiver.span()
    } else if let Some(path) = receiver.as_constant_path_node() {
        path.name_loc().span()
    } else {
        return Vec::new();
    };
    vec![Edit::replace(name_span, b"Time".to_vec())]
}
