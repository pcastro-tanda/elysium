//! `Layout/MultilineHashKeyLineBreaks`, ported from RuboCop's
//! `lib/rubocop/cop/layout/multiline_hash_key_line_breaks.rb` plus the
//! `MultilineElementLineBreaks` mixin it includes.
//!
//! Prism only produces a `HashNode` for a braced hash literal (`node.loc.begin`
//! is always present); a braceless keyword-argument hash is a distinct
//! `KeywordHashNode`, which `Layout/MultilineMethodArgumentLineBreaks`
//! handles instead, so `on_hash`'s `return unless node.loc.begin` guard
//! (and its `starts_with_curly_brace?` twin) is dead code here -- every
//! `HashNode` reaching [`MultilineHashKeyLineBreaks::enter`] already
//! qualifies.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Each key in a multi-line hash must start on a separate line.";

/// Checks that each item in a multi-line hash literal starts on a separate line.
#[derive(Debug, Clone)]
pub struct MultilineHashKeyLineBreaks {
    allow_multiline_final_element: bool,
}

impl Rule for MultilineHashKeyLineBreaks {
    const META: RuleMeta = RuleMeta {
        name: "Layout/MultilineHashKeyLineBreaks",
        department: Department::Layout,
        summary: "Checks that each item in a multi-line hash literal starts on a separate line.",
        explanation: "\
```ruby
# bad
{
  a: 1, b: 2,
  c: 3
}

# good
{
  a: 1,
  b: 2,
  c: 3
}
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::HashNode],
        config: &[ConfigOption {
            name: "AllowMultilineFinalElement",
            default: ConfigDefault::Bool(false),
            allowed: &[],
            doc: "Whether the last key in the hash is allowed to start a new, multi-line \
                  element without triggering this cop.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allow_multiline_final_element: options.bool("AllowMultilineFinalElement") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(hash) = node.as_hash_node() else { return };
        let children: Vec<Span> = hash.elements().iter().map(|n| n.span()).collect();
        check_line_breaks(ctx, &children, self.allow_multiline_final_element);
    }
}

/// RuboCop's `MultilineElementLineBreaks#check_line_breaks`.
fn check_line_breaks(ctx: &mut Context<'_>, children: &[Span], ignore_last: bool) {
    if all_on_same_line(ctx, children, ignore_last) {
        return;
    }

    let mut last_seen_line: i64 = -1;
    for &child in children {
        let first_line = i64::from(ctx.line_col(child.start).line);
        if last_seen_line >= first_line {
            let fix = Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::insert(child.start, b"\n".to_vec())],
            };
            ctx.report_with_fix(&MultilineHashKeyLineBreaks::META, child, MSG, fix);
        } else {
            last_seen_line = i64::from(ctx.last_line(child));
        }
    }
}

/// RuboCop's `MultilineElementLineBreaks#all_on_same_line?`.
fn all_on_same_line(ctx: &Context<'_>, children: &[Span], ignore_last: bool) -> bool {
    let (Some(&first), Some(&last)) = (children.first(), children.last()) else { return true };
    if ignore_last {
        // RuboCop's `same_line?(nodes.first, nodes.last)`: both nodes' own first lines match.
        ctx.line_col(first.start).line == ctx.line_col(last.start).line
    } else {
        ctx.line_col(first.start).line == ctx.last_line(last)
    }
}
