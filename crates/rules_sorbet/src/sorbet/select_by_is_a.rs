//! `Sorbet/SelectByIsA`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/select_by_is_a.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Use `grep` instead of `select` when using it only for type narrowing.";

/// Suggests using `grep` over `select` when using it only for type narrowing.
#[derive(Debug, Clone)]
pub struct SelectByIsA;

impl Rule for SelectByIsA {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/SelectByIsA",
        department: Department::Sorbet,
        summary: "Suggests using `grep` over `select` when using it only for type narrowing. This is because Sorbet can properly infer types when using `grep` but not with `select`.",
        explanation: "Suggests using `grep` over `select` when using it only for type narrowing.\n\n```ruby\n# bad\nstrings_or_integers.select { |e| e.is_a?(String) }\nstrings_or_integers.filter { |e| e.is_a?(String) }\nstrings_or_integers.select { |e| e.kind_of?(String) }\n\n# good\nstrings_or_integers.grep(String)\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if !matches!(call.name().as_slice(), b"select" | b"filter") {
            return;
        }
        let Some(block_node) = call.block() else { return };
        let Some(block) = block_node.as_block_node() else { return };
        // `(call _ {:select :filter})`: no arguments.
        if call.arguments().is_some() {
            return;
        }
        let Some(receiver) = call.receiver() else { return };

        // block: `(args (arg _))`; numblock/itblock: any parameters.
        let Some(parameters) = block.parameters() else { return };
        match &parameters {
            Node::BlockParametersNode { .. } => {
                let Some(block_parameters) = parameters.as_block_parameters_node() else {
                    return;
                };
                if block_parameters.locals().iter().next().is_some() {
                    return;
                }
                let Some(params) = block_parameters.parameters() else { return };
                let mut requireds = params.requireds().iter();
                let single_arg = matches!(
                    (requireds.next(), requireds.next()),
                    (Some(Node::RequiredParameterNode { .. }), None)
                );
                if !single_arg
                    || !params.optionals().is_empty()
                    || params.rest().is_some()
                    || !params.posts().is_empty()
                    || !params.keywords().is_empty()
                    || params.keyword_rest().is_some()
                    || params.block().is_some()
                {
                    return;
                }
            }
            Node::NumberedParametersNode { .. } | Node::ItParametersNode { .. } => {}
            _ => return,
        }

        // body: `(send (lvar _) {:is_a? :kind_of?} (const nil? _))`.
        let Some(body) = block.body() else { return };
        let Some(statements) = body.as_statements_node() else { return };
        let mut stmts = statements.body().iter();
        let (Some(only), None) = (stmts.next(), stmts.next()) else { return };
        let Some(check) = only.as_call_node() else { return };
        if check.is_safe_navigation()
            || !matches!(check.name().as_slice(), b"is_a?" | b"kind_of?")
            || check.block().is_some()
        {
            return;
        }
        if !matches!(
            check.receiver(),
            Some(Node::LocalVariableReadNode { .. } | Node::ItLocalVariableReadNode { .. })
        ) {
            return;
        }
        let Some(check_args) = check.arguments() else { return };
        let mut args = check_args.arguments().iter();
        let (Some(type_class), None) = (args.next(), args.next()) else { return };
        if !matches!(type_class, Node::ConstantReadNode { .. }) {
            return;
        }

        let navigation: &[u8] = if call.is_safe_navigation() { b"&." } else { b"." };
        let mut replacement = ctx.text(receiver.span()).to_vec();
        replacement.extend_from_slice(navigation);
        replacement.extend_from_slice(b"grep(");
        replacement.extend_from_slice(ctx.text(type_class.span()));
        replacement.push(b')');
        let span = node.span();
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
