//! `Style/EmptyLambdaParameter`, ported from RuboCop's
//! `lib/rubocop/cop/style/empty_lambda_parameter.rb` (mixing in
//! `EmptyParameter`).
//!
//! Whitequark represents a stabby lambda `-> ... { }` as a `block` node
//! wrapping a `(send nil :lambda)`, gated by `send_node.lambda_literal?`.
//! Prism instead gives `->` its own dedicated
//! [`ruby_ast::node::LambdaNode`], never a `CallNode`/`BlockNode` pair, so
//! this rule subscribes to `LambdaNode` directly and skips the
//! `lambda_literal?`/`send_node` guards entirely.
//!
//! Upstream's `empty_arguments?` node-matcher is `(block _ $(args) _)`,
//! matching any (possibly empty) args node, and `empty_and_without_delimiters?`
//! then filters out the parameter-less shape with no `()`/`||` at all (e.g.
//! `-> { }`). In Prism, an explicit (possibly empty) parameter list is a
//! [`ruby_ast::node::BlockParametersNode`] with `opening_loc`/`closing_loc`
//! set to the delimiters; `LambdaNode::parameters()` being `None` is the
//! delimiter-less case (`-> { }`), which this rule also skips. A stabby
//! lambda's parameters are always parenthesized when present (Ruby doesn't
//! allow `->x { }` or `->|x| { }`), so an empty `BlockParametersNode` here is
//! always the `-> () { }` shape upstream flags.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

const MSG: &str = "Omit parentheses for the empty lambda parameters.";

/// Omit parens for empty lambda parameters.
#[derive(Debug, Clone)]
pub struct EmptyLambdaParameter;

impl Rule for EmptyLambdaParameter {
    const META: RuleMeta = RuleMeta {
        name: "Style/EmptyLambdaParameter",
        department: Department::Style,
        summary: "Omit parens for empty lambda parameters.",
        explanation: "Checks for parentheses for empty lambda parameters. Parentheses \
            for empty lambda parameters do not cause syntax errors, but they are \
            redundant.\n\n\
            # bad\n\
            -> () { do_something }\n\n\
            # good\n\
            -> { do_something }\n\n\
            # good\n\
            -> (arg) { do_something(arg) }",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::LambdaNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let lambda = node.as_lambda_node().expect("kind matched");
        let Some(params) = lambda.parameters() else { return };
        let Some(block_params) = params.as_block_parameters_node() else { return };
        if block_params.opening_loc().is_none() {
            return;
        }
        if block_params.parameters().is_some() || !block_params.locals().is_empty() {
            return;
        }
        let Some(opening) = block_params.opening_loc() else { return };
        let Some(closing) = block_params.closing_loc() else { return };
        let span = Span::new(opening.span().start, closing.span().end);
        let remove_from = lambda.operator_loc().span().end;
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::delete(Span::new(remove_from, span.end))],
        };
        ctx.report_with_fix(&Self::META, span, MSG, fix);
    }
}
