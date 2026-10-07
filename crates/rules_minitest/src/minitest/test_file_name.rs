//! `Minitest/TestFileName`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/test_file_name.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Test file path should start with `test_` or end with `_test.rb`.";

/// Checks if test file names start with `test_` or end with `_test.rb`.
#[derive(Debug, Clone)]
pub struct TestFileName {
    test_file: bool,
}

impl Rule for TestFileName {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/TestFileName",
        department: Department::Minitest,
        summary: "Checks if test file names start with `test_` or end with `_test.rb`.",
        explanation: "Checks if test file names start with `test_` or end with `_test.rb`. Files which define classes having names ending with `Test` are checked. Not following this convention may result in tests not being run.\n\n```ruby\n# bad\nmy_class.rb\n\n# good\nmy_class_test.rb\ntest_my_class.rb\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::ClassNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { test_file: false })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if self.test_file {
            return;
        }
        let Some(class) = node.as_class_node() else { return };
        // `test_class?`: `class_node.parent_class &&
        // class_node.identifier.source.end_with?('Test')`.
        if class.superclass().is_some() && ctx.text(class.constant_path().span()).ends_with(b"Test")
        {
            self.test_file = true;
        }
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        if !self.test_file {
            return;
        }
        let path = ctx.source().path();
        let basename = path.file_name().map(|name| name.to_string_lossy()).unwrap_or_default();
        if !(basename.starts_with("test_") || basename.ends_with("_test.rb")) {
            ctx.report_global(&Self::META, MSG);
        }
    }
}
