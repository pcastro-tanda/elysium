//! `Rails/ApplicationController`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/application_controller.rb` (with its
//! `EnforceSuperclass` mixin, `lib/rubocop/cop/mixin/enforce_superclass.rb`).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Controllers should subclass `ApplicationController`.";
const SUPERCLASS: &str = "ApplicationController";

/// Check that controllers subclass `ApplicationController`.
#[derive(Debug, Clone)]
pub struct ApplicationController {
    /// The `value` of an `ApplicationController = ...` constant assignment,
    /// which may be `Class.new(ActionController::Base)` (the definition
    /// itself).
    definition_value: Option<Span>,
}

impl Rule for ApplicationController {
    const META: RuleMeta = RuleMeta {
        name: "Rails/ApplicationController",
        department: Department::Rails,
        summary: "Check that controllers subclass ApplicationController.",
        explanation: "Checks that controllers subclass `ApplicationController`.\n\nThis cop's \
                      autocorrection is unsafe because it may let the logic from \
                      `ApplicationController` sneak into a controller that is not purposed to \
                      inherit logic common among other controllers.\n\n```ruby\n# good\nclass \
                      MyController < ApplicationController\n  # ...\nend\n\n# bad\nclass \
                      MyController < ActionController::Base\n  # ...\nend\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::ClassNode,
            NodeKind::CallNode,
            NodeKind::ConstantWriteNode,
            NodeKind::ConstantPathWriteNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { definition_value: None })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
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
                // `(class (const _ !:ApplicationController) BASE_PATTERN ...)`.
                let Some(class) = node.as_class_node() else { return };
                if class.name().as_slice() == SUPERCLASS.as_bytes() {
                    return;
                }
                let Some(superclass) = class.superclass() else { return };
                if is_action_controller_base(&superclass) {
                    register_offense(ctx, superclass.span());
                }
            }
            NodeKind::CallNode => self.check_class_new(node, ctx),
            _ => {}
        }
    }
}

impl ApplicationController {
    /// `(send (const {nil? cbase} :Class) :new BASE_PATTERN)`, unless it is
    /// what `ApplicationController` (unqualified or `::`) is assigned,
    /// directly or through a literal block.
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
        if !is_action_controller_base(&argument) || self.definition_value == Some(node.span()) {
            return;
        }
        register_offense(ctx, argument.span());
    }
}

/// `BASE_PATTERN`: `(const (const {nil? cbase} :ActionController) :Base)`.
fn is_action_controller_base(node: &Node<'_>) -> bool {
    let Some(path) = node.as_constant_path_node() else { return false };
    if path.name().is_none_or(|name| name.as_slice() != b"Base") {
        return false;
    }
    path.parent().is_some_and(|parent| {
        is_bare_or_toplevel_const(&parent)
            && const_name(&parent).as_deref() == Some("ActionController")
    })
}

fn register_offense(ctx: &mut Context<'_>, span: Span) {
    ctx.report_with_fix(
        &ApplicationController::META,
        span,
        MSG,
        Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::replace(span, SUPERCLASS.as_bytes().to_vec())],
        },
    );
}
