//! `Style/RedundantCapitalW`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_capital_w.rb`, including the
//! `PercentLiteral` mixin (`lib/rubocop/cop/mixin/percent_literal.rb`) it
//! includes: Prism gives every array literal (bracketed or `%w`/`%W`) one
//! `NodeKind::ArrayNode`, and the percent-literal form is told apart by its
//! own opening delimiter's source text starting with `%W` (RuboCop-AST's
//! `percent_literal?`/`type`).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Do not use `%W` unless interpolation is needed. If not, use `%w`.";

/// Checks for usage of the `%W()` syntax when `%w()` would do.
#[derive(Debug, Clone)]
pub struct RedundantCapitalW;

impl Rule for RedundantCapitalW {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantCapitalW",
        department: Department::Style,
        summary: "Checks for %W when interpolation is not needed.",
        explanation: "\
Checks for usage of the `%W()` syntax when `%w()` would do.

```ruby
# bad
%W(cat dog pig)
%W[door wall floor]

# good
%w/swim run bike/
%w[shirt pants shoes]
%W(apple #{fruit} grape)
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ArrayNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(array) = node.as_array_node() else { return };
        let Some(opening) = array.opening_loc() else { return };
        let opening_bytes = opening.as_slice();
        // RuboCop-AST's `PercentLiteral#type`: the opening delimiter's
        // source text with its final (bracket) character dropped, compared
        // against `'%W'`.
        if opening_bytes.len() < 2 || &opening_bytes[..2] != b"%W" {
            return;
        }

        let elements: Vec<Node<'_>> = array.elements().iter().collect();
        if requires_interpolation(&elements, ctx) {
            return;
        }

        let corrected: Vec<u8> =
            opening_bytes.iter().map(|&b| if b == b'W' { b'w' } else { b }).collect();
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(opening.span(), corrected)],
        };
        ctx.report_with_fix(&Self::META, node.span(), MSG, fix);
    }
}

/// RuboCop's `requires_interpolation?`: any word is itself interpolated, or
/// its raw (unescaped) source text needs double quotes to represent as a
/// regular string literal (i.e. it would change meaning if written into a
/// `%w()`).
fn requires_interpolation(elements: &[Node<'_>], ctx: &Context<'_>) -> bool {
    elements.iter().any(|element| {
        if element.kind() == NodeKind::InterpolatedStringNode {
            return true;
        }
        let Some(string_node) = element.as_string_node() else { return false };
        double_quotes_required(ctx.text(string_node.content_loc().span()))
    })
}

/// RuboCop's `Util#double_quotes_required?`: a literal `'`, or a
/// (non-doubled) backslash escape sequence not immediately followed by
/// another backslash or a `"`.
fn double_quotes_required(bytes: &[u8]) -> bool {
    if bytes.contains(&b'\'') {
        return true;
    }
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            let start = i;
            while i < bytes.len() && bytes[i] == b'\\' {
                i += 1;
            }
            if (i - start) % 2 == 1 && bytes.get(i) != Some(&b'"') {
                return true;
            }
        } else {
            i += 1;
        }
    }
    false
}
