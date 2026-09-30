//! `Metrics/AbcSize`, ported from RuboCop's
//! `lib/rubocop/cop/metrics/abc_size.rb` and the `MethodComplexity` mixin it
//! includes.
//!
//! The calculation lives in [`crate::metrics::util::abc_size`], a port of
//! `Metrics::Utils::AbcSizeCalculator`.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, OptionValue,
    Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeKind};

use super::util::{abc_size, complexity_target, format_g4, it_is_parameter, AllowedNames};

/// Checks that the ABC size of methods is not higher than the configured maximum.
#[derive(Debug, Clone)]
pub struct AbcSize {
    max: f64,
    discount_repeated_attributes: bool,
    allowed: AllowedNames,
    it_parameter: bool,
}

impl Rule for AbcSize {
    const META: RuleMeta = RuleMeta {
        name: "Metrics/AbcSize",
        department: Department::Metrics,
        summary: "Checks that the ABC size of methods is not higher than the configured maximum.",
        explanation: "The ABC size is based on assignments, branches (method calls), and \
            conditions. See https://wiki.c2.com/?AbcMetric and \
            https://en.wikipedia.org/wiki/ABC_Software_Metric.\n\n\
            Interpreting ABC size:\n\n\
            * `<= 17` satisfactory\n\
            * `18..30` unsatisfactory\n\
            * `> 30` dangerous\n\n\
            You can have repeated \"attributes\" calls count as a single \"branch\". For this \
            purpose, attributes are any method with no argument; no attempt is meant to \
            distinguish actual `attr_reader` from other methods.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode, NodeKind::CallNode],
        config: &[
            ConfigOption {
                name: "Max",
                // A calculated magnitude, so an Integer or a Float.
                default: ConfigDefault::Int(17),
                allowed: &[],
                doc: "Maximum ABC size allowed.",
            },
            ConfigOption {
                name: "CountRepeatedAttributes",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Count each repeated attribute call as its own branch.",
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
            max: options.get("Max").and_then(OptionValue::as_float).unwrap_or(17.0),
            discount_repeated_attributes: !options.bool("CountRepeatedAttributes"),
            allowed: AllowedNames::new(options),
            it_parameter: it_is_parameter(options.target_ruby_version()),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(target) = complexity_target(node) else { return };
        if self.allowed.allows(target.name) {
            return;
        }
        let (size, assignment, branch, condition) =
            abc_size(&target.body, self.discount_repeated_attributes, self.it_parameter);
        if size <= self.max {
            return;
        }
        let name = String::from_utf8_lossy(target.name);
        let size = format_g4(size);
        let max = format_g4(self.max);
        ctx.report(
            &Self::META,
            target.span,
            format!(
                "Assignment Branch Condition size for `{name}` is too high. \
                 [<{assignment}, {branch}, {condition}> {size}/{max}]"
            ),
        );
    }
}
