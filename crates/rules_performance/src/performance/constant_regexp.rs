//! `Performance/ConstantRegexp`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/constant_regexp.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str =
    "Extract this regexp into a constant, memoize it, or append an `/o` option to its options.";

/// Finds regular expressions with dynamic components that are all constants.
#[derive(Debug, Clone)]
pub struct ConstantRegexp;

fn is_const(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode)
}

/// `(send (const nil? :Regexp) :escape const_type?)`
fn is_regexp_escape(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.is_safe_navigation() || call.name().as_slice() != b"escape" || call.block().is_some() {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    let Some(constant) = receiver.as_constant_read_node() else { return false };
    if constant.name().as_slice() != b"Regexp" {
        return false;
    }
    let Some(arguments) = call.arguments() else { return false };
    let mut it = arguments.arguments().iter();
    matches!((it.next(), it.next()), (Some(arg), None) if is_const(&arg))
}

fn include_interpolated_const(parts: &[Node<'_>]) -> bool {
    let mut interpolated = false;
    for part in parts {
        let Some(embedded) = part.as_embedded_statements_node() else { continue };
        interpolated = true;
        let Some(statements) = embedded.statements() else { return false };
        let Some(inner) = statements.body().iter().next() else { return false };
        if !(is_const(&inner) || is_regexp_escape(&inner)) {
            return false;
        }
    }
    interpolated
}

impl Rule for ConstantRegexp {
    const META: RuleMeta = RuleMeta {
        name: "Performance/ConstantRegexp",
        department: Department::Performance,
        summary: "Finds regular expressions with dynamic components that are all constants.",
        explanation: "\
Finds regular expressions with dynamic components that are all constants.

Ruby allocates a new Regexp object every time it executes a code containing such
a regular expression. It is more efficient to extract it into a constant,
memoize it, or add an `/o` option to perform `#{}` interpolation only once and
reuse that Regexp object.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::InterpolatedRegularExpressionNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(regexp) = node.as_interpolated_regular_expression_node() else { return };
        if regexp.is_once() {
            return;
        }
        let allowed = ctx.ancestors().iter().any(|a| {
            matches!(
                a.kind,
                NodeKind::ConstantWriteNode
                    | NodeKind::ConstantPathWriteNode
                    | NodeKind::LocalVariableOrWriteNode
                    | NodeKind::InstanceVariableOrWriteNode
                    | NodeKind::ClassVariableOrWriteNode
                    | NodeKind::GlobalVariableOrWriteNode
                    | NodeKind::ConstantOrWriteNode
                    | NodeKind::ConstantPathOrWriteNode
                    | NodeKind::CallOrWriteNode
                    | NodeKind::IndexOrWriteNode
            )
        });
        if allowed {
            return;
        }
        let parts: Vec<Node<'_>> = regexp.parts().iter().collect();
        if !include_interpolated_const(&parts) {
            return;
        }
        let span = node.span();
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::insert(span.end, b"o".to_vec())],
        };
        ctx.report_with_fix(&Self::META, span, MSG, fix);
    }
}
