//! `Rails/RefuteMethods`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/refute_methods.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

/// `CORRECTIONS`: `(refute method, assert_not method)`.
const CORRECTIONS: [(&str, &str); 14] = [
    ("refute", "assert_not"),
    ("refute_empty", "assert_not_empty"),
    ("refute_equal", "assert_not_equal"),
    ("refute_in_delta", "assert_not_in_delta"),
    ("refute_in_epsilon", "assert_not_in_epsilon"),
    ("refute_includes", "assert_not_includes"),
    ("refute_instance_of", "assert_not_instance_of"),
    ("refute_kind_of", "assert_not_kind_of"),
    ("refute_nil", "assert_not_nil"),
    ("refute_operator", "assert_not_operator"),
    ("refute_predicate", "assert_not_predicate"),
    ("refute_respond_to", "assert_not_respond_to"),
    ("refute_same", "assert_not_same"),
    ("refute_match", "assert_no_match"),
];

/// Use `assert_not` methods instead of `refute` methods.
#[derive(Debug, Clone, Copy)]
pub struct RefuteMethods {
    /// `style == :assert_not`.
    assert_not: bool,
}

impl Rule for RefuteMethods {
    const META: RuleMeta = RuleMeta {
        name: "Rails/RefuteMethods",
        department: Department::Rails,
        summary: "Use `assert_not` methods instead of `refute` methods.",
        explanation: "Use `assert_not` methods instead of `refute` methods.\n\n```ruby\n\
                      # bad\nrefute false\nrefute_empty [1, 2, 3]\n\n# good\nassert_not false\n\
                      assert_not_empty [1, 2, 3]\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("assert_not"),
            allowed: &["assert_not", "refute"],
            doc: "Which family of methods to prefer.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { assert_not: options.style("EnforcedStyle")? == "assert_not" })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        // `(send nil? #bad_method? ...)`.
        if call.receiver().is_some() {
            return;
        }
        let name = call.name();
        let Some(&(refute, assert_not)) = CORRECTIONS.iter().find(|(refute, assert_not)| {
            let bad = if self.assert_not { refute } else { assert_not };
            bad.as_bytes() == name.as_slice()
        }) else {
            return;
        };
        let (bad, good) = if self.assert_not { (refute, assert_not) } else { (assert_not, refute) };
        let Some(selector) = call.message_loc().map(|loc| loc.span()) else { return };
        ctx.report_with_fix(
            &Self::META,
            selector,
            format!("Prefer `{good}` over `{bad}`."),
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(selector, good.as_bytes().to_vec())],
            },
        );
    }
}
