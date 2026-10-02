//! `Rails/FreezeTime`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/freeze_time.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{call_span_excluding_block, const_name, is_bare_or_toplevel_const};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Use `freeze_time` instead of `travel_to`.";

/// `minimum_target_rails_version 5.2`.
const MINIMUM_TARGET_RAILS_VERSION: f64 = 5.2;

/// Prefer `freeze_time` over `travel_to` with an argument of the current time.
#[derive(Debug, Clone)]
pub struct FreezeTime {
    /// Whether the target Rails version reaches `minimum_target_rails_version`;
    /// RuboCop does not run the cop at all otherwise.
    supported: bool,
}

impl Rule for FreezeTime {
    const META: RuleMeta = RuleMeta {
        name: "Rails/FreezeTime",
        department: Department::Rails,
        summary: "Prefer `freeze_time` over `travel_to` with an argument of the current time.",
        explanation: "Identifies usages of `travel_to` with an argument of the current time and \
                      change them to use `freeze_time` instead.\n\nThis cop's autocorrection is \
                      unsafe because `freeze_time` just delegates to `travel_to` with a default \
                      `Time.now`, it is not strictly equivalent to `Time.now` if the argument of \
                      `travel_to` is the current time considering time zone.\n\n```ruby\n# \
                      bad\ntravel_to(Time.now)\ntravel_to(Time.new)\ntravel_to(DateTime.now)\n\
                      travel_to(Time.current)\ntravel_to(Time.zone.now)\n\
                      travel_to(Time.now.in_time_zone)\ntravel_to(Time.current.to_time)\n\n# \
                      good\nfreeze_time\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { supported: options.target_rails_version() >= MINIMUM_TARGET_RAILS_VERSION })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"travel_to" {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let Some(first) = arguments.arguments().iter().next() else { return };
        // `child_node, method_name, time_argument = *first_argument.children`:
        // a call with a receiver and no arguments.
        let Some(inner) = bare_receiver_call(&first) else { return };
        let Some(child) = inner.receiver() else { return };
        let name = inner.name();
        let name = name.as_slice();
        if !(current_time(&child, name) || current_time_with_convert(&child, name)) {
            return;
        }

        let block_pass = call.block().filter(|b| b.as_block_argument_node().is_some());
        let replacement = block_pass.as_ref().map_or_else(
            || b"freeze_time".to_vec(),
            |block| {
                format!("freeze_time({})", String::from_utf8_lossy(ctx.text(block.span())))
                    .into_bytes()
            },
        );
        let mut span = call_span_excluding_block(&call);
        if let Some(block) = &block_pass {
            span = Span::new(span.start, span.end.max(block.span().end));
        }
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, replacement)],
            },
        );
    }
}

/// `node` as a call taking no arguments and no block (so its whitequark
/// children are exactly receiver and name).
fn bare_receiver_call<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    let call = node.as_call_node()?;
    (call.arguments().is_none() && call.block().is_none()).then_some(call)
}

/// `(const {nil? cbase} {:Time :DateTime})` (`time_now?`), or with `zone`
/// (`zoned_time_now?`): `(send (const {nil? cbase} :Time) :zone)`.
fn current_time(node: &Node<'_>, method_name: &[u8]) -> bool {
    if !matches!(method_name, b"now" | b"new" | b"current") {
        return false;
    }
    match node.as_call_node() {
        // `send_type?` excludes `csend`.
        Some(call) => {
            !call.is_safe_navigation()
                && call.name().as_slice() == b"zone"
                && bare_receiver_call(node).is_some()
                && call.receiver().is_some_and(|r| is_time_const(&r, &[b"Time"]))
        }
        None => is_time_const(node, &[b"Time", b"DateTime"]),
    }
}

fn is_time_const(node: &Node<'_>, names: &[&[u8]]) -> bool {
    is_bare_or_toplevel_const(node)
        && const_name(node).is_some_and(|name| names.contains(&name.as_bytes()))
}

fn current_time_with_convert(node: &Node<'_>, method_name: &[u8]) -> bool {
    if !matches!(method_name, b"to_time" | b"in_time_zone") {
        return false;
    }
    let Some(inner) = bare_receiver_call(node) else { return false };
    let Some(child) = inner.receiver() else { return false };
    current_time(&child, inner.name().as_slice())
}
