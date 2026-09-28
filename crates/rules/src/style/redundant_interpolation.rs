//! `Style/RedundantInterpolation`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_interpolation.rb`.
//!
//! # whitequark → Prism shapes
//!
//! Whitequark's `dstr` children are the string's raw parts directly: a
//! shorthand `#@var`/`#$var`/`#$1` embeds the variable/reference node
//! itself as a child, while `#{...}` embeds a `begin` node wrapping the
//! statement(s) inside the braces. Prism instead always wraps a `#{...}`
//! part in an [`NodeKind::EmbeddedStatementsNode`] (holding an optional
//! `StatementsNode`) and a shorthand part in an
//! [`NodeKind::EmbeddedVariableNode`] (holding the variable/reference node
//! directly) -- so `single_interpolation?`'s `interpolation?(node)` check
//! (`variable_interpolation? || begin_type?`) becomes "the lone part is one
//! of those two node kinds".
//!
//! `variable_interpolation?` (whitequark's `VARIABLES`/`REFERENCES`) maps to
//! `LocalVariableReadNode`/`InstanceVariableReadNode`/`ClassVariableReadNode`/
//! `GlobalVariableReadNode` (variables) and `BackReferenceReadNode`/
//! `NumberedReferenceReadNode` (`$~`-style/`$1`-style references).
//!
//! `use_match_pattern?` only rejects the strict `=>` rightward-assignment
//! pattern match (whitequark's `match_pattern` node, Prism's
//! [`NodeKind::MatchRequiredNode`]) -- the boolean `in` form (whitequark's
//! `match_pattern_p`, Prism's `MatchPredicateNode`) is left alone and still
//! produces an offense, matching the two `..._in_pattern_matching` fixtures.
//!
//! Implicit string concatenation (`"#{a}" 'b'`) is not a distinct node in
//! Prism: adjacent string literals are merged into an outer
//! `InterpolatedStringNode` whose parts are the original literals (each
//! itself a nested `InterpolatedStringNode`/`StringNode`), so
//! `implicit_concatenation?`'s `node.parent&.dstr_type?` becomes "the
//! immediate parent is itself an `InterpolatedStringNode`". Likewise a
//! `%W(#{@var} foo)` element loses its own quote characters (`opening_loc`/
//! `closing_loc` become `None`) and its `ArrayNode` parent's own `opening_loc`
//! starts with `%`, which stands in for `percent_literal?`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CallNode, EmbeddedStatementsNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Prefer `to_s` over string interpolation.";

/// Checks for strings that are just an interpolated expression.
#[derive(Debug, Clone)]
pub struct RedundantInterpolation {
    target_ruby_version: f32,
}

impl Rule for RedundantInterpolation {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantInterpolation",
        department: Department::Style,
        summary: "Checks for strings that are just an interpolated expression.",
        explanation: "\
Checks for strings that are just an interpolated expression.

@safety
  Autocorrection is unsafe because when calling a destructive method to string,
  the resulting string may have different behavior or raise `FrozenError`.

  ```ruby
  x = 'a'
  y = \"#{x}\"
  y << 'b'   # return 'ab'
  x          # return 'a'
  y = x.to_s
  y << 'b'   # return 'ab'
  x          # return 'ab'

  x = 'a'.freeze
  y = \"#{x}\"
  y << 'b'   # return 'ab'.
  y = x.to_s
  y << 'b'   # raise `FrozenError`.
  ```

```ruby
# bad
\"#{@var}\"

# good
@var.to_s

# good if @var is already a String
@var
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::InterpolatedStringNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { target_ruby_version: options.target_ruby_version() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let dstr = node.as_interpolated_string_node().expect("kind matched");
        let parts = dstr.parts();
        if parts.len() != 1 {
            return;
        }
        if implicit_concatenation(ctx) || embedded_in_percent_array(ctx) {
            return;
        }

        let part = parts.iter().next().expect("len checked above");
        let Some(replacement) = self.replacement_for(&part, ctx) else { return };

        let fix = Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::replace(node.span(), replacement.into_bytes())],
        };
        ctx.report_with_fix(&Self::META, node.span(), MSG, fix);
    }
}

impl RedundantInterpolation {
    /// RuboCop's `on_dstr` body: `None` when the lone part isn't an
    /// interpolation at all, or `use_match_pattern?` vetoes it.
    fn replacement_for(&self, part: &Node<'_>, ctx: &Context<'_>) -> Option<String> {
        match part {
            Node::EmbeddedVariableNode { .. } => {
                // RuboCop's `autocorrect_variable_interpolation`: the
                // shorthand `#@var`/`#$var`/`#$1` case.
                let embedded = part.as_embedded_variable_node().expect("kind matched");
                let variable = embedded.variable();
                Some(format!("{}.to_s", text(ctx, variable.span())))
            }
            Node::EmbeddedStatementsNode { .. } => {
                let embedded = part.as_embedded_statements_node().expect("kind matched");
                self.replacement_for_statements(&embedded, ctx)
            }
            _ => None,
        }
    }

    fn replacement_for_statements(
        &self,
        embedded: &EmbeddedStatementsNode<'_>,
        ctx: &Context<'_>,
    ) -> Option<String> {
        let body = embedded.statements().map(|s| s.body());

        // RuboCop's `use_match_pattern?`.
        if self.target_ruby_version > 2.7
            && body
                .as_ref()
                .is_some_and(|b| b.iter().any(|s| s.kind() == NodeKind::MatchRequiredNode))
        {
            return None;
        }

        // RuboCop's `single_variable_interpolation?` +
        // `autocorrect_single_variable_interpolation`.
        if let Some(body) = &body {
            if body.len() == 1 {
                let stmt = body.iter().next().expect("len checked above");
                if is_variable_interpolation(&stmt) {
                    return Some(format!("{}.to_s", text(ctx, stmt.span())));
                }
                if let Node::CallNode { .. } = &stmt {
                    let call = stmt.as_call_node().expect("kind matched");
                    if !is_operator_method(call.name().as_slice()) {
                        return Some(format!("{}.to_s", call_source(&call, ctx)));
                    }
                }
            }
        }

        // RuboCop's `autocorrect_other`: wrap the `#{...}` contents in
        // parentheses in place.
        let inner_start = embedded.opening_loc().span().end;
        let inner_end = embedded.closing_loc().span().start;
        let inner = text(ctx, Span::new(inner_start, inner_end));
        Some(format!("({inner}).to_s"))
    }
}

