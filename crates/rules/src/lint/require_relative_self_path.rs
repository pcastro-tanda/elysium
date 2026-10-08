//! `Lint/RequireRelativeSelfPath`, ported from RuboCop's
//! `lib/rubocop/cop/lint/require_relative_self_path.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// Upstream's `MSG`.
const MSG: &str = "Remove the `require_relative` that requires itself.";

/// Checks for a file requiring itself with `require_relative`.
#[derive(Debug, Clone)]
pub struct RequireRelativeSelfPath;

impl Rule for RequireRelativeSelfPath {
    const META: RuleMeta = RuleMeta {
        name: "Lint/RequireRelativeSelfPath",
        department: Department::Lint,
        summary: "Checks for a file requiring itself with `require_relative`.",
        explanation: "\
```ruby
# bad

# foo.rb
require_relative 'foo'
require_relative 'bar'

# good

# foo.rb
require_relative 'bar'
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
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
        if call.receiver().is_some() || call.name().as_slice() != b"require_relative" {
            return;
        }
        let Some(first) = call.arguments().and_then(|a| a.arguments().first()) else { return };
        let Some(s) = first.as_string_node() else { return };
        let required = String::from_utf8_lossy(s.unescaped()).into_owned();
        if !same_file(ctx.source().path(), &required) {
            return;
        }
        let span = ctx.whole_lines(node.span());
        let fix = Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(span)] };
        ctx.report_with_fix(&Self::META, node.span(), MSG, fix);
    }
}

/// Upstream's `same_file?`.
fn same_file(file_path: &std::path::Path, required_feature: &str) -> bool {
    if file_path.extension().and_then(|e| e.to_str()) != Some("rb") {
        return false;
    }
    let Some(basename) = file_path.file_stem().and_then(|s| s.to_str()) else { return false };
    required_feature == basename || required_feature == format!("{basename}.rb")
}
