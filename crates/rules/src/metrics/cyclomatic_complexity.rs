//! `Metrics/CyclomaticComplexity`, ported from RuboCop's
//! `lib/rubocop/cop/metrics/cyclomatic_complexity.rb` and the
//! `MethodComplexity` mixin it includes.
//!
//! The scoring itself lives in [`crate::metrics::util::cyclomatic`], shared
//! with `Metrics/PerceivedComplexity`, which subclasses this cop upstream.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeKind};

use super::util::{complexity_target, cyclomatic, it_is_parameter, AllowedNames};

/// Checks that the cyclomatic complexity of methods is not higher than the configured maximum.
#[derive(Debug, Clone)]
pub struct CyclomaticComplexity {
    max: i64,
    allowed: AllowedNames,
    it_parameter: bool,
}

impl Rule for CyclomaticComplexity {
    const META: RuleMeta = RuleMeta {
        name: "Metrics/CyclomaticComplexity",
        department: Department::Metrics,
        summary: "Checks that the cyclomatic complexity of methods is not higher than the configured maximum.",
        explanation: "The cyclomatic complexity is the number of linearly independent paths \
            through a method. The algorithm counts decision points and adds one.\n\n\
            An `if` statement (or `unless` or `?:`) increases the complexity by one. An `else` \
            branch does not, since it doesn't add a decision point. The `&&` operator (or \
            keyword `and`) can be converted to a nested `if` statement, and `||`/`or` is \
            shorthand for a sequence of `if`s, so they also add one. Loops can be said to have \
            an exit condition, so they add one. Blocks that are calls to builtin iteration \
            methods (e.g. `ary.map { ... }`) also add one, others are ignored.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode, NodeKind::CallNode],
        config: &[
            ConfigOption {
                name: "Max",
                default: ConfigDefault::Int(7),
                allowed: &[],
                doc: "Maximum cyclomatic complexity allowed.",
            },
            ConfigOption {
                name: "AllowedMethods",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Method names to exempt.",
            },
            ConfigOption {
                name: "AllowedPatterns",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Patterns matching method names to exempt.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            max: options.int("Max"),
            allowed: AllowedNames::new(options),
            it_parameter: it_is_parameter(options.target_ruby_version()),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(target) = complexity_target(node) else { return };
        if self.allowed.allows(target.name) {
            return;
        }
        let score = cyclomatic(&target.body, self.it_parameter);
        if i64::from(score) <= self.max {
            return;
        }
        let name = String::from_utf8_lossy(target.name);
        let max = self.max;
        ctx.report(
            &Self::META,
            target.span,
            format!("Cyclomatic complexity for `{name}` is too high. [{score}/{max}]"),
        );
    }
}
