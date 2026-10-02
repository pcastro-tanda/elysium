//! `Rails/ApplicationRecord`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/application_record.rb` (with its
//! `EnforceSuperclass` mixin, `lib/rubocop/cop/mixin/enforce_superclass.rb`).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Models should subclass `ApplicationRecord`.";
const SUPERCLASS: &str = "ApplicationRecord";

/// `minimum_target_rails_version 5.0`.
const MINIMUM_TARGET_RAILS_VERSION: f64 = 5.0;

/// Check that models subclass `ApplicationRecord`.
#[derive(Debug, Clone)]
pub struct ApplicationRecord {
    /// Whether the target Rails version reaches
    /// `minimum_target_rails_version`; RuboCop does not run the cop at all
    /// otherwise.
    supported: bool,
    /// The `value` of an `ApplicationRecord = ...` constant assignment, which
    /// may be `Class.new(ActiveRecord::Base)` (the definition itself).
    definition_value: Option<Span>,
}

impl Rule for ApplicationRecord {
    const META: RuleMeta = RuleMeta {
        name: "Rails/ApplicationRecord",
        department: Department::Rails,
        summary: "Check that models subclass ApplicationRecord.",
        explanation: "Checks that models subclass `ApplicationRecord` with Rails 5.0.\n\nIt is \
                      a common practice to define models inside migrations in order to retain \
                      forward compatibility by avoiding loading any application code. And so \
                      migration files are excluded by default for this cop.\n\nThis cop's \
                      autocorrection is unsafe because it may let the logic from \
                      `ApplicationRecord` sneak into an Active Record model that is not \
                      purposed to inherit logic common among other Active Record models.\n\n\
                      ```ruby\n# good\nclass Rails5Model < ApplicationRecord\n  # ...\nend\n\n\
                      # bad\nclass Rails4Model < ActiveRecord::Base\n  # ...\nend\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[
            NodeKind::ClassNode,
            NodeKind::CallNode,
            NodeKind::ConstantWriteNode,
            NodeKind::ConstantPathWriteNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            supported: options.target_rails_version() >= MINIMUM_TARGET_RAILS_VERSION,
            definition_value: None,
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        match node.kind() {
            NodeKind::ConstantWriteNode => {
                let Some(write) = node.as_constant_write_node() else { return };
                if write.name().as_slice() == SUPERCLASS.as_bytes() {
                    self.definition_value = Some(write.value().span());
                }
            }
            NodeKind::ConstantPathWriteNode => {
                let Some(write) = node.as_constant_path_write_node() else { return };
                let target = write.target();
                if target.parent().is_none()
                    && target.name().is_some_and(|name| name.as_slice() == SUPERCLASS.as_bytes())
                {
                    self.definition_value = Some(write.value().span());
                }
            }
            NodeKind::ClassNode => {
                // `(class (const _ !:ApplicationRecord) BASE_PATTERN ...)`.
                let Some(class) = node.as_class_node() else { return };
                if class.name().as_slice() == SUPERCLASS.as_bytes() {
                    return;
                }
                let Some(superclass) = class.superclass() else { return };
                if is_active_record_base(&superclass) {
                    register_offense(ctx, superclass.span());
                }
            }
            NodeKind::CallNode => self.check_class_new(node, ctx),
            _ => {}
        }
    }
}

impl ApplicationRecord {
    /// `(send (const {nil? cbase} :Class) :new BASE_PATTERN)`, unless it is
    /// what `ApplicationRecord` (unqualified or `::`) is assigned, directly
    /// or through a literal block.
    fn check_class_new(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"new" || call.is_safe_navigation() {
            return;
        }
        // A `&block` argument is a second `send` argument in whitequark.
        if call.block().is_some_and(|block| block.as_block_argument_node().is_some()) {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        if !is_bare_or_toplevel_const(&receiver)
            || const_name(&receiver).as_deref() != Some("Class")
        {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let mut arguments = arguments.arguments().iter();
        let (Some(argument), None) = (arguments.next(), arguments.next()) else { return };
        if !is_active_record_base(&argument) || self.definition_value == Some(node.span()) {
            return;
        }
        register_offense(ctx, argument.span());
    }
}

/// `BASE_PATTERN`: `(const (const {nil? cbase} :ActiveRecord) :Base)`.
fn is_active_record_base(node: &Node<'_>) -> bool {
    let Some(path) = node.as_constant_path_node() else { return false };
    if path.name().is_none_or(|name| name.as_slice() != b"Base") {
        return false;
    }
    path.parent().is_some_and(|parent| {
        is_bare_or_toplevel_const(&parent) && const_name(&parent).as_deref() == Some("ActiveRecord")
    })
}

fn register_offense(ctx: &mut Context<'_>, span: Span) {
    ctx.report_with_fix(
        &ApplicationRecord::META,
        span,
        MSG,
        Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::replace(span, SUPERCLASS.as_bytes().to_vec())],
        },
    );
}
