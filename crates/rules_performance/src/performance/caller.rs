//! `Performance/Caller`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/caller.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeKind};

/// Use `caller(n..n)` instead of `caller`.
#[derive(Debug, Clone)]
pub struct Caller;

impl Rule for Caller {
    const META: RuleMeta = RuleMeta {
        name: "Performance/Caller",
        department: Department::Performance,
        summary: "Use `caller(n..n)` instead of `caller`.",
        explanation: "Identifies places where `caller[n]` can be replaced by \
                      `caller(n..n).first`.\n\n```ruby\n# bad\ncaller[1]\ncaller.first\n\
                      caller_locations[1]\ncaller_locations.first\n\n# good\n\
                      caller(2..2).first\ncaller(1..1).first\ncaller_locations(2..2).first\n\
                      caller_locations(1..1).first\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "An integer argument outside the 32-bit range is not recognized.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        // `on_send` only (no `on_csend`), and `(send ...)` never matches `&.`.
        if call.is_safe_navigation() {
            return;
        }
        let scope_arg = match (call.name().as_slice(), argument_list(&call).as_slice()) {
            (b"first", []) => None,
            (b"[]", [index]) => {
                let Some(index) = integer_value(index) else { return };
                Some(index)
            }
            _ => return,
        };
        let Some(receiver) = call.receiver().and_then(|receiver| receiver.as_call_node()) else {
            return;
        };
        let Some(method_name) = slow_caller(&receiver) else { return };
        let mut n = match argument_list(&receiver).as_slice() {
            [] => 1,
            [arg] => {
                let Some(value) = integer_value(arg) else { return };
                value
            }
            _ => return,
        };
        if let Some(m) = scope_arg {
            let Some(sum) = n.checked_add(m) else { return };
            n = sum;
        }

        let span = call_span_excluding_block(&call);
        let preferred = format!("{method_name}({n}..{n}).first");
        let message =
            format!("Use `{preferred}` instead of `{}`.", String::from_utf8_lossy(ctx.text(span)));
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, preferred.into_bytes())],
            },
        );
    }
}

/// `(send nil? {:caller :caller_locations} ...)` -- a receiverless call,
/// whose whitequark node is a `send` only without a literal block -- and
/// its name.
fn slow_caller(call: &CallNode<'_>) -> Option<&'static str> {
    if call.receiver().is_some()
        || call.block().is_some_and(|block| block.as_block_node().is_some())
    {
        return None;
    }
    match call.name().as_slice() {
        b"caller" => Some("caller"),
        b"caller_locations" => Some("caller_locations"),
        _ => None,
    }
}

/// The call's arguments the way whitequark lists a `send`'s: a `&block`
/// argument is one of them.
fn argument_list<'pr>(call: &CallNode<'pr>) -> Vec<Node<'pr>> {
    let mut args: Vec<Node<'pr>> =
        call.arguments().map(|args| args.arguments().iter().collect()).unwrap_or_default();
    if let Some(block) = call.block().filter(|block| block.as_block_argument_node().is_some()) {
        args.push(block);
    }
    args
}

/// An `int` literal's value.
fn integer_value(node: &Node<'_>) -> Option<i64> {
    let value: i32 = node.as_integer_node()?.value().try_into().ok()?;
    Some(i64::from(value))
}
