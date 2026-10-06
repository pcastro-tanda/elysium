//! `Performance/InefficientHashSearch`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/inefficient_hash_search.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue,
    Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Use `key?` or `value?` instead of `keys.include?` or `values.include?`.
#[derive(Debug, Clone)]
pub struct InefficientHashSearch {
    use_long_method: bool,
}

impl Rule for InefficientHashSearch {
    const META: RuleMeta = RuleMeta {
        name: "Performance/InefficientHashSearch",
        department: Department::Performance,
        summary: "Use `key?` or `value?` instead of `keys.include?` or `values.include?`.",
        explanation: "Checks for inefficient searching of keys and values within hashes. `Hash#keys.include?` allocates an array and does an O(n) search, while `Hash#key?` is O(1); `Hash#values.include?` allocates an array, while `Hash#value?` does not. Unsafe because the receiver may not be a hash.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        // `config.for_enabled_cop('Style/PreferredHashMethods')['EnforcedStyle']`:
        // a disabled cop yields an empty config.
        let enabled = !matches!(
            options.peer("Style/PreferredHashMethods", "Enabled"),
            Some(OptionValue::Bool(false) | OptionValue::Null)
        );
        let use_long_method = enabled
            && matches!(
                options.peer("Style/PreferredHashMethods", "EnforcedStyle"),
                Some(OptionValue::Str(style)) if style == "long"
            );
        Ok(Self { use_long_method })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"include?" {
            return;
        }
        let Some(first_argument) = sole_argument(&call) else { return };
        let Some(inner_node) = call.receiver() else { return };
        let Some(inner) = inner_node.as_call_node() else { return };
        let current = inner.name();
        let current = current.as_slice();
        if (current != b"keys" && current != b"values")
            || inner.arguments().is_some()
            || inner.block().is_some()
        {
            return;
        }
        let Some(hash) = inner.receiver() else { return };

        let correct: &str = match (current, self.use_long_method) {
            (b"keys", true) => "has_key?",
            (b"keys", false) => "key?",
            (_, true) => "has_value?",
            (_, false) => "value?",
        };
        let current = String::from_utf8_lossy(current);
        let message = format!("Use `#{correct}` instead of `#{current}.include?`.");

        let dot = inner.call_operator_loc().map_or(&b""[..], |loc| ctx.text(loc.span()));
        let mut replacement = Vec::new();
        replacement.extend_from_slice(ctx.text(hash.span()));
        replacement.extend_from_slice(dot);
        replacement.extend_from_slice(correct.as_bytes());
        replacement.push(b'(');
        replacement.extend_from_slice(ctx.text(first_argument));
        replacement.push(b')');

        let mut span = call_span_excluding_block(&call);
        if let Some(block) = call.block() {
            if block.kind() == NodeKind::BlockArgumentNode {
                span.end = span.end.max(block.span().end);
            }
        }
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, replacement)],
            },
        );
    }
}

/// The span of the call's only argument, counting a `&blk` block-pass as an
/// argument (whitequark keeps it in the argument list).
fn sole_argument(call: &CallNode<'_>) -> Option<Span> {
    let block_pass = call.block().filter(|b| b.kind() == NodeKind::BlockArgumentNode);
    match call.arguments() {
        Some(args) => {
            let mut it = args.arguments().iter();
            let first = it.next()?;
            (it.next().is_none() && block_pass.is_none()).then(|| first.span())
        }
        None => block_pass.map(|b| b.span()),
    }
}
