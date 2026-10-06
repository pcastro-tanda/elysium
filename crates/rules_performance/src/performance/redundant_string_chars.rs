//! `Performance/RedundantStringChars`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/redundant_string_chars.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const RESTRICT_ON_SEND: [&[u8]; 8] =
    [b"[]", b"slice", b"first", b"last", b"take", b"length", b"size", b"empty?"];

/// Checks for redundant `String#chars`.
#[derive(Debug, Clone)]
pub struct RedundantStringChars;

impl Rule for RedundantStringChars {
    const META: RuleMeta = RuleMeta {
        name: "Performance/RedundantStringChars",
        department: Department::Performance,
        summary: "Checks for redundant `String#chars`.",
        explanation: "",
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
        if call.is_safe_navigation() || !RESTRICT_ON_SEND.contains(&call.name().as_slice()) {
            return;
        }
        let method = call.name();
        let method = std::str::from_utf8(method.as_slice()).unwrap_or("");
        // `(send $(send !nil? :chars) $_ $...)`
        let Some(chars) = call.receiver().and_then(|r| r.as_call_node()) else { return };
        if chars.is_safe_navigation()
            || chars.name().as_slice() != b"chars"
            || chars.receiver().is_none()
            || chars.arguments().is_some()
            || chars.block().is_some()
        {
            return;
        }
        let mut args: Vec<Node<'_>> = Vec::new();
        if let Some(arguments) = call.arguments() {
            args.extend(arguments.arguments().iter());
        }
        if let Some(block) = call.block().filter(|b| b.as_block_argument_node().is_some()) {
            args.push(block);
        }
        if method == "last" && !args.is_empty() {
            return;
        }
        if args.len() == 2 {
            return;
        }
        let Some(selector) = chars.message_loc() else { return };
        let Some(dot) = chars.call_operator_loc() else { return };
        let end = send_span(&call).end;
        let range = Span::new(selector.span().start, end);
        let sources: Vec<&[u8]> = args.iter().map(|a| ctx.text(a.span())).collect();
        let call_args = sources.iter().map(|s| String::from_utf8_lossy(s)).collect::<Vec<_>>().join(", ");
        let good = build_good_method(method, &args, &call_args, ctx);
        let bad = if method == "[]" {
            format!("chars[{call_args}]")
        } else if args.is_empty() {
            format!("chars.{method}")
        } else {
            format!("chars.{method}({call_args})")
        };
        ctx.report_with_fix(
            &Self::META,
            range,
            format!("Use `{good}` instead of `{bad}`."),
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(Span::new(dot.span().start, end), good.into_bytes())],
            },
        );
    }
}

/// The call's own span: excludes an attached `do ... end`/`{ }` block but keeps a `&blk` argument.
fn send_span(call: &CallNode<'_>) -> Span {
    if call.block().is_some_and(|b| b.as_block_argument_node().is_some()) {
        call.as_node().span()
    } else {
        call_span_excluding_block(call)
    }
}

fn build_good_method(method: &str, args: &[Node<'_>], call_args: &str, ctx: &Context<'_>) -> String {
    let first_source = |a: &Node<'_>| String::from_utf8_lossy(ctx.text(a.span())).into_owned();
    match method {
        "slice" => format!("[{call_args}].chars"),
        "[]" | "first" => {
            let first = args.first();
            if first.is_some_and(|a| a.as_range_node().is_some()) {
                format!("[{call_args}].chars")
            } else if method == "first" && !args.is_empty() {
                format!("[0...{}].chars", first_source(&args[0]))
            } else if let Some(first) = first {
                format!("[{}]", first_source(first))
            } else {
                "[0]".to_owned()
            }
        }
        "last" => "[-1]".to_owned(),
        "take" => format!("[0...{}].chars", args.first().map_or_else(String::new, first_source)),
        _ => format!(".{method}"),
    }
}
