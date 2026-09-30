//! `Layout/SpaceInLambdaLiteral`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_in_lambda_literal.rb`.
//!
//! RuboCop's `on_send` fires for the `send nil :lambda` node whitequark
//! synthesizes for an arrow lambda literal (`->`), guarded by
//! `lambda_literal?` (its own source text reads `->`) plus
//! `node.parent.arguments?` (the sibling `block` node has parameters).
//! Prism instead gives `->` its own dedicated [`NodeKind::LambdaNode`], with
//! `operator_loc` for the `->` text itself and `parameters` for the
//! parameter list (a `BlockParametersNode`, present with or without
//! parentheses -- `-> x { x }` is valid and gives it no `opening_loc`), so
//! this port subscribes to that node kind directly instead of reconstructing
//! the whitequark send+block shape.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    RequireNoSpace,
    RequireSpace,
}

/// RuboCop's `MSG_REQUIRE_SPACE`.
const MSG_REQUIRE_SPACE: &str = "Use a space between `->` and `(` in lambda literals.";
/// RuboCop's `MSG_REQUIRE_NO_SPACE`.
const MSG_REQUIRE_NO_SPACE: &str = "Do not use spaces between `->` and `(` in lambda literals.";

/// Checks for spaces in lambda literals.
#[derive(Debug, Clone)]
pub struct SpaceInLambdaLiteral {
    style: Style,
}

impl Rule for SpaceInLambdaLiteral {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceInLambdaLiteral",
        department: Department::Layout,
        summary: "Checks for spaces between `->` and opening parameter parenthesis (`(`) in \
                   lambda literals.",
        explanation: "\
```ruby
# bad, EnforcedStyle: require_no_space (default)
a = -> (x, y) { x + y }

# good, EnforcedStyle: require_no_space (default)
a = ->(x, y) { x + y }

# bad, EnforcedStyle: require_space
a = ->(x, y) { x + y }

# good, EnforcedStyle: require_space
a = -> (x, y) { x + y }
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::LambdaNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("require_no_space"),
            allowed: &["require_no_space", "require_space"],
            doc: "Which spacing style to enforce between `->` and `(` in lambda literals.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "require_space" => Style::RequireSpace,
            _ => Style::RequireNoSpace,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let NodeKind::LambdaNode = node.kind() else { return };
        let lambda = node.as_lambda_node().expect("kind matched");
        let Some(params) = lambda.parameters() else { return };

        let arrow_end = lambda.operator_loc().span().end;
        let params_start = params.span().start;
        let gap = Span::new(arrow_end, params_start);
        let has_space = gap.start < gap.end;

        match self.style {
            Style::RequireSpace if !has_space => {
                let offense = Span::new(lambda.operator_loc().span().start, params.span().end);
                ctx.report_with_fix(
                    &Self::META,
                    offense,
                    MSG_REQUIRE_SPACE,
                    Fix {
                        applicability: Applicability::Safe,
                        edits: vec![Edit::insert(params_start, *b" ")],
                    },
                );
            }
            Style::RequireNoSpace if has_space => {
                ctx.report_with_fix(
                    &Self::META,
                    gap,
                    MSG_REQUIRE_NO_SPACE,
                    Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(gap)] },
                );
            }
            _ => {}
        }
    }
}
