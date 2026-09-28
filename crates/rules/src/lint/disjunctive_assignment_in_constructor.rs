//! `Lint/DisjunctiveAssignmentInConstructor`, ported from RuboCop's
//! `lib/rubocop/cop/lint/disjunctive_assignment_in_constructor.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

const MSG: &str = "Unnecessary disjunctive assignment. Use plain assignment.";

/// In constructor, plain assignment is preferred over disjunctive.
#[derive(Debug, Clone)]
pub struct DisjunctiveAssignmentInConstructor;

impl Rule for DisjunctiveAssignmentInConstructor {
    const META: RuleMeta = RuleMeta {
        name: "Lint/DisjunctiveAssignmentInConstructor",
        department: Department::Lint,
        summary: "In constructor, plain assignment is preferred over disjunctive.",
        explanation: "Checks constructors for disjunctive assignments (`||=`) that should \
                      be plain assignments.\n\nSo far, this cop is only concerned with \
                      disjunctive assignment of instance variables.\n\nIn ruby, an instance \
                      variable is nil until a value is assigned, so the disjunction is \
                      unnecessary. A plain assignment has the same effect.",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(def) = node.as_def_node() else { return };
        if def.name().as_slice() != b"initialize" {
            return;
        }
        let Some(body) = def.body() else { return };
        let lines: Vec<Node<'_>> = match body.as_statements_node() {
            Some(stmts) => stmts.body().iter().collect(),
            None => vec![body],
        };
        for line in lines {
            let Some(or_write) = line.as_instance_variable_or_write_node() else { break };
            let op_span = or_write.operator_loc().span();
            ctx.report_with_fix(
                &Self::META,
                op_span,
                MSG,
                Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::replace(op_span, b"=".to_vec())],
                },
            );
        }
    }
}
