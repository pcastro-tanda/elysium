//! `Rails/TopLevelHashWithIndifferentAccess`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/top_level_hash_with_indifferent_access.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

use super::application_record::target_rails_version;

const MSG: &str = "Avoid top-level `HashWithIndifferentAccess`.";
const NAME: &[u8] = b"HashWithIndifferentAccess";
/// `minimum_target_rails_version 5.1`.
const MINIMUM_TARGET_RAILS_VERSION: f64 = 5.1;

/// Identifies top-level `HashWithIndifferentAccess`.
#[derive(Debug, Clone)]
pub struct TopLevelHashWithIndifferentAccess {
    supported: bool,
}

impl Rule for TopLevelHashWithIndifferentAccess {
    const META: RuleMeta = RuleMeta {
        name: "Rails/TopLevelHashWithIndifferentAccess",
        department: Department::Rails,
        summary: "Identifies top-level `HashWithIndifferentAccess`.",
        explanation: "Identifies top-level `HashWithIndifferentAccess`. This has been \
                      soft-deprecated since Rails 5.1.\n\n```ruby\n# bad\n\
                      HashWithIndifferentAccess.new(foo: 'bar')\n\n# good\n\
                      ActiveSupport::HashWithIndifferentAccess.new(foo: 'bar')\n```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::ConstantReadNode, NodeKind::ConstantPathNode],
        config: &[],
        blind_spots: "Without `AllCops/TargetRailsVersion` the Rails version is taken to be \
                      5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { supported: target_rails_version(options) >= MINIMUM_TARGET_RAILS_VERSION })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        // `(const {nil? cbase} :HashWithIndifferentAccess)`.
        let name_start = if node.kind() == NodeKind::ConstantReadNode {
            let Some(read) = node.as_constant_read_node() else { return };
            if read.name().as_slice() != NAME {
                return;
            }
            node.span().start
        } else {
            let Some(path) = node.as_constant_path_node() else { return };
            if path.name().is_none_or(|name| name.as_slice() != NAME) || path.parent().is_some() {
                return;
            }
            node.span().end - u32::try_from(NAME.len()).expect("name length fits u32")
        };
        // `node.parent&.class_type? && node.parent.ancestors.any?(&:module_type?)`.
        // A lone statement of a class body is the class's direct child in
        // whitequark, with no `begin` in between.
        let mut ancestors = ctx.ancestors();
        if let [rest @ .., class, body] = ancestors {
            if class.kind == NodeKind::ClassNode
                && body.kind == NodeKind::StatementsNode
                && body.span == node.span()
            {
                ancestors = &ancestors[..=rest.len()];
            }
        }
        if let Some((parent, outer)) = ancestors.split_last() {
            if parent.kind == NodeKind::ClassNode
                && outer.iter().any(|a| a.kind == NodeKind::ModuleNode)
            {
                return;
            }
        }
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::insert(name_start, b"ActiveSupport::".to_vec())],
            },
        );
    }
}
