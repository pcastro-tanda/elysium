//! `Rails/AfterCommitOverride`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/after_commit_override.rb` (with its
//! `ClassSendNodeHelper` mixin).

use std::collections::HashSet;

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const AFTER_COMMIT_CALLBACKS: &[&[u8]] = &[
    b"after_commit",
    b"after_create_commit",
    b"after_update_commit",
    b"after_save_commit",
    b"after_destroy_commit",
];

/// Check that there is only one `after_commit` style call per callback name
/// in a model.
#[derive(Debug, Clone)]
pub struct AfterCommitOverride;

impl Rule for AfterCommitOverride {
    const META: RuleMeta = RuleMeta {
        name: "Rails/AfterCommitOverride",
        department: Department::Rails,
        summary: "Enforces that there is only one call to `after_commit` (and its aliases - \
                  `after_create_commit`, `after_update_commit`, and `after_destroy_commit`) \
                  with the same callback name per model.",
        explanation: "Checks that only one `after_*_commit` hook is defined per callback name \
                      in a model, because a second hook with the same name overrides the \
                      first.\n\n```ruby\n# bad\nclass User < ApplicationRecord\n  \
                      after_create_commit :log_action\n  after_update_commit :log_action\nend\n\n\
                      # good\nclass User < ApplicationRecord\n  after_save_commit :log_action\n\
                      end\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::ClassNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(class) = node.as_class_node() else { return };
        let Some(body) = class.body() else { return };
        // `class_send_nodes`: the body when it is a lone `send`, else the
        // `send` children of its `begin`.
        let Some(statements) = body.as_statements_node() else { return };
        let mut seen: HashSet<Vec<u8>> = HashSet::new();
        for statement in &statements.body() {
            let Some(call) = statement.as_call_node() else { continue };
            // A call with a literal block is a `block` node, not a `send`.
            if call.is_safe_navigation()
                || call.block().is_some_and(|block| block.as_block_node().is_some())
                || !AFTER_COMMIT_CALLBACKS.contains(&call.name().as_slice())
            {
                continue;
            }
            let Some(first) = call.arguments().and_then(|a| a.arguments().iter().next()) else {
                continue;
            };
            let Some(symbol) = first.as_symbol_node() else { continue };
            let name = symbol.unescaped().to_vec();
            if seen.contains(&name) {
                ctx.report(
                    &Self::META,
                    statement.span(),
                    format!(
                        "There can only be one `after_*_commit :{}` hook defined for a model.",
                        String::from_utf8_lossy(&name)
                    ),
                );
            } else {
                seen.insert(name);
            }
        }
    }
}
