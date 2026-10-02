//! `Style/NumberedParameters`, ported from RuboCop's
//! `lib/rubocop/cop/style/numbered_parameters.rb`.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG_DISALLOW`.
const MSG_DISALLOW: &str = "Avoid using numbered parameters.";
/// RuboCop's `MSG_MULTI_LINE`.
const MSG_MULTI_LINE: &str = "Avoid using numbered parameters for multi-line blocks.";

/// `ConfigurableEnforcedStyle`'s `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    AllowSingleLine,
    Disallow,
}

/// Restrict the usage of numbered parameters.
#[derive(Debug, Clone)]
pub struct NumberedParameters {
    style: Style,
    /// `minimum_target_ruby_version 2.7`.
    enabled: bool,
}

impl Rule for NumberedParameters {
    const META: RuleMeta = RuleMeta {
        name: "Style/NumberedParameters",
        department: Department::Style,
        summary: "Restrict the usage of numbered parameters.",
        explanation: "Checks for numbered parameters.\n\nIt can either restrict the use of \
            numbered parameters to single-lined blocks, or disallow completely numbered \
            parameters.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::BlockNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("allow_single_line"),
            allowed: &["allow_single_line", "disallow"],
            doc: "Whether numbered parameters are restricted to single-line blocks, or \
                  disallowed entirely.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "disallow" => Style::Disallow,
            _ => Style::AllowSingleLine,
        };
        Ok(Self { style, enabled: options.target_ruby_version() >= 2.7 })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.enabled {
            return;
        }
        let block = node.as_block_node().expect("kind matched");
        let Some(parameters) = block.parameters() else { return };
        if parameters.as_numbered_parameters_node().is_none() {
            return;
        }
        // whitequark's `numblock` node wraps the whole call, matching
        // Prism's `CallNode` (always this block's parent); `multiline?`
        // still reads the `BlockNode`'s own span (KIT: "upstream measures
        // the block node only").
        let span = ctx.parent().map_or_else(|| node.span(), |p| p.span);

        if self.style == Style::Disallow {
            ctx.report(&Self::META, span, MSG_DISALLOW);
        } else if !ctx.is_single_line(node.span()) {
            ctx.report(&Self::META, span, MSG_MULTI_LINE);
        }
    }
}
