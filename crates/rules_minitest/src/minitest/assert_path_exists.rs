//! `Minitest/AssertPathExists`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/assert_path_exists.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

const ASSERTION_TYPE: &str = "assert";

/// Enforces the test to use `assert_path_exists` instead of using
/// `assert(File.exist?(path))`.
#[derive(Debug, Clone)]
pub struct AssertPathExists;

impl Rule for AssertPathExists {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/AssertPathExists",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `assert_path_exists` instead of using `assert(File.exist?(path))`.",
        explanation: "Enforces the test to use `assert_path_exists` instead of using \
                      `assert(File.exist?(path))`.\n\n```ruby\n# bad\n\
                      assert(File.exist?(path))\nassert(File.exist?(path), 'message')\n\n\
                      # good\nassert_path_exists(path)\n\
                      assert_path_exists(path, 'message')\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
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
        let arguments = argument_list(&call);
        let Some(path) = file_exists(&call, &arguments) else { return };

        let path_source = String::from_utf8_lossy(ctx.text(path.span())).into_owned();
        let message_argument = arguments.get(1);
        let args = match message_argument {
            Some(message) => {
                format!("{path_source}, {}", String::from_utf8_lossy(ctx.text(message.span())))
            }
            None => path_source.clone(),
        };
        let good_method = if call.opening_loc().is_some() {
            format!("{ASSERTION_TYPE}_path_exists({args})")
        } else {
            format!("{ASSERTION_TYPE}_path_exists {args}")
        };
        let message = format!("Prefer using `{good_method}`.");

        let Some(selector) = call.message_loc() else { return };
        let edits = vec![
            Edit::replace(selector.span(), format!("{ASSERTION_TYPE}_path_exists").into_bytes()),
            Edit::replace(arguments[0].span(), path_source.into_bytes()),
        ];
        ctx.report_with_fix(
            &Self::META,
            call_span_excluding_block(&call),
            message,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}

/// The `assert_file_exists` node pattern:
///
/// ```text
/// (send nil? :assert
///   (send
///     (const _ :File) {:exist? :exists?} $_)
///     $...)
/// ```
///
/// Returns the captured path.
fn file_exists<'pr>(call: &CallNode<'pr>, arguments: &[Node<'pr>]) -> Option<Node<'pr>> {
    if call.name().as_slice() != ASSERTION_TYPE.as_bytes() {
        return None;
    }
    let inner = arguments.first()?.as_call_node()?;
    if !matches!(inner.name().as_slice(), b"exist?" | b"exists?")
        || inner.is_safe_navigation()
        || inner.block().is_some_and(|block| block.as_block_argument_node().is_none())
    {
        return None;
    }
    let receiver = inner.receiver()?;
    let is_file = match (receiver.as_constant_read_node(), receiver.as_constant_path_node()) {
        (Some(constant), _) => constant.name().as_slice() == b"File",
        (_, Some(path)) => path.name().is_some_and(|name| name.as_slice() == b"File"),
        _ => false,
    };
    if !is_file {
        return None;
    }
    let inner_arguments = argument_list(&inner);
    let [path] = inner_arguments.as_slice() else { return None };
    Some(*path)
}

/// The call's arguments the way whitequark lists a `send`'s: a `&block`
/// argument is one of them.
fn argument_list<'pr>(call: &CallNode<'pr>) -> Vec<Node<'pr>> {
    let mut args: Vec<Node<'pr>> =
        call.arguments().map(|args| args.arguments().iter().collect()).unwrap_or_default();
    if let Some(block) = call.block().filter(|block| block.as_block_argument_node().is_some()) {
        args.push(block);
    }
    args
}
