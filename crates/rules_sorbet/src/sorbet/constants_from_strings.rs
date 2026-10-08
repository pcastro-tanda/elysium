//! `Sorbet/ConstantsFromStrings`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/constants_from_strings.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

/// Disallows the calls that are used to get constants from Strings.
#[derive(Debug, Clone)]
pub struct ConstantsFromStrings;

impl Rule for ConstantsFromStrings {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/ConstantsFromStrings",
        department: Department::Sorbet,
        summary: "Forbids constant access through meta-programming.",
        explanation: "Disallows the calls that are used to get constants fom Strings\nsuch as `constantize`, `const_get`, and `constants`.\n\nThe goal of this cop is to make the code easier to statically analyze,\nmore IDE-friendly, and more predictable. It leads to code that clearly\nexpresses which values the constant can have.\n\n```ruby\n# bad\nclass_name.constantize\n\n# bad\nconstants.detect { |c| c.name == \"User\" }\n\n# bad\nconst_get(class_name)\n\n# good\ncase class_name\nwhen \"User\"\n  User\nelse\n  raise ArgumentError\nend\n\n# good\n{ \"User\" => User }.fetch(class_name)\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
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
        let name = call.name();
        let name = name.as_slice();
        if !matches!(name, b"constantize" | b"constants" | b"const_get" | b"safe_constantize") {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        let method_name = String::from_utf8_lossy(name);
        ctx.report(
            &Self::META,
            selector.span(),
            format!(
                "Don't use `{method_name}`, it makes the code harder to understand, less \
                 editor-friendly, and impossible to analyze. Replace `{method_name}` with a \
                 case/when or a hash."
            ),
        );
    }
}
