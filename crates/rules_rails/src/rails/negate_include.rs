//! `Rails/NegateInclude`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/negate_include.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Use `.exclude?` and remove the negation part.";

/// Prefer `collection.exclude?(obj)` over `!collection.include?(obj)`.
#[derive(Debug, Clone)]
pub struct NegateInclude;

impl Rule for NegateInclude {
    const META: RuleMeta = RuleMeta {
        name: "Rails/NegateInclude",
        department: Department::Rails,
        summary: "Prefer `collection.exclude?(obj)` over `!collection.include?(obj)`.",
        explanation: "Enforces the use of `collection.exclude?(obj)` over \
                      `!collection.include?(obj)`.\n\nThis cop is unsafe because false \
                      positive will occur for receiver objects that do not have an `exclude?` \
                      method. (e.g. `IPAddr`)\n\n```ruby\n# bad\n!array.include?(2)\n\
                      !hash.include?(:key)\n\n# good\narray.exclude?(2)\nhash.exclude?(:key)\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        // `(send (send $!nil? :include? $_) :!)`.
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"!" || call.is_safe_navigation() || call.arguments().is_some()
        {
            return;
        }
        let Some(inner) = call.receiver().and_then(|receiver| receiver.as_call_node()) else {
            return;
        };
        if inner.name().as_slice() != b"include?" || inner.is_safe_navigation() {
            return;
        }
        let Some(receiver) = inner.receiver() else { return };
        if inner.block().is_some() {
            return;
        }
        let Some(arguments) = inner.arguments() else { return };
        let mut arguments = arguments.arguments().iter();
        let (Some(argument), None) = (arguments.next(), arguments.next()) else { return };

        let mut replacement = ctx.text(receiver.span()).to_vec();
        replacement.extend_from_slice(b".exclude?(");
        replacement.extend_from_slice(ctx.text(argument.span()));
        replacement.push(b')');
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            MSG,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(node.span(), replacement)],
            },
        );
    }
}