/// RuboCop's `implicit_concatenation?`: adjacent string literals merge into
/// an outer `InterpolatedStringNode` whose parts are the original literals.
fn implicit_concatenation(ctx: &Context<'_>) -> bool {
    ctx.parent().is_some_and(|p| p.kind == NodeKind::InterpolatedStringNode)
}

/// RuboCop's `embedded_in_percent_array?`: a `%W(...)`/`%I(...)` element
/// loses its own quotes, and the array's own opening delimiter starts with
/// `%`.
fn embedded_in_percent_array(ctx: &Context<'_>) -> bool {
    ctx.parent()
        .is_some_and(|p| p.kind == NodeKind::ArrayNode && ctx.text(p.span).first() == Some(&b'%'))
}

/// RuboCop's `variable_interpolation?` (`node.variable? || node.reference?`).
fn is_variable_interpolation(node: &Node<'_>) -> bool {
    matches!(
        node,
        Node::LocalVariableReadNode { .. }
            | Node::InstanceVariableReadNode { .. }
            | Node::ClassVariableReadNode { .. }
            | Node::GlobalVariableReadNode { .. }
            | Node::BackReferenceReadNode { .. }
            | Node::NumberedReferenceReadNode { .. }
    )
}

/// RuboCop's `autocorrect_single_variable_interpolation`'s `source`
/// computation: wraps bare (unparenthesized) call arguments in parentheses,
/// otherwise uses the call's own source verbatim.
fn call_source(call: &CallNode<'_>, ctx: &Context<'_>) -> String {
    let has_args = call.arguments().is_some_and(|a| !a.arguments().is_empty());
    let parenthesized = call.opening_loc().is_some();
    if has_args && !parenthesized {
        let message_end = call.message_loc().map_or(call.as_node().span().end, |l| l.span().end);
        let receiver = text(ctx, Span::new(call.as_node().span().start, message_end));
        let args: Vec<String> = call
            .arguments()
            .expect("has_args checked")
            .arguments()
            .iter()
            .map(|a| text(ctx, a.span()))
            .collect();
        format!("{receiver}({})", args.join(", "))
    } else {
        text(ctx, call.as_node().span())
    }
}

/// RuboCop-AST's `OPERATOR_METHODS`.
fn is_operator_method(name: &[u8]) -> bool {
    matches!(
        name,
        b"|" | b"^"
            | b"&"
            | b"<=>"
            | b"=="
            | b"==="
            | b"=~"
            | b">"
            | b">="
            | b"<"
            | b"<="
            | b"<<"
            | b">>"
            | b"+"
            | b"-"
            | b"*"
            | b"/"
            | b"%"
            | b"**"
            | b"~"
            | b"+@"
            | b"-@"
            | b"!@"
            | b"~@"
            | b"[]"
            | b"[]="
            | b"!"
            | b"!="
            | b"!~"
            | b"`"
    )
}

/// Decodes `span`'s source bytes as UTF-8, replacing invalid sequences (Ruby
/// source is not guaranteed to be valid UTF-8, but every node this cop
/// touches is plain identifier/operator/expression text).
fn text(ctx: &Context<'_>, span: Span) -> String {
    String::from_utf8_lossy(ctx.text(span)).into_owned()
}
