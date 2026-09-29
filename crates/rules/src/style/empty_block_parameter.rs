//! `Style/EmptyBlockParameter`, ported from RuboCop's
//! `lib/rubocop/cop/style/empty_block_parameter.rb`.
//!
//! Upstream's `include EmptyParameter` matches `(block _ $(args) _)` where
//! the node-pattern literal `(args)` requires the parameters node to have
//! *zero children* -- i.e. no positional/keyword/rest params and no
//! trailing `; shadow` block-locals -- then skips further if that args
//! node has no source location at all (`empty_and_without_delimiters?`,
//! true only when the block has no `||` clause whatsoever). In Prism this
//! is: `BlockNode::parameters()` is `Some(BlockParametersNode)` (pipes
//! exist), its inner `parameters()` is `None` and `locals()` is empty (no
//! actual params, no shadow locals). A block with no parameters clause at
//! all (`parameters()` is `None`) never reaches here. `->(){ }` stabby
//! lambdas parse as a distinct `LambdaNode`, so restricting `kinds` to
//! `BlockNode` alone already reproduces upstream's `send_node.lambda_literal?`
//! guard for free -- no separate check needed.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

const MSG: &str = "Omit pipes for the empty block parameters.";

/// Omit pipes for empty block parameters.
#[derive(Debug, Clone)]
pub struct EmptyBlockParameter;

impl Rule for EmptyBlockParameter {
    const META: RuleMeta = RuleMeta {
        name: "Style/EmptyBlockParameter",
        department: Department::Style,
        summary: "Omit pipes for empty block parameters.",
        explanation: "Checks for pipes for empty block parameters. Pipes for empty \
            block parameters do not cause syntax errors, but they are redundant.\n\n\
            ```ruby\n\
            # bad\n\
            a do ||\n\
              do_something\n\
            end\n\n\
            # bad\n\
            a { || do_something }\n\n\
            # good\n\
            a do\n\
            end\n\n\
            # good\n\
            a { do_something }\n\
            ```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::BlockNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(block) = node.as_block_node() else { return };
        let Some(params) = block.parameters() else { return };
        let Some(block_params) = params.as_block_parameters_node() else { return };
        if block_params.parameters().is_some() || !block_params.locals().is_empty() {
            return;
        }
        let Some(opening) = block_params.opening_loc() else { return };
        let Some(closing) = block_params.closing_loc() else { return };
        let args_span = Span::new(opening.span().start, closing.span().end);

        let delete_span = Span::new(block.opening_loc().span().end, args_span.end);
        ctx.report_with_fix(
            &Self::META,
            args_span,
            MSG,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(delete_span)] },
        );
    }
}
