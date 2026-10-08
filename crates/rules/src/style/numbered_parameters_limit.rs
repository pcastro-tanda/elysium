//! `Style/NumberedParametersLimit`, ported from RuboCop's
//! `lib/rubocop/cop/style/numbered_parameters_limit.rb`.
//!
//! `AutoGenConfig`'s `self.max = param_count` bookkeeping (`exclude_limit
//! 'Max'`) is autogen-config tooling with no equivalent in a linter that
//! lacks `.rubocop_todo.yml` generation; it never affects which offense
//! fires, so it is simply omitted.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{each_descendant, Node, NodeExt as _, NodeKind};

/// Ruby does not allow more than 9 numbered parameters.
const MAX_POSSIBLE: i64 = 9;

/// Avoid excessive numbered params in a single block.
#[derive(Debug, Clone)]
pub struct NumberedParametersLimit {
    max: i64,
    /// `minimum_target_ruby_version 2.7`.
    enabled: bool,
}

impl Rule for NumberedParametersLimit {
    const META: RuleMeta = RuleMeta {
        name: "Style/NumberedParametersLimit",
        department: Department::Style,
        summary: "Avoid excessive numbered params in a single block.",
        explanation: "Detects use of an excessive amount of numbered parameters in a single \
            block. Having too many numbered parameters can make code too cryptic and hard to \
            read.\n\nThe cop defaults to registering an offense if there is more than 1 \
            numbered parameter but this maximum can be configured by setting `Max`.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::BlockNode],
        config: &[ConfigOption {
            name: "Max",
            default: ConfigDefault::Int(1),
            allowed: &[],
            doc: "Maximum number of distinct numbered parameters (`_1`..`_9`) a single block \
                  may use.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            max: options.int("Max").min(MAX_POSSIBLE),
            enabled: options.target_ruby_version() >= 2.7,
        })
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
        let Some(body) = block.body() else { return };

        let mut names: Vec<Vec<u8>> = Vec::new();
        each_descendant(&body, &mut |child| {
            let Some(lvar) = child.as_local_variable_read_node() else { return };
            let name = lvar.name().as_slice();
            if is_numbered_parameter(name) && !names.iter().any(|n| n == name) {
                names.push(name.to_vec());
            }
        });
        let count = i64::try_from(names.len()).unwrap_or(i64::MAX);
        if count <= self.max {
            return;
        }

        let parameter = if self.max > 1 { "parameters" } else { "parameter" };
        let message =
            format!("Avoid using more than {} numbered {parameter}; {count} detected.", self.max);
        // whitequark's `numblock` node wraps the whole call; Prism's
        // `BlockNode` only starts at `{`/`do`, so its parent `CallNode`
        // (which always exists, and whose span already extends through the
        // block) is the equivalent offense range.
        let span = ctx.parent().map_or_else(|| node.span(), |p| p.span);
        ctx.report(&Self::META, span, message);
    }
}

/// `NUMBERED_PARAMETER_PATTERN`: `_1` through `_9`.
fn is_numbered_parameter(name: &[u8]) -> bool {
    matches!(name, [b'_', b'1'..=b'9'])
}
