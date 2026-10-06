//! `Rails/DurationArithmetic`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/duration_arithmetic.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{call_span_excluding_block, const_name, is_bare_or_toplevel_const};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Do not add or subtract duration.";

const DURATIONS: &[&[u8]] = &[
    b"second",
    b"seconds",
    b"minute",
    b"minutes",
    b"hour",
    b"hours",
    b"day",
    b"days",
    b"week",
    b"weeks",
    b"fortnight",
    b"fortnights",
    b"month",
    b"months",
    b"year",
    b"years",
];

/// Checks if a duration is added to or subtracted from `Time.current`.
#[derive(Debug, Clone)]
pub struct DurationArithmetic;

impl Rule for DurationArithmetic {
    const META: RuleMeta = RuleMeta {
        name: "Rails/DurationArithmetic",
        department: Department::Rails,
        summary: "Do not use duration as arithmetic operand with `Time.current`.",
        explanation: "Checks if a duration is added to or subtracted from `Time.current`.\n\n\
                      ```ruby\n# bad\nTime.current - 1.minute\nTime.zone.now + 2.days\n\n\
                      # good\n1.minute.ago\n2.days.from_now\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
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
        let name = call.name();
        let name = name.as_slice();
        if (name != b"+" && name != b"-") || call.is_safe_navigation() {
            return;
        }
        // `(send #time_current? ${ :+ :- } $#duration?)`: exactly one argument.
        if call.block().is_some_and(|block| block.as_block_argument_node().is_some()) {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        if !is_time_current(&receiver) {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let mut arguments = arguments.arguments().iter();
        let (Some(duration), None) = (arguments.next(), arguments.next()) else { return };
        if !is_duration(&duration) {
            return;
        }
        let span = call_span_excluding_block(&call);
        let suffix = if name == b"-" { "ago" } else { "from_now" };
        let mut replacement = ctx.text(duration.span()).to_vec();
        replacement.push(b'.');
        replacement.extend_from_slice(suffix.as_bytes());
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, replacement)],
            },
        );
    }
}

/// A plain `send`: a call with neither safe navigation nor a block.
fn plain_call<'a>(node: &Node<'a>) -> Option<CallNode<'a>> {
    let call = node.as_call_node()?;
    (!call.is_safe_navigation() && call.block().is_none()).then_some(call)
}

fn is_time_const(node: &Node<'_>) -> bool {
    is_bare_or_toplevel_const(node) && const_name(node).as_deref() == Some("Time")
}

/// `{(send (const {nil? cbase} :Time) :current)
///   (send (send (const {nil? cbase} :Time) :zone) :now)}`.
fn is_time_current(node: &Node<'_>) -> bool {
    let Some(call) = plain_call(node) else { return false };
    if call.arguments().is_some() {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    match call.name().as_slice() {
        b"current" => is_time_const(&receiver),
        b"now" => plain_call(&receiver).is_some_and(|zone| {
            zone.name().as_slice() == b"zone"
                && zone.arguments().is_none()
                && zone.receiver().is_some_and(|time| is_time_const(&time))
        }),
        _ => false,
    }
}

/// `(send { int float (send nil _) } DURATIONS)`.
fn is_duration(node: &Node<'_>) -> bool {
    let Some(call) = plain_call(node) else { return false };
    if call.arguments().is_some() || !DURATIONS.contains(&call.name().as_slice()) {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    match receiver.kind() {
        NodeKind::IntegerNode | NodeKind::FloatNode => true,
        NodeKind::CallNode => plain_call(&receiver)
            .is_some_and(|inner| inner.receiver().is_none() && inner.arguments().is_none()),
        _ => false,
    }
}
