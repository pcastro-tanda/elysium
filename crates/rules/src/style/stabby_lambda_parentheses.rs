//! `Style/StabbyLambdaParentheses`, ported from RuboCop's
//! `lib/rubocop/cop/style/stabby_lambda_parentheses.rb`.
//!
//! Whitequark represents a stabby lambda `-> ... { }` as a `block` node
//! wrapping a `(send nil :lambda)`, gated by `RESTRICT_ON_SEND = %i[lambda]`
//! plus `send_node.lambda_literal?`. Prism instead gives `->` its own
//! dedicated [`ruby_ast::node::LambdaNode`], never a `CallNode`/`BlockNode`
//! pair, so this rule subscribes to `LambdaNode` directly: it naturally never
//! fires on `lambda { }`/`lambda(&:nil?)` (a `CallNode`) or `o.lambda` (also
//! a `CallNode`), matching upstream's `does not check the old lambda syntax`
//! and `does not check a method call named lambda` examples for free.
//!
//! `node.block_node.arguments` (whitequark's `args` node, present even when
//! empty for a `-> { }` block) is Prism's `LambdaNode::parameters()`: `None`
//! for `-> { }` (skipped, matching upstream's `stabby_lambda_with_args?`
//! guard), otherwise a [`ruby_ast::node::BlockParametersNode`] whose
//! `opening_loc`/`closing_loc` are `Some` when the arguments are
//! parenthesized (`->(a,b,c) { }`) and `None` when bare (`->a,b,c { }`) --
//! exactly upstream's `parentheses?` (`arguments.loc.begin`) check. The
//! offense span is the `BlockParametersNode`'s own span, matching upstream's
//! `add_offense(arguments)`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

const MSG_REQUIRE: &str = "Wrap stabby lambda arguments with parentheses.";
const MSG_NO_REQUIRE: &str = "Do not wrap stabby lambda arguments with parentheses.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    RequireParentheses,
    RequireNoParentheses,
}

/// Checks for parentheses around stabby lambda arguments.
#[derive(Debug, Clone)]
pub struct StabbyLambdaParentheses {
    style: Style,
}

impl Rule for StabbyLambdaParentheses {
    const META: RuleMeta = RuleMeta {
        name: "Style/StabbyLambdaParentheses",
        department: Department::Style,
        summary: "Checks for the usage of parentheses around stabby lambda arguments.",
        explanation: "Checks for parentheses around stabby lambda arguments. There are two \
            different styles. Defaults to `require_parentheses`.\n\n\
            # bad\n\
            ->a,b,c { a + b + c }\n\n\
            # good\n\
            ->(a,b,c) { a + b + c}\n\n\
            # EnforcedStyle: require_no_parentheses\n\n\
            # bad\n\
            ->(a,b,c) { a + b + c }\n\n\
            # good\n\
            ->a,b,c { a + b + c}",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::LambdaNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("require_parentheses"),
            allowed: &["require_parentheses", "require_no_parentheses"],
            doc: "Whether to require parentheses around stabby lambda arguments.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "require_no_parentheses" => Style::RequireNoParentheses,
            _ => Style::RequireParentheses,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let lambda = node.as_lambda_node().expect("kind matched");
        let Some(params) = lambda.parameters() else { return };
        let Some(block_params) = params.as_block_parameters_node() else { return };
        let has_parens = block_params.opening_loc().is_some();

        let (message, fix) = match self.style {
            Style::RequireParentheses if !has_parens => {
                let span = block_params.as_node().location().span();
                let fix = Fix {
                    applicability: Applicability::Safe,
                    edits: vec![
                        Edit::insert(span.start, b"(".to_vec()),
                        Edit::insert(span.end, b")".to_vec()),
                    ],
                };
                (MSG_REQUIRE, fix)
            }
            Style::RequireNoParentheses if has_parens => {
                let opening = block_params.opening_loc().expect("has_parens checked").span();
                let closing = block_params.closing_loc().expect("has_parens checked").span();
                let fix = Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::delete(opening), Edit::delete(closing)],
                };
                (MSG_NO_REQUIRE, fix)
            }
            _ => return,
        };

        let span = block_params.as_node().location().span();
        ctx.report_with_fix(&Self::META, span, message, fix);
    }
}
