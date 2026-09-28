//! `Lint/NestedPercentLiteral`, ported from RuboCop's
//! `lib/rubocop/cop/lint/nested_percent_literal.rb` plus the `PercentLiteral`
//! mixin's `process`/`percent_literal?`/`type` helpers it uses.
//!
//! Upstream subscribes to every `array` node and only proceeds when the
//! node's own opening delimiter is a percent literal whose type (the
//! delimiter's source text minus its trailing opening-bracket character,
//! e.g. `"%i"` for `%i[...]`) is one of `PreferredDelimiters::PERCENT_LITERAL_TYPES`
//! (`%`, `%i`, `%I`, `%q`, `%Q`, `%r`, `%s`, `%w`, `%W`, `%x`). Of those, only
//! `%i`/`%I`/`%w`/`%W` can ever actually produce an `array` node -- the rest
//! are string/regexp/symbol literals, distinct Prism node kinds this rule
//! never subscribes to -- so checking the opening delimiter's first letter
//! against that four-way set reproduces the type filter exactly.
//!
//! `contains_percent_literals?` then re-derives, for each element, the
//! *value* a Ruby `str`/`sym` AST node's `children.first.to_s` would produce
//! (its literal content) and tests it against `/\A#{type}\W/` for every
//! percent type in turn. A nested percent-opener splits the word/symbol
//! stream on whitespace exactly like top-level percent literals do (`%i[c`
//! and `d]` become two separate elements of the *outer* array, each keeping
//! the nested opener/closer as literal text), so this reproduces as: does
//! any element's own unescaped bytes start with one of the nine percent
//! prefixes immediately followed by a non-word byte? Prism gives every
//! `%i`/`%w` word/symbol a plain `StringNode`/`SymbolNode` whose `unescaped()`
//! is exactly that `children.first` value; an interpolated element
//! (`InterpolatedStringNode`/`InterpolatedSymbolNode`, only possible when a
//! word literally contains `#{`) is skipped, matching upstream: its Ruby
//! `children.first` would be a nested `str`/`begin` AST node rather than a
//! raw string, so `.to_s` there produces a parser s-expression that can
//! never start with `%`.
//!
//! `.scrub` (replacing invalid UTF-8 byte sequences with U+FFFD before the
//! regex match) is reproduced with `String::from_utf8_lossy`, matching this
//! codebase's established substitute for encoding-invalid source bytes (see
//! `Style/WordArray`'s `blind_spots`).

use std::sync::LazyLock;

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use regex::Regex;
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Within percent literals, nested percent literals do not function and may be \
                    unwanted in the result.";

/// Checks for nested percent literals.
#[derive(Debug, Clone)]
pub struct NestedPercentLiteral;

impl Rule for NestedPercentLiteral {
    const META: RuleMeta = RuleMeta {
        name: "Lint/NestedPercentLiteral",
        department: Department::Lint,
        summary: "Checks for nested percent literals.",
        explanation: "\
Checks for nested percent literals.

```ruby
# bad

# The percent literal for nested_attributes is parsed as four tokens,
# yielding the array [:name, :content, :\"%i[incorrectly\", :\"nested]\"].
attributes = {
  valid_attributes: %i[name content],
  nested_attributes: %i[name content %i[incorrectly nested]]
}

# good

# Neither is incompatible with the bad case, but probably the intended code.
attributes = {
  valid_attributes: %i[name content],
  nested_attributes: [:name, :content, %i[incorrectly nested]]
}

attributes = {
  valid_attributes: %i[name content],
  nested_attributes: [:name, :content, [:incorrectly, :nested]]
}
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::ArrayNode],
        config: &[],
        blind_spots: "\
Only plain (non-interpolated) `%i`/`%w` elements are checked, matching upstream's own
`children.first.to_s` shape for a Ruby `str`/`sym` node; see the module doc for why an
interpolated element (one containing `#{`) can never match either implementation.",
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
            && matches!(opening_bytes[1], b'i' | b'I' | b'w' | b'W'))
        {
            return;
        }

        let has_nested = array.elements().iter().any(|element| {
            let unescaped = match &element {
                Node::StringNode { .. } => {
                    element.as_string_node().expect("kind matched").unescaped().to_vec()
                }
                Node::SymbolNode { .. } => {
                    element.as_symbol_node().expect("kind matched").unescaped().to_vec()
                }
                _ => return false,
            };
            let text = String::from_utf8_lossy(&unescaped);
            nested_percent_opener_re().is_match(&text)
        });

        if has_nested {
            ctx.report(&Self::META, node.span(), MSG);
        }
    }
}

/// `PreferredDelimiters::PERCENT_LITERAL_TYPES.map { |t| /\A#{t}\W/ }`,
/// combined into one alternation (any branch matching is equivalent to
/// upstream's `REGEXES.any? { |regex| literal.match?(regex) }`).
fn nested_percent_opener_re() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\A(?:%i|%I|%q|%Q|%r|%s|%w|%W|%x|%)\W").expect("static regex is valid")
    });
    &RE
}
