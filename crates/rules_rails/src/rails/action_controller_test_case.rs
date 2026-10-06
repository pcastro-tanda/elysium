//! `Rails/ActionControllerTestCase`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/action_controller_test_case.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Use `ActionDispatch::IntegrationTest` instead.";
/// `minimum_target_rails_version 5.0`.
const MINIMUM_TARGET_RAILS_VERSION: f64 = 5.0;

/// Use `ActionDispatch::IntegrationTest` instead of `ActionController::TestCase`.
#[derive(Debug, Clone)]
pub struct ActionControllerTestCase {
    supported: bool,
}

impl Rule for ActionControllerTestCase {
    const META: RuleMeta = RuleMeta {
        name: "Rails/ActionControllerTestCase",
        department: Department::Rails,
        summary: "Use `ActionDispatch::IntegrationTest` instead of `ActionController::TestCase`.",
        explanation: "Using `ActionController::TestCase` is discouraged and should be replaced \
                      by `ActionDispatch::IntegrationTest`. Controller tests are too close to \
                      the internals of a controller whereas integration tests mimic the \
                      browser/user.\n\nThis cop's autocorrection is unsafe because the API of \
                      each test case class is different. Make sure to update each test of your \
                      controller test cases after changing the superclass.\n\n```ruby\n# bad\n\
                      class MyControllerTest < ActionController::TestCase\nend\n\n# good\n\
                      class MyControllerTest < ActionDispatch::IntegrationTest\nend\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ClassNode],
        config: &[],
        blind_spots: "Without `AllCops/TargetRailsVersion` the Rails version is taken to be \
                      5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { supported: options.target_rails_version() >= MINIMUM_TARGET_RAILS_VERSION })
    }

    /// `(class (const _ _) (const (const {nil? cbase} :ActionController) :TestCase) _)`.
    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        let Some(class) = node.as_class_node() else { return };
        // `(const _ _)`: a name that is not `Foo::Bar` with a non-constant scope
        // still matches any `const`; whitequark's `_` scope also admits `cbase`.
        let name = class.constant_path();
        if name.as_constant_read_node().is_none() && name.as_constant_path_node().is_none() {
            return;
        }
        let Some(superclass) = class.superclass() else { return };
        let Some(path) = superclass.as_constant_path_node() else { return };
        if path.name().is_none_or(|n| n.as_slice() != b"TestCase") {
            return;
        }
        let Some(parent) = path.parent() else { return };
        if !is_bare_or_toplevel_const(&parent)
            || const_name(&parent).as_deref() != Some("ActionController")
        {
            return;
        }
        let span = superclass.span();
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, b"ActionDispatch::IntegrationTest".to_vec())],
            },
        );
    }
}
