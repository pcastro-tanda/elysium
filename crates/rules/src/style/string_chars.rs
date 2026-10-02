//! `Style/StringChars`, ported from RuboCop's
//! `lib/rubocop/cop/style/string_chars.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const BAD_ARGUMENTS: &[&[u8]] = &[b"//", b"''", b"\"\""];

/// Checks for uses of `String#split` with empty string or regexp literal argument.
#[derive(Debug, Clone)]
pub struct StringChars;

impl Rule for StringChars {
    const META: RuleMeta = RuleMeta {
        name: "Style/StringChars",
        department: Department::Style,
        summary: "Checks for uses of `String#split` with empty string or regexp literal argument.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
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
        if call.name().as_slice() != b"split" {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let args = arguments.arguments();
        if args.len() != 1 {
            return;
        }
        let arg = args.iter().next().expect("checked len == 1");
        let arg_source = ctx.text(arg.span());
        if !BAD_ARGUMENTS.contains(&arg_source) {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        let range = Span::new(selector.span().start, node.span().end);
        let current = String::from_utf8_lossy(ctx.text(range)).into_owned();
        let message = format!("Use `chars` instead of `{current}`.");
        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(range, b"chars".to_vec())],
            },
        );
    }
}
