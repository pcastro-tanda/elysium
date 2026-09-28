//! `Style/Dir`, ported from RuboCop's
//! `lib/rubocop/cop/style/dir.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::const_name;
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Use `__dir__` to get an absolute path to the current file's directory.";

/// Use the `__dir__` method to retrieve the canonicalized absolute path to the current file.
#[derive(Debug, Clone)]
pub struct Dir;

impl Rule for Dir {
    const META: RuleMeta = RuleMeta {
        name: "Style/Dir",
        department: Department::Style,
        summary: "Use the `__dir__` method to retrieve the canonicalized absolute path to the current file.",
        explanation: "Checks for places where the `#__dir__` method can replace more \
            complex constructs to retrieve a canonicalized absolute path to the \
            current file.\n\n\
            ```ruby\n\
            # bad\n\
            path = File.expand_path(File.dirname(__FILE__))\n\n\
            # bad\n\
            path = File.dirname(File.realpath(__FILE__))\n\n\
            # good\n\
            path = __dir__\n\
            ```",
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
        let name = call.name().as_slice();
        if name != b"expand_path" && name != b"dirname" {
            return;
        }
        if !is_file_const(call.receiver().as_ref()) {
            return;
        }
        let Some(sole_arg) = single_argument(&call) else { return };
        let Some(inner) = sole_arg.as_call_node() else { return };
        if !is_file_const(inner.receiver().as_ref()) {
            return;
        }

        let matches = (name == b"expand_path" && inner.name().as_slice() == b"dirname")
            || (name == b"dirname" && inner.name().as_slice() == b"realpath");
        if !matches {
            return;
        }
        let Some(inner_arg) = single_argument(&inner) else { return };
        if inner_arg.as_source_file_node().is_none() {
            return;
        }

        let span = node.span();
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, b"__dir__".to_vec())],
            },
        );
    }
}

/// Whether the (optional) receiver is a bare `File` or `::File` constant.
fn is_file_const(receiver: Option<&Node<'_>>) -> bool {
    let Some(receiver) = receiver else { return false };
    const_name(receiver).is_some_and(|n| n == "File")
}

/// The single positional argument of a call, if there is exactly one.
fn single_argument<'a>(call: &ruby_ast::node::CallNode<'a>) -> Option<Node<'a>> {
    let args = call.arguments()?;
    let mut list = args.arguments().iter();
    let first = list.next()?;
    if list.next().is_some() {
        return None;
    }
    Some(first)
}
