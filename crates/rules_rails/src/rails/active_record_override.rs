//! `Rails/ActiveRecordOverride`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/active_record_override.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{each_descendant, Node, NodeExt as _, NodeKind};

const BAD_METHODS: &[&[u8]] = &[b"create", b"destroy", b"save", b"update"];
const ACTIVE_RECORD_CLASSES: &[&[u8]] =
    &[b"ApplicationRecord", b"ActiveModel::Base", b"ActiveRecord::Base"];

/// Checks for overriding built-in Active Record methods instead of using
/// callbacks.
#[derive(Debug, Clone)]
pub struct ActiveRecordOverride {
    /// The superclass source of each enclosing `class` (`None` for a class
    /// without one), innermost last: `find_parent_class_name` stops at the
    /// nearest class.
    superclasses: Vec<Option<Vec<u8>>>,
}

impl Rule for ActiveRecordOverride {
    const META: RuleMeta = RuleMeta {
        name: "Rails/ActiveRecordOverride",
        department: Department::Rails,
        summary: "Check for overriding Active Record methods instead of using callbacks.",
        explanation: "Checks for overriding built-in Active Record methods instead of using \
                      callbacks.\n\n```ruby\n# bad\nclass Book < ApplicationRecord\n  def save\n    \
                      self.title = title.upcase!\n    super\n  end\nend\n\n# good\nclass Book < \
                      ApplicationRecord\n  before_save :upcase_title\n\n  def upcase_title\n    \
                      self.title = title.upcase!\n  end\nend\n```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::ClassNode, NodeKind::DefNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { superclasses: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(class) = node.as_class_node() {
            let superclass = class.superclass().map(|s| ctx.text(s.span()).to_vec());
            self.superclasses.push(superclass);
            return;
        }
        let Some(def) = node.as_def_node() else { return };
        // `on_def`, not `on_defs`.
        if def.receiver().is_some() {
            return;
        }
        let name = def.name();
        if !BAD_METHODS.contains(&name.as_slice()) {
            return;
        }
        let Some(Some(parent)) = self.superclasses.last() else { return };
        if !ACTIVE_RECORD_CLASSES.contains(&parent.as_slice()) {
            return;
        }
        let mut zsuper = false;
        each_descendant(node, &mut |child| {
            zsuper |= child.as_forwarding_super_node().is_some();
        });
        if !zsuper {
            return;
        }
        let method = String::from_utf8_lossy(name.as_slice());
        ctx.report(
            &Self::META,
            node.span(),
            format!(
                "Use `before_{method}`, `around_{method}`, or `after_{method}` callbacks \
                 instead of overriding the Active Record method `{method}`."
            ),
        );
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if node.as_class_node().is_some() {
            self.superclasses.pop();
        }
    }
}
