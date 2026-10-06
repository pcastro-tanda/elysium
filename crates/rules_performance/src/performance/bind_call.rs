//! `Performance/BindCall`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/bind_call.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Use `bind_call(obj, args, ...)` instead of `bind(obj).call(args, ...)`.
#[derive(Debug, Clone)]
pub struct BindCall {
    /// `minimum_target_ruby_version 2.7`.
    supported: bool,
}

impl Rule for BindCall {
    const META: RuleMeta = RuleMeta {
        name: "Performance/BindCall",
        department: Department::Performance,
        summary: "Use `bind_call(obj, args, ...)` instead of `bind(obj).call(args, ...)`.",
        explanation: "In Ruby 2.7, `UnboundMethod#bind_call` has been added.\n\nThis cop \
                      identifies places where `bind(obj).call(args, ...)` can be replaced by \
                      `bind_call(obj, args, ...)`.\n\nThe `bind_call(obj, args, ...)` method is \
                      faster than `bind(obj).call(args, ...)`.\n\n```ruby\n# bad\n\
                      umethod.bind(obj).call(foo, bar)\numethod.bind(obj).(foo, bar)\n\n# good\n\
                      umethod.bind_call(obj, foo, bar)\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { supported: options.target_ruby_version() >= 2.7 })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"call" || call.is_safe_navigation() {
            return;
        }
        let Some(bind) = call.receiver().and_then(|receiver| receiver.as_call_node()) else {
            return;
        };
        if bind.name().as_slice() != b"bind"
            || bind.is_safe_navigation()
            || bind.block().is_some_and(|block| block.as_block_node().is_some())
        {
            return;
        }
        let bind_args = argument_list(&bind);
        let [bind_arg] = bind_args.as_slice() else { return };
        let Some(selector) = bind.message_loc() else { return };

        let range = Span::new(selector.span().start, call_span_excluding_block(&call).end);
        let bind_arg = String::from_utf8_lossy(ctx.text(bind_arg.span())).into_owned();
        let call_args = argument_list(&call)
            .iter()
            .map(|arg| String::from_utf8_lossy(ctx.text(arg.span())).into_owned())
            .collect::<Vec<_>>()
            .join(", ");
        let comma = if call_args.is_empty() { "" } else { ", " };

        let message = format!(
            "Use `bind_call({bind_arg}{comma}{call_args})` instead of \
             `bind({bind_arg}).call({call_args})`."
        );
        let replacement = format!("bind_call({bind_arg}{comma}{call_args})");
        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(range, replacement.into_bytes())],
            },
        );
    }
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
