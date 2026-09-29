//! `Lint/ImplicitStringConcatenation`, ported from RuboCop's
//! `lib/rubocop/cop/lint/implicit_string_concatenation.rb`.
//!
//! # whitequark → Prism shapes
//!
//! Whitequark parses adjacent string literals (`"abc" "def"`) into a `dstr`
//! node whose direct children are the original literals (`str`/`dstr`
//! nodes), so `on_dstr` walks `node.children.each_cons(2)`. Prism does the
//! same: adjacent literals merge into an outer `InterpolatedStringNode`
//! whose `parts` are the original literals (each itself a nested
//! `InterpolatedStringNode`/`StringNode`), so this port walks
//! `parts().windows(2)` the same way.
//!
//! `node.parent&.send_type?` (is this the argument of a method call?) does
//! not translate directly: Prism always wraps a `CallNode`'s arguments in an
//! `ArgumentsNode`, so the dstr's immediate parent is `ArgumentsNode`, not
//! `CallNode`. This port checks the grandparent instead.
//!
//! `lhs_node.value`/`rhs_node.value` (rubocop-ast's `DstrNode#value`, which
//! is `''` only when every part is itself string content) becomes
//! [`node_is_empty_string`]: `false` as soon as any part is not a
//! `StringNode`/`InterpolatedStringNode` made only of empty string content
//! (an interpolation's raw source is never empty, matching upstream's
//! `child.source` fallback for non-string children).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `FOR_ARRAY`.
const FOR_ARRAY: &str = " Or, if they were intended to be separate array \
elements, separate them with a comma.";
/// RuboCop's `FOR_METHOD`.
const FOR_METHOD: &str = " Or, if they were intended to be separate method \
arguments, separate them with a comma.";

/// Checks for implicit string concatenation of string literals which are on
/// the same line.
#[derive(Debug, Clone)]
pub struct ImplicitStringConcatenation;

