//! `Performance/TimesMap`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/times_map.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

const MESSAGE_ONLY_IF: &str = "only if `{count}` is always 0 or more";

/// Checks for .times.map calls.
#[derive(Debug, Clone)]
pub struct TimesMap;

impl Rule for TimesMap {
    const META: RuleMeta = RuleMeta {
        name: "Performance/TimesMap",
        department: Department::Performance,
        summary: "Checks for .times.map calls.",
        explanation: "",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
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
        let name = call.name();
        let name = name.as_slice();
        if name != b"map" && name != b"collect" {
            return;
        }
        // `(call (call $!nil? :times) {:map :collect} ...)`
        let Some(times) = call.receiver().and_then(|r| r.as_call_node()) else { return };
        if times.name().as_slice() != b"times"
            || times.arguments().is_some()
            || times.block().is_some()
        {
            return;
        }
        let Some(count) = times.receiver() else { return };
        // Block form (any_block, no arguments) or `(block_pass ...)` as the only argument.
        let block_pass = match call.block() {
            Some(b) if b.as_block_node().is_some() => false,
            Some(b) if b.as_block_argument_node().is_some() => true,
            _ => return,
        };
        if call.arguments().is_some() {
            return;
        }
        // `handleable_receiver?`
        let literal_number = count.as_integer_node().is_some() || count.as_float_node().is_some();
        let dot = times.call_operator_loc().is_some_and(|l| ctx.text(l.span()) == b".");
        if !(literal_number || dot) {
            return;
        }
        let count_src = String::from_utf8_lossy(ctx.text(count.span())).into_owned();
        let map_or_collect = String::from_utf8_lossy(name).into_owned();
        let mut message = format!(
            "Use `Array.new({count_src})` with a block instead of `.times.{map_or_collect}`"
        );
        if !is_literal(&count) {
            message.push(' ');
            message.push_str(&MESSAGE_ONLY_IF.replace("{count}", &count_src));
        }
        message.push('.');
        let replacement = if block_pass {
            let blk = call.block().expect("checked above");
            format!("Array.new({count_src}, {})", String::from_utf8_lossy(ctx.text(blk.span())))
        } else {
            format!("Array.new({count_src})")
        };
        let replace_span = if block_pass {
            call.as_node().span()
        } else {
            call_span_excluding_block(&call)
        };
        ctx.report_with_fix(
            &Self::META,
            call.as_node().span(),
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(replace_span, replacement.into_bytes())],
            },
        );
    }
}

/// rubocop-ast's `Node#literal?` (`LITERALS` by type).
fn is_literal(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::StringNode
            | NodeKind::InterpolatedStringNode
            | NodeKind::XStringNode
            | NodeKind::InterpolatedXStringNode
            | NodeKind::SourceFileNode
            | NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::SymbolNode
            | NodeKind::InterpolatedSymbolNode
            | NodeKind::ArrayNode
            | NodeKind::HashNode
            | NodeKind::RegularExpressionNode
            | NodeKind::InterpolatedRegularExpressionNode
            | NodeKind::TrueNode
            | NodeKind::FalseNode
            | NodeKind::NilNode
            | NodeKind::RangeNode
            | NodeKind::ImaginaryNode
            | NodeKind::RationalNode
    )
}
