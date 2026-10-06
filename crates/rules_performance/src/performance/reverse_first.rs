//! `Performance/ReverseFirst`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/reverse_first.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Use `last(n).reverse` instead of `reverse.first(n)`.
#[derive(Debug, Clone)]
pub struct ReverseFirst;

impl Rule for ReverseFirst {
    const META: RuleMeta = RuleMeta {
        name: "Performance/ReverseFirst",
        department: Department::Performance,
        summary: "Use `last(n).reverse` instead of `reverse.first(n)`.",
        explanation: "Identifies places where `reverse.first(n)` and `reverse.first` can be \
                      replaced by `last(n).reverse` and `last`.\n\n```ruby\n# bad\n\
                      array.reverse.first(5)\narray.reverse.first\n\n# good\n\
                      array.last(5).reverse\narray.last\n```",
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

    /// `(call $(call _ :reverse) :first (int _)?)`.
    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"first" {
            return;
        }
        if call.block().is_some_and(|b| b.as_block_argument_node().is_some()) {
            return;
        }
        let first_arg = match call.arguments() {
            None => None,
            Some(args) => {
                let list: Vec<_> = args.arguments().iter().collect();
                match list.as_slice() {
                    [arg] if arg.as_integer_node().is_some() => Some(arg.span()),
                    _ => return,
                }
            }
        };
        let Some(receiver) = call.receiver() else { return };
        let Some(reverse) = receiver.as_call_node() else { return };
        if reverse.name().as_slice() != b"reverse" || reverse.receiver().is_none() {
            return;
        }
        let Some(selector) = reverse.message_loc() else { return };
        let range = Span::new(selector.span().start, call_span_excluding_block(&call).end);
        let good = match first_arg {
            Some(arg) => {
                let dot = call.call_operator_loc().map_or(&b""[..], |l| ctx.text(l.span()));
                format!(
                    "last({}){}reverse",
                    String::from_utf8_lossy(ctx.text(arg)),
                    String::from_utf8_lossy(dot)
                )
            }
            None => "last".to_string(),
        };
        let message = format!(
            "Use `{good}` instead of `{}`.",
            String::from_utf8_lossy(ctx.text(range))
        );
        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(range, good.into_bytes())],
            },
        );
    }
}
