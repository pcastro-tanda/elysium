//! `Style/RedundantCurrentDirectoryInPath`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_current_directory_in_path.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

const MSG: &str = "Remove the redundant current directory path.";

/// Checks for a redundant current directory in a path given to `require_relative`.
#[derive(Debug, Clone)]
pub struct RedundantCurrentDirectoryInPath;

impl Rule for RedundantCurrentDirectoryInPath {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantCurrentDirectoryInPath",
        department: Department::Style,
        summary: "Checks for a redundant current directory in a path given to \
            `require_relative`.",
        explanation: "Checks for paths given to `require_relative` that start with \
            the current directory (`./`), which can be omitted.",
        enabled_by_default: false,
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
        if call.receiver().is_some() || call.name().as_slice() != b"require_relative" {
            return;
        }
        let Some(args) = call.arguments() else { return };
        let list = args.arguments();
        let Some(first_argument) = list.first() else { return };

        let Some(content_span) = leading_path_content(&first_argument) else { return };
        let content = ctx.text(content_span);
        let Some(redundant_length) = redundant_path_length(content) else { return };

        let range = Span::new(content_span.start, content_span.start + redundant_length);
        ctx.report_with_fix(
            &Self::META,
            range,
            MSG,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(range)] },
        );
    }
}

/// RuboCop's `leading_path_content`: the literal text at the start of the
/// path, which is the whole string for a plain string and the leading
/// literal segment for an interpolated one (`None` when it starts with
/// interpolation).
fn leading_path_content(node: &Node<'_>) -> Option<Span> {
    if let Some(string) = node.as_string_node() {
        return Some(string.content_loc().span());
    }
    let interpolated = node.as_interpolated_string_node()?;
    let first = interpolated.parts().first()?;
    Some(first.as_string_node()?.content_loc().span())
}

/// RuboCop's `redundant_path_length`: the length of a `\A\./+` match
/// (a literal dot followed by one or more slashes) at the start of `path`.
fn redundant_path_length(path: &[u8]) -> Option<u32> {
    if path.first() != Some(&b'.') {
        return None;
    }
    let slash_count = path[1..].iter().take_while(|&&b| b == b'/').count();
    if slash_count == 0 {
        return None;
    }
    Some(u32::try_from(1 + slash_count).expect("path length exceeds u32"))
}
