//! `Metrics/PerceivedComplexity`, ported from RuboCop's
//! `lib/rubocop/cop/metrics/perceived_complexity.rb`, a subclass of
//! `Metrics::CyclomaticComplexity` that swaps `when`/`in_pattern` for
//! `case`/`case_match` in the counted node list and charges an `if` with a
//! real `else` twice.
//!
//! The scoring lives in [`crate::metrics::util::perceived`].

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeKind};

use super::util::{complexity_target, it_is_parameter, perceived, AllowedNames};

/// Checks that the perceived complexity of methods is not higher than the configured maximum.
#[derive(Debug, Clone)]
pub struct PerceivedComplexity {
    max: i64,
    allowed: AllowedNames,
    it_parameter: bool,
}

impl Rule for PerceivedComplexity {
    const META: RuleMeta = RuleMeta {
        name: "Metrics/PerceivedComplexity",
        department: Department::Metrics,
        summary: "Checks that the perceived complexity of methods is not higher than the configured maximum.",
        explanation: "Tries to produce a complexity score that's a measure of the complexity the \
            reader experiences when looking at a method. For that reason it considers `when` \
            nodes as something that doesn't add as much complexity as an `if` or a `&&`. Except \
            if it's one of those special `case`/`when` constructs where there's no expression \
            after `case`. Then the cop treats it as an `if`/`elsif`/`elsif`... and lets all the \
            `when` nodes count. In contrast to the `CyclomaticComplexity` cop, this cop \
            considers `else` nodes as adding complexity.\n\n\
            A `case`/`in` branch whose pattern is a simple literal (e.g. `in 1`, `in \"red\"`, \
            `in 1..10`) or a constant/type (e.g. `in Integer`) and has no guard is just as easy \
            to read as a `when` branch, so it is discounted the same way. Branches with \
            structural patterns (e.g. array, hash, or find patterns), bindings, alternatives, \
            or a guard add the full complexity of a decision point.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode, NodeKind::CallNode],
        config: &[
            ConfigOption {
                name: "Max",
                default: ConfigDefault::Int(8),
                allowed: &[],
                doc: "Maximum perceived complexity allowed.",
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
        let score = perceived(&target.body, self.it_parameter);
        if i64::from(score) <= self.max {
            return;
        }
        let name = String::from_utf8_lossy(target.name);
        let max = self.max;
        ctx.report(
            &Self::META,
            target.span,
            format!("Perceived complexity for `{name}` is too high. [{score}/{max}]"),
        );
    }
}
