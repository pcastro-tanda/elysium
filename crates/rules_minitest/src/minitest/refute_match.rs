//! `Minitest/RefuteMatch`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/refute_match.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Enforces the test to use `refute_match` instead of using
/// `refute(matcher.match(string))`.
#[derive(Debug, Clone)]
pub struct RefuteMatch;

impl Rule for RefuteMatch {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/RefuteMatch",
        department: Department::Minitest,
        summary: "This cop enforces the test to use `refute_match` instead of using `refute(matcher.match(object))`.",
        explanation: "Enforces the test to use `refute_match` instead of using \
                      `refute(matcher.match(string))`.\n\n```ruby\n# bad\n\
                      refute(matcher.match(string))\nrefute(matcher.match?(string))\n\
                      refute(matcher =~ string)\nrefute_operator(matcher, :=~, string)\nassert_operator(matcher, :!~, string)\nrefute(matcher.match(string), 'message')\n\n\
                      # good\nrefute_match(matcher, string)\n\
                      refute_match(matcher, string, 'message')\n```",
        enabled_by_default: true,
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
        let Some((expected, actual)) = refute_match(&call, &arguments) else { return };

        let (first, second) = if actual.as_regular_expression_node().is_some()
            || actual.as_interpolated_regular_expression_node().is_some()
        {
            (actual, expected)
        } else {
            (expected, actual)
        };
        let basic_arguments = format!(
            "{}, {}",
            String::from_utf8_lossy(ctx.text(first.span())),
            String::from_utf8_lossy(ctx.text(second.span()))
        );
        let rest_start = if call.name().as_slice() == b"refute" { 1 } else { 3 };
        let preferred = match arguments.get(rest_start) {
            Some(message) => {
                format!("{basic_arguments}, {}", String::from_utf8_lossy(ctx.text(message.span())))
            }
            None => basic_arguments.clone(),
        };
        let message = format!("Prefer using `refute_match({preferred})`.");

        let Some(selector) = call.message_loc() else { return };
        let range = if call.name().as_slice() == b"refute" {
            arguments[0].span()
        } else {
            Span::new(arguments[0].span().start, arguments[2].span().end)
        };
        let edits = vec![
            Edit::replace(selector.span(), b"refute_match".to_vec()),
            Edit::replace(range, basic_arguments.into_bytes()),
        ];
        ctx.report_with_fix(
            &Self::META,
            call_span_excluding_block(&call),
            message,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}

/// The `refute_match` node pattern:
///
/// ```text
/// {
///   (send nil? :refute (send $_ {:match :match? :=~} $_) $...)
///   (send nil? :refute_operator $_ (sym :=~) $_ $...)
///   (send nil? :assert_operator $_ (sym :!~) $_ $...)
/// }
/// ```
///
/// Returns `($expected, $actual)`.
fn refute_match<'pr>(
    call: &CallNode<'pr>,
    arguments: &[Node<'pr>],
) -> Option<(Node<'pr>, Node<'pr>)> {
    let name = call.name();
    let name = name.as_slice();
    match name {
        b"refute" => {
            let inner = arguments.first()?.as_call_node()?;
            if !matches!(inner.name().as_slice(), b"match" | b"match?" | b"=~")
                || inner.is_safe_navigation()
                || inner.block().is_some_and(|block| block.as_block_argument_node().is_none())
            {
                return None;
            }
            let inner_arguments = argument_list(&inner);
            let [actual] = inner_arguments.as_slice() else { return None };
            Some((inner.receiver()?, *actual))
        }
        b"refute_operator" => {
            let [expected, operator, actual, ..] = arguments else { return None };
            if operator.as_symbol_node()?.unescaped() != b"=~" {
                return None;
            }
            Some((*expected, *actual))
        }
        b"assert_operator" => {
            let [expected, operator, actual, ..] = arguments else { return None };
            if operator.as_symbol_node()?.unescaped() != b"!~" {
                return None;
            }
            Some((*expected, *actual))
        }
        _ => None,
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
