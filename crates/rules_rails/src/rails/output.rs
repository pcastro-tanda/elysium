//! `Rails/Output`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/output.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Do not write to stdout. Use Rails's logger if you want to log.";

/// Checks for calls to puts, print, etc.
#[derive(Debug, Clone)]
pub struct Output;

impl Rule for Output {
    const META: RuleMeta = RuleMeta {
        name: "Rails/Output",
        department: Department::Rails,
        summary: "Checks for calls to puts, print, etc.",
        explanation: "Checks for the use of output calls like puts and print\n\nThis cop's \
                      autocorrection is unsafe because depending on the Rails log level \
                      configuration, changing from `puts` to `Rails.logger.debug` could result \
                      in no output being shown.\n\n```ruby\n# bad\nputs 'A debug message'\npp \
                      'A debug message'\nprint 'A debug message'\n\n# good\nRails.logger.debug \
                      'A debug message'\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
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
        if call.is_safe_navigation() {
            return;
        }
        let name = call.name();
        let name = name.as_slice();
        let is_output = match call.receiver() {
            None => matches!(name, b"ap" | b"p" | b"pp" | b"pretty_print" | b"print" | b"puts"),
            Some(receiver) => {
                matches!(name, b"binwrite" | b"syswrite" | b"write" | b"write_nonblock")
                    && is_stdio(&receiver)
            }
        };
        if !is_output {
            return;
        }
        // `node.parent&.call_type? || node.block_node`
        if parent_is_call(ctx) || call.block().is_some_and(|b| b.as_block_node().is_some()) {
            return;
        }
        if call.block().is_some() {
            return; // a `&block` argument
        }
        if call.arguments().is_some_and(|args| {
            args.arguments()
                .iter()
                .any(|arg| matches!(arg.kind(), NodeKind::HashNode | NodeKind::KeywordHashNode))
        }) {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        let range = if call.receiver().is_some() {
            Span::new(node.span().start, selector.span().end)
        } else {
            selector.span()
        };
        ctx.report_with_fix(
            &Self::META,
            range,
            MSG,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(range, b"Rails.logger.debug".to_vec())],
            },
        );
    }
}

/// `(gvar {$stdout $stderr})` or `(const {nil? cbase} {:STDOUT :STDERR})`.
fn is_stdio(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::GlobalVariableReadNode => node
            .as_global_variable_read_node()
            .is_some_and(|g| matches!(g.name().as_slice(), b"$stdout" | b"$stderr")),
        NodeKind::ConstantReadNode => node
            .as_constant_read_node()
            .is_some_and(|c| matches!(c.name().as_slice(), b"STDOUT" | b"STDERR")),
        NodeKind::ConstantPathNode => node.as_constant_path_node().is_some_and(|path| {
            path.parent().is_none()
                && path.name().is_some_and(|n| matches!(n.as_slice(), b"STDOUT" | b"STDERR"))
        }),
        _ => false,
    }
}

/// Whether the parser's parent of the current node is a `send`/`csend`: a
/// receiver, or an argument (Prism wraps those in an `ArgumentsNode`).
fn parent_is_call(ctx: &Context<'_>) -> bool {
    let ancestors = ctx.ancestors();
    match ancestors.last().map(|p| p.kind) {
        Some(NodeKind::CallNode) => true,
        Some(NodeKind::ArgumentsNode) => {
            ancestors.len().checked_sub(2).is_some_and(|i| ancestors[i].kind == NodeKind::CallNode)
        }
        _ => false,
    }
}
