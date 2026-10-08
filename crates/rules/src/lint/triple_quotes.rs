//! `Lint/TripleQuotes`, ported from RuboCop's
//! `lib/rubocop/cop/lint/triple_quotes.rb`.
//!
//! whitequark's `dstr` is Prism's [`ruby_ast::node::InterpolatedStringNode`];
//! its `parts` field holds both plain [`NodeKind::StringNode`] pieces and
//! any interpolation (`EmbeddedStatementsNode`/`EmbeddedVariableNode`)
//! pieces, matching upstream's `node.each_child_node(:str)` filter. Prism
//! represents both a real triple-quoted string (`"""..."""`) and bare
//! implicit concatenation (`'a''b''c'`) as this same node shape, with no
//! node-level field distinguishing them (`opening_loc` is always absent on
//! the dstr itself; only its individual `str` parts carry their own
//! one-character `opening_loc`). Upstream's own test is therefore a raw
//! source scan: `node.source.scan(/(?<=\A)['"]*/)[0]`, counting the run of
//! quote characters (of either kind) starting at the node's own first byte.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str =
    "Delimiting a string with multiple quotes has no effect, use a single quote instead.";

/// Checks for useless triple quote constructs.
#[derive(Debug, Clone)]
pub struct TripleQuotes;

impl Rule for TripleQuotes {
    const META: RuleMeta = RuleMeta {
        name: "Lint/TripleQuotes",
        department: Department::Lint,
        summary: "Checks for useless triple quote constructs.",
        explanation: "\
Checks for \"triple quotes\" (strings delimited by any odd number of quotes
greater than 1).

Ruby allows multiple strings to be implicitly concatenated by just being
adjacent in a statement (ie. `\"foo\"\"bar\" == \"foobar\"`). This sometimes gives
the impression that there is something special about triple quotes, but in
fact it is just extra unnecessary quotes and produces the same string. Each
pair of quotes produces an additional concatenated empty string, so the
result is still only the \"actual\" string within the delimiters.

NOTE: Although this cop is called triple quotes, the same behavior is
present for strings delimited by 5, 7, etc. quotation marks.

```ruby
# bad
\"\"\"
  A string
\"\"\"

# bad
'''
  A string
'''

# good
\"
  A string
\"

# good
<<STRING
  A string
STRING

# good (but not the same spacing as the bad case)
'A string'
```",
        enabled_by_default: false,
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
        let dstr = node.as_interpolated_string_node().expect("kind matched");
        let parts = dstr.parts();

        let mut empty_spans: Vec<Span> = Vec::new();
        for part in &parts {
            if let Some(str_node) = part.as_string_node() {
                if str_node.unescaped().is_empty() {
                    empty_spans.push(part.span());
                }
            }
        }
        if empty_spans.is_empty() {
            return;
        }

        let span = node.span();
        let text = ctx.text(span);
        let opening_quotes = text.iter().take_while(|&&b| b == b'\'' || b == b'"').count();
        if opening_quotes < 3 {
            return;
        }

        // If the node is composed of only empty `str` nodes, keep one.
        let to_remove: &[Span] =
            if empty_spans.len() == parts.len() { &empty_spans[1..] } else { &empty_spans };

        let edits = to_remove.iter().map(|&s| Edit::delete(s)).collect();
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}
