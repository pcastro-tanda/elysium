//! `Style/Proc`, ported from RuboCop's
//! `lib/rubocop/cop/style/proc.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{call_span_excluding_block, const_name};
use ruby_ast::{Node, NodeKind};

const MSG: &str = "Use `proc` instead of `Proc.new`.";

/// Use proc instead of Proc.new.
#[derive(Debug, Clone)]
pub struct Proc;

impl Rule for Proc {
    const META: RuleMeta = RuleMeta {
        name: "Style/Proc",
        department: Department::Style,
        summary: "Use proc instead of Proc.new.",
        explanation: "Checks for uses of `Proc.new` where `Kernel#proc`\n\
            would be more appropriate. `proc` is the shorter and\n\
            more idiomatic way to create procs in Ruby.\n\n\
            ```ruby\n\
            # bad\n\
            p = Proc.new { |n| puts n }\n\n\
            # good\n\
            p = proc { |n| puts n }\n\
            ```",
        enabled_by_default: true,
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
        if call.block().is_none() {
            return;
        }
        if call.name().as_slice() != b"new" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        if const_name(&receiver).as_deref() != Some("Proc") {
            return;
        }

        let span = call_span_excluding_block(&call);
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, b"proc".to_vec())],
            },
        );
    }
}
