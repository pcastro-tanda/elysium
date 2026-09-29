//! `Style/RedundantFileExtensionInRequire`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_file_extension_in_require.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

const MSG: &str = "Redundant `.rb` file extension detected.";

/// Checks for the presence of superfluous `.rb` extension in the filename provided to `require` and `require_relative`.
#[derive(Debug, Clone)]
pub struct RedundantFileExtensionInRequire;

impl Rule for RedundantFileExtensionInRequire {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantFileExtensionInRequire",
        department: Department::Style,
        summary: "Checks for the presence of superfluous `.rb` extension in the filename provided to `require` and `require_relative`.",
        explanation: "",
        enabled_by_default: true,
        severity: Severity::Convention,
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
        if call.receiver().is_some() {
            return;
        }
        let name = call.name().as_slice();
        if name != b"require" && name != b"require_relative" {
            return;
        }
        let Some(args) = call.arguments() else { return };
        let arguments = args.arguments();
        if arguments.len() != 1 {
            return;
        }
        let Some(name_node) = arguments.iter().next().and_then(|n| n.as_string_node()) else {
            return;
        };
        if !name_node.unescaped().ends_with(b".rb") {
            return;
        }

        let node_span = name_node.location().span();
        let end_of_path_string = node_span.end;
        // A backslash right before the extension means the path is escaped;
        // dropping the extension would either produce invalid Ruby (a trailing
        // backslash escaping the closing quote) or a confusing path, so skip it.
        let extension_start = end_of_path_string - 4;
        let extension_end = end_of_path_string - 1;
        let extension_range = Span::new(extension_start, extension_end);

        let offset = extension_start - node_span.start;
        let node_source = ctx.text(node_span);
        let before = &node_source[..offset as usize];
        if before.ends_with(b"\\") {
            return;
        }

        ctx.report_with_fix(
            &Self::META,
            extension_range,
            MSG,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(extension_range)] },
        );
    }
}
