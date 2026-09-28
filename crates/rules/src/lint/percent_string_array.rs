//! `Lint/PercentStringArray`, ported from RuboCop's
//! `lib/rubocop/cop/lint/percent_string_array.rb` plus the `PercentLiteral`
//! mixin it includes (`lib/rubocop/cop/mixin/percent_literal.rb`).
//!
//! `%w(...)`/`%W(...)` word arrays are one [`NodeKind::ArrayNode`] whose
//! `opening_loc` starts with `%w`/`%W` (RuboCop-AST's `percent_literal?` plus
//! `type(node)`); each word is a plain [`NodeKind::StringNode`] unless a `%W`
//! word actually interpolates (`#{...}`), which becomes an
//! [`NodeKind::InterpolatedStringNode`] instead -- upstream's
//! `value.children.first.to_s` on such a node stringifies the *AST node*
//! itself (never starting/ending with a quote character), so it can never
//! trip `QUOTES_AND_COMMAS`; detection here likewise only ever inspects plain
//! `StringNode` words. Autocorrection, however, is unconditional over *every*
//! word (`node.each_value`), so it still runs on an interpolated word's raw
//! source text -- reproduced the same way here, using each element's own
//! span text regardless of kind.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Within `%w`/`%W`, quotes and ',' are unnecessary and may be \
unwanted in the resulting strings.";

/// Checks for unwanted commas and quotes in %w/%W literals.
#[derive(Debug, Clone)]
pub struct PercentStringArray;

impl Rule for PercentStringArray {
    const META: RuleMeta = RuleMeta {
        name: "Lint/PercentStringArray",
        department: Department::Lint,
        summary: "Checks for unwanted commas and quotes in %w/%W literals.",
        explanation: "\
Checks for quotes and commas in `%w`, e.g. `%w('foo', \"bar\")`

It is more likely that the additional characters are unintended (for
example, mistranslating an array of literals to percent string notation)
rather than meant to be part of the resulting strings.

```ruby
# bad
%w('foo', \"bar\")

# good
%w(foo bar)
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
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
            && matches!(opening_bytes[1], b'w' | b'W'))
        {
            return;
        }
        let elements: Vec<Node<'_>> = array.elements().iter().collect();
        if !contains_quotes_or_commas(&elements) {
            return;
        }
        let edits = string_array_edits(&elements, ctx);
        let fix = Fix { applicability: Applicability::Unsafe, edits };
        ctx.report_with_fix(&Self::META, node.span(), MSG, fix);
    }
}

/// RuboCop's `contains_quotes_or_commas?`.
fn contains_quotes_or_commas(elements: &[Node<'_>]) -> bool {
    elements.iter().any(|value| {
        let Some(sn) = value.as_string_node() else { return false };
        let scrubbed = String::from_utf8_lossy(sn.unescaped());
        let stripped: String = scrubbed.chars().filter(|c| c.is_alphanumeric()).collect();
        if stripped.is_empty() {
            return false;
        }
        scrubbed.split('\n').any(line_triggers)
    })
}

/// RuboCop's `QUOTES_AND_COMMAS = [/,$/, /^'.*'$/, /^".*"$/]`, matched
/// per-line the way Ruby's `^`/`$` do inside a (possibly multiline) string.
fn line_triggers(line: &str) -> bool {
    let bytes = line.as_bytes();
    bytes.last() == Some(&b',')
        || (bytes.len() >= 2 && bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\'')
        || (bytes.len() >= 2 && bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
}

/// RuboCop's `on_percent_literal`'s autocorrect block: `TRAILING_QUOTE =
/// /['"]?,?$/` (an optional quote then an optional comma, matched against
/// each word's own raw source) followed by `LEADING_QUOTE = /^['"]/`.
fn string_array_edits(elements: &[Node<'_>], ctx: &Context<'_>) -> Vec<Edit> {
    let mut edits = Vec::new();
    for element in elements {
        let span = element.span();
        let raw = ctx.text(span);
        if raw.is_empty() {
            continue;
        }
        let trailing_len = trailing_quote_comma_len(raw);
        let leading_len = usize::from(matches!(raw[0], b'\'' | b'"'));
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

/// Length of `['"]?,?$`'s match against `raw`'s tail: a trailing
/// quote-then-comma (2), a lone trailing quote or comma (1), or no match (0).
fn trailing_quote_comma_len(raw: &[u8]) -> usize {
    if raw.len() >= 2 && matches!(raw[raw.len() - 2], b'\'' | b'"') && raw[raw.len() - 1] == b',' {
        return 2;
    }
    match raw.last() {
        Some(b'\'' | b'"' | b',') => 1,
        _ => 0,
    }
}
