//! `Style/EnvHome`, ported from RuboCop's `lib/rubocop/cop/style/env_home.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{ext::const_name, Node, NodeExt as _, NodeKind};

const MSG: &str = "Use `Dir.home` instead.";

/// Checks for consistent usage of `ENV['HOME']`.
#[derive(Debug, Clone)]
pub struct EnvHome;

impl Rule for EnvHome {
    const META: RuleMeta = RuleMeta {
        name: "Style/EnvHome",
        department: Department::Style,
        summary: "Checks for consistent usage of `ENV['HOME']`.",
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
        if !matches!(call.name().as_slice(), b"[]" | b"fetch") {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        if const_name(&receiver).as_deref() != Some("ENV") {
            return;
        }
        let Some(args) = call.arguments() else { return };
        let items: Vec<Node<'_>> = args.arguments().iter().collect();
        let Some(first) = items.first() else { return };
        if first.as_string_node().is_none_or(|s| s.unescaped() != b"HOME") {
            return;
        }
        if items.len() == 2 && items[1].as_nil_node().is_none() {
            return;
        }
        // `ENV.fetch('HOME') { default }` supplies a fallback, just like
        // `ENV.fetch('HOME', default)`. `Dir.home` ignores the block, so
        // converting would silently drop it.
        if call.block().is_some() {
            return;
        }

        ctx.report_with_fix(
            &Self::META,
            node.span(),
            MSG,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(node.span(), b"Dir.home".to_vec())],
            },
        );
    }
}
