//! `Style/WhenThen`, ported from RuboCop's
//! `lib/rubocop/cop/style/when_then.rb`.
//!
//! Prism's `WhenNode#then_keyword_loc` is only set when an explicit `then`
//! keyword is present; a `;` delimiter (`when b; c`) leaves it `None`, same
//! as a plain newline delimiter (`when b\n  c`). The `multiline?` guard
//! below is what tells the two apart: a `;`-delimited clause is always
//! single-line, so once we know `then_keyword_loc` is absent *and* the
//! clause is single-line, the only token that could have separated the
//! conditions from the body is a `;` -- there's no room on the line for a
//! comment or anything else between them. So the `;`'s location is found by
//! scanning the (guaranteed short, single-line) gap between the last
//! condition and the body for that byte, rather than reading it off the
//! node directly.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Do not use `when %s;`. Use `when %s then` instead.";

/// Checks for `when;` uses in `case` expressions.
#[derive(Debug, Clone)]
pub struct WhenThen;

impl Rule for WhenThen {
    const META: RuleMeta = RuleMeta {
        name: "Style/WhenThen",
        department: Department::Style,
        summary: "Use when x then ... for one-line cases.",
        explanation: "\
Checks for `when;` uses in `case` expressions.

```ruby
# bad
case foo
when 1; 'baz'
when 2; 'bar'
end

# good
case foo
when 1 then 'baz'
when 2 then 'bar'
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::WhenNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(when) = node.as_when_node() else { return };
        let Some(statements) = when.statements() else { return };
        if when.then_keyword_loc().is_some() || !ctx.is_single_line(when.location().span()) {
            return;
        }

        let conditions: Vec<Node<'_>> = when.conditions().iter().collect();
        let Some(last_condition) = conditions.last() else { return };

        // The `;` sits somewhere between the last condition and the body;
        // on a single line with no `then`, nothing else can occupy that gap.
        let gap = Span::new(last_condition.span().end, statements.location().span().start);
        let gap_text = ctx.text(gap);
        let Some(semicolon_offset) = gap_text.iter().position(|&b| b == b';') else { return };
        let semicolon_start =
            gap.start + u32::try_from(semicolon_offset).expect("offset exceeds u32");
        let semicolon_span = Span::new(semicolon_start, semicolon_start + 1);

        let expression = conditions
            .iter()
            .map(|c| String::from_utf8_lossy(ctx.text(c.span())).into_owned())
            .collect::<Vec<_>>()
            .join(", ");
        let message = MSG.replacen("%s", &expression, 1).replacen("%s", &expression, 1);

        ctx.report_with_fix(
            &Self::META,
            semicolon_span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(semicolon_span, b" then".to_vec())],
            },
        );
    }
}
