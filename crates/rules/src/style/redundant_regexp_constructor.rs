//! `Style/RedundantRegexpConstructor`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_regexp_constructor.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{ext, LocationExt as _, Node, NodeExt as _, NodeKind};

/// Checks for the instantiation of a regexp using a redundant `Regexp.new` or
/// `Regexp.compile`. Autocorrect replaces it with a regexp literal which is
/// the simplest and fastest.
///
/// # Examples
///
/// ```ruby
/// # bad
/// Regexp.new(/regexp/)
/// Regexp.compile(/regexp/)
///
/// # good
/// /regexp/
/// Regexp.new('regexp')
/// Regexp.compile('regexp')
/// ```
#[derive(Debug, Clone)]
pub struct RedundantRegexpConstructor;

impl Rule for RedundantRegexpConstructor {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantRegexpConstructor",
        department: Department::Style,
        summary: "Checks for the instantiation of regexp using redundant `Regexp.new` or `Regexp.compile`.",
        explanation: "\
Checks for the instantiation of a regexp using a redundant `Regexp.new` or \
`Regexp.compile`. Autocorrect replaces it with a regexp literal which is the \
simplest and fastest.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let name = String::from_utf8_lossy(call.name().as_slice()).into_owned();
        if name != "new" && name != "compile" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        if ext::const_name(&receiver).as_deref() != Some("Regexp") {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let args: Vec<Node<'_>> = arguments.arguments().iter().collect();
        if args.len() != 1 {
            return;
        }
        let arg = &args[0];
        if !matches!(
            arg.kind(),
            NodeKind::RegularExpressionNode | NodeKind::InterpolatedRegularExpressionNode
        ) {
            return;
        }

        let span = node.location().span();
        let message = format!("Remove the redundant `Regexp.{name}`.");
        let regexp_source = ctx.text(arg.location().span()).to_vec();
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(span, regexp_source)],
        };
        ctx.report_with_fix(&Self::META, span, message, fix);
    }
}
