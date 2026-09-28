//! `Security/MarshalLoad`, ported from RuboCop's
//! `lib/rubocop/cop/security/marshal_load.rb`.
//!
//! # Node-pattern arity
//!
//! Upstream's matcher, `(send (const {nil? cbase} :Marshal) ${:load
//! :restore} !(send (const {nil? cbase} :Marshal) :dump ...))`, has no
//! trailing `...` after the negated argument slot, so it only matches a
//! `Marshal.load`/`Marshal.restore` call with *exactly one* argument (zero
//! or two-or-more arguments simply fail to match the pattern's fixed
//! arity, and so are never flagged); [`marshal_load`] mirrors that by
//! requiring exactly one positional argument. The nested `Marshal.dump`
//! exemption, in contrast, does end in `...`, so it matches a `dump` call
//! with any number of arguments (including zero).

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{ext, LocationExt as _, Node, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Avoid using `Marshal.{}`.";

/// Checks for the use of `Marshal` class methods which have potential
/// security issues leading to remote code execution when loading from an
/// untrusted source.
#[derive(Debug, Clone)]
pub struct MarshalLoad;

impl Rule for MarshalLoad {
    const META: RuleMeta = RuleMeta {
        name: "Security/MarshalLoad",
        department: Department::Security,
        summary: "Checks for the use of `Marshal` class methods which have potential security \
                   issues.",
        explanation: "\
Checks for the use of Marshal class methods which have
potential security issues leading to remote code execution when
loading from an untrusted source.

```ruby
# bad
Marshal.load(\"{}\")
Marshal.restore(\"{}\")

# good
Marshal.dump(\"{}\")

# okish - deep copy hack
Marshal.load(Marshal.dump({}))
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
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
        let Some(method) = marshal_load(&call) else { return };
        let selector = call.message_loc().map_or_else(|| call.location().span(), |loc| loc.span());
        ctx.report(&Self::META, selector, MSG.replacen("{}", method, 1));
    }
}

/// RuboCop's `marshal_load` node-matcher, applied to one call: `Some(name)`
/// (`"load"` or `"restore"`) when `call` is a bare-or-toplevel-qualified
/// `Marshal.load`/`Marshal.restore` with exactly one argument that is not
/// itself a bare-or-toplevel-qualified `Marshal.dump(...)` call.
fn marshal_load(call: &CallNode<'_>) -> Option<&'static str> {
    let method = match call.name().as_slice() {
        b"load" => "load",
        b"restore" => "restore",
        _ => return None,
    };
    let receiver = call.receiver()?;
    if !is_marshal_const(&receiver) {
        return None;
    }
    let arguments = call.arguments()?;
    let arg_list = arguments.arguments();
    if arg_list.len() != 1 {
        return None;
    }
    let argument = arg_list.first()?;
    if is_marshal_dump(&argument) {
        return None;
    }
    Some(method)
}

/// `is_marshal_dump` recognizes `(send (const {nil? cbase} :Marshal) :dump
/// ...)`: a bare-or-toplevel-qualified `Marshal.dump` call with any number
/// of arguments.
fn is_marshal_dump(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.name().as_slice() != b"dump" {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    is_marshal_const(&receiver)
}

/// `(const {nil? cbase} :Marshal)`: a bare or top-level-qualified `Marshal`
/// constant reference.
fn is_marshal_const(node: &Node<'_>) -> bool {
    ext::is_bare_or_toplevel_const(node)
        && ext::const_name(node).is_some_and(|name| name == "Marshal")
}