impl Rule for ImplicitStringConcatenation {
    const META: RuleMeta = RuleMeta {
        name: "Lint/ImplicitStringConcatenation",
        department: Department::Lint,
        summary: "Checks for adjacent string literals on the same line, which could better be represented as a single string literal.",
        explanation: "\
Checks for implicit string concatenation of string literals
which are on the same line.

```ruby
# bad
array = ['Item 1' 'Item 2']

# good
array = ['Item 1Item 2']
array = ['Item 1' + 'Item 2']
array = [
  'Item 1' \\
  'Item 2'
]
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::InterpolatedStringNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(dstr) = node.as_interpolated_string_node() else { return };
        let parts: Vec<Node<'_>> = dstr.parts().iter().collect();
        if parts.len() < 2 {
            return;
        }

        let suffix = message_suffix(ctx);

        for pair in parts.windows(2) {
            let (lhs, rhs) = (&pair[0], &pair[1]);
            if !is_string_literal(lhs) || !is_string_literal(rhs) {
                continue;
            }
            if ctx.last_line(lhs.span()) != ctx.line_col(rhs.span().start).line {
                continue;
            }

            // Implicit string concatenation does not work with `%{}`, etc.:
            // make sure `lhs` genuinely ends with the quote it opened with.
            let lhs_source = ctx.text(lhs.span());
            let Some(&opening) = lhs_source.first() else { continue };
            let closing = match opening {
                b'\'' => b'\'',
                b'"' => b'"',
                _ => continue,
            };
            if lhs_source.last() != Some(&closing) {
                continue;
            }

            let span = Span::new(lhs.span().start, rhs.span().end);
            let mut message = format!(
                "Combine {} and {} into a single string literal, rather than using implicit string concatenation.",
                display_str(ctx, lhs),
                display_str(ctx, rhs),
            );
            message.push_str(suffix);

            let edits = if node_is_empty_string(lhs, ctx) {
                vec![Edit::delete(lhs.span())]
            } else if node_is_empty_string(rhs, ctx) {
                vec![Edit::delete(rhs.span())]
            } else {
                let between = Span::new(lhs.span().end, rhs.span().start);
                vec![Edit::replace(between, b" + ".to_vec())]
            };

            ctx.report_with_fix(
                &Self::META,
                span,
                message,
                Fix { applicability: Applicability::Safe, edits },
            );
        }
    }
}

/// RuboCop's `string_literal?`: `node.type?(:str, :dstr)`.
fn is_string_literal(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::StringNode | NodeKind::InterpolatedStringNode)
}

/// RuboCop's `node.parent&.array_type?` / `node.parent&.send_type?`, adjusted
/// for Prism wrapping call arguments in an `ArgumentsNode`: the dstr's
/// parent is `ArgumentsNode`, and the call itself is the grandparent.
fn message_suffix(ctx: &Context<'_>) -> &'static str {
    let ancestors = ctx.ancestors();
    let Some(parent) = ancestors.last() else { return "" };
    match parent.kind {
        NodeKind::ArrayNode => FOR_ARRAY,
        NodeKind::ArgumentsNode
            if ancestors.len() >= 2
                && ancestors[ancestors.len() - 2].kind == NodeKind::CallNode =>
        {
            FOR_METHOD
        }
        _ => "",
    }
}

/// RuboCop's `display_str`: the node's raw source, unless it contains a
/// literal newline, in which case its cooked string content is rendered via
/// Ruby's `String#inspect`.
fn display_str(ctx: &Context<'_>, node: &Node<'_>) -> String {
    let source = ctx.text(node.span());
    if source.contains(&b'\n') {
        let content = str_content(node);
        let mut out = String::from("\"");
        out.push_str(&String::from_utf8_lossy(&ruby_inspect_body(&content)));
        out.push('"');
        out
    } else {
        String::from_utf8_lossy(source).into_owned()
    }
}

/// RuboCop's `str_content`: the concatenation of every string part's cooked
/// content; non-string parts (interpolations) contribute nothing.
fn str_content(node: &Node<'_>) -> Vec<u8> {
    match node.kind() {
        NodeKind::StringNode => {
            node.as_string_node().map(|n| n.unescaped().to_vec()).unwrap_or_default()
        }
        NodeKind::InterpolatedStringNode => node
            .as_interpolated_string_node()
            .map(|n| n.parts().iter().flat_map(|p| str_content(&p)).collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// RuboCop's `lhs_node.value == ''` / `rhs_node.value == ''`: true only when
/// every string part is itself empty (an interpolation's raw source is
/// never empty).
fn node_is_empty_string(node: &Node<'_>, ctx: &Context<'_>) -> bool {
    let _ = ctx;
    match node.kind() {
        NodeKind::StringNode => node.as_string_node().is_some_and(|n| n.unescaped().is_empty()),
        NodeKind::InterpolatedStringNode => node
            .as_interpolated_string_node()
            .is_some_and(|n| n.parts().iter().all(|p| node_is_empty_string(&p, ctx))),
        _ => false,
    }
}

/// RuboCop's double-quoted `value.inspect[1..-2]` body escaping (copied
/// privately from `style/string_concatenation.rs`).
fn ruby_inspect_body(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'"' => out.extend_from_slice(b"\\\""),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\t' => out.extend_from_slice(b"\\t"),
            b'\r' => out.extend_from_slice(b"\\r"),
            0x00 => out.extend_from_slice(b"\\0"),
            0x07 => out.extend_from_slice(b"\\a"),
            0x08 => out.extend_from_slice(b"\\b"),
            0x0C => out.extend_from_slice(b"\\f"),
            0x0B => out.extend_from_slice(b"\\v"),
            0x1B => out.extend_from_slice(b"\\e"),
            b'#' if matches!(bytes.get(i + 1), Some(b'{' | b'@' | b'$')) => {
                out.push(b'\\');
                out.push(b'#');
                out.push(bytes[i + 1]);
                i += 2;
                continue;
            }
            0x20..=0x7E => out.push(b),
            _ if b < 0x20 || b == 0x7F => {
                out.extend_from_slice(format!("\\x{b:02X}").as_bytes());
            }
            _ => out.push(b),
        }
        i += 1;
    }
    out
}
