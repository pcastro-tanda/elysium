//! `Lint/PercentSymbolArray`, ported from RuboCop's
//! `lib/rubocop/cop/lint/percent_symbol_array.rb` plus the `PercentLiteral`
//! mixin it includes (`lib/rubocop/cop/mixin/percent_literal.rb`).
//!
//! `%i(...)`/`%I(...)` symbol arrays are one [`NodeKind::ArrayNode`] whose
//! `opening_loc` starts with `%i`/`%I` (RuboCop-AST's `percent_literal?` plus
//! `type(node)`); each symbol is a plain [`NodeKind::SymbolNode`] unless a
//! `%I` symbol actually interpolates (`#{...}`), which becomes an
//! [`NodeKind::InterpolatedSymbolNode`] instead -- upstream's
//! `child.children.first.to_s` on such a node stringifies the *AST node*
//! itself (never starting with `:` or ending with `,`), so it can never trip
//! `contains_colons_or_commas?`; detection here likewise only ever inspects
//! plain `SymbolNode` symbols. Autocorrection, however, is unconditional over
//! *every* child (`node.children.each`), so it still runs on an interpolated
//! symbol's raw source text -- reproduced the same way here, using each
//! element's own span text regardless of kind.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Within `%i`/`%I`, ':' and ',' are unnecessary and may be \
unwanted in the resulting symbols.";

/// Checks for unwanted commas and colons in %i/%I literals.
#[derive(Debug, Clone)]
pub struct PercentSymbolArray;

impl Rule for PercentSymbolArray {
    const META: RuleMeta = RuleMeta {
        name: "Lint/PercentSymbolArray",
        department: Department::Lint,
        summary: "Checks for unwanted commas and colons in %i/%I literals.",
        explanation: "\
Checks for colons and commas in `%i`, e.g. `%i(:foo, :bar)`

It is more likely that the additional characters are unintended (for
example, mistranslating an array of literals to percent string notation)
rather than meant to be part of the resulting symbols.

```ruby
# bad
%i(:foo, :bar)

# good
%i(foo bar)
```",
        enabled_by_default: true,
        severity: Severity::Warning,
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
        if !(opening_bytes.len() >= 2
            && opening_bytes[0] == b'%'
            && matches!(opening_bytes[1], b'i' | b'I'))
        {
            return;
        }
        let elements: Vec<Node<'_>> = array.elements().iter().collect();
        if !contains_colons_or_commas(&elements) {
            return;
        }
        let edits = symbol_array_edits(&elements, ctx);
        let fix = Fix { applicability: Applicability::Safe, edits };
        ctx.report_with_fix(&Self::META, node.span(), MSG, fix);
    }
}

/// RuboCop's `contains_colons_or_commas?`/`non_alphanumeric_literal?`.
fn contains_colons_or_commas(elements: &[Node<'_>]) -> bool {
    elements.iter().any(|child| {
        let Some(sym) = child.as_symbol_node() else { return false };
        let literal = String::from_utf8_lossy(sym.unescaped());
        if !literal.chars().any(char::is_alphanumeric) {
            return false;
        }
        literal.starts_with(':') || literal.ends_with(',')
    })
}

/// RuboCop's `autocorrect`: removes a trailing `,` and/or a leading `:` from
/// each child's own raw source text, unconditionally.
fn symbol_array_edits(elements: &[Node<'_>], ctx: &Context<'_>) -> Vec<Edit> {
    let mut edits = Vec::new();
    for element in elements {
        let span = element.span();
        let raw = ctx.text(span);
        if raw.is_empty() {
            continue;
        }
        let trailing_len = usize::from(raw.last() == Some(&b','));
        let leading_len = usize::from(raw[0] == b':');
        let mut start = leading_len;
        let end = raw.len() - trailing_len;
        if start > end {
            start = end;
        }
        if start == 0 && end == raw.len() {
            continue;
        }
        edits.push(Edit::replace(span, raw[start..end].to_vec()));
    }
    edits
}
