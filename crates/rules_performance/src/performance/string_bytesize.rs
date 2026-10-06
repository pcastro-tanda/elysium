//! `Performance/StringBytesize`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/string_bytesize.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

const MSG: &str = "Use `String#bytesize` instead of calculating the size of the bytes array.";

/// Checks for calls to `#bytes` counting method and suggests using `bytesize` instead.
#[derive(Debug, Clone)]
pub struct StringBytesize;

impl Rule for StringBytesize {
    const META: RuleMeta = RuleMeta {
        name: "Performance/StringBytesize",
        department: Department::Performance,
        summary: "Use `String#bytesize` instead of calculating the size of the bytes array.",
        explanation: "Checks for calls to `#bytes` counting method and suggests using `bytesize` \
                      instead. The `bytesize` method is more efficient and directly returns the \
                      size in bytes, avoiding the intermediate array allocation that `bytes.size` \
                      incurs.\n\nThis cop is unsafe because it assumes that the receiver responds \
                      to `#bytesize` method.\n\n```ruby\n# bad\nstring_var.bytes.count\n\
                      \"foobar\".bytes.size\n\n# good\nstring_var.bytesize\n\"foobar\".bytesize\n```",
        enabled_by_default: false,
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
        if !matches!(call.name().as_slice(), b"size" | b"length" | b"count") {
            return;
        }
        // The outer call takes no arguments (a `&blk` argument would be one).
        if call.arguments().is_some() || call.block().is_some_and(|b| b.as_block_node().is_none()) {
            return;
        }
        let Some(bytes) = call.receiver().and_then(|r| r.as_call_node()) else { return };
        if bytes.name().as_slice() != b"bytes"
            || bytes.arguments().is_some()
            || bytes.block().is_some()
        {
            return;
        }
        let Some(bytes_receiver) = bytes.receiver() else { return };
        if bytes_receiver.as_integer_node().is_some() {
            return;
        }
        let Some(selector) = bytes.message_loc() else { return };
        let end = ruby_ast::ext::call_span_excluding_block(&call).end;
        let range = Span::new(selector.span().start, end);
        ctx.report_with_fix(
            &Self::META,
            range,
            MSG,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(range, b"bytesize".to_vec())],
            },
        );
    }
}
