//! `Layout/SpaceBeforeComment`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_before_comment.rb`.
//!
//! Upstream walks `processed_source.sorted_tokens.each_cons(2)` and flags a
//! comment token whose immediately preceding token ends exactly where the
//! comment begins (`token1.pos.end == token2.pos.begin`), on the same line.
//! Since a token boundary can only abut a comment with zero gap when the
//! byte immediately before the comment is neither a line terminator nor
//! horizontal whitespace (any such byte would itself have to be part of a
//! token, which token-adjacency to the comment forbids), this port skips
//! reconstructing the token stream and reads that one byte directly.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Put a space before an end-of-line comment.";

/// Checks for missing space between a token and a comment on the same line.
#[derive(Debug, Clone, Default)]
pub struct SpaceBeforeComment;

impl Rule for SpaceBeforeComment {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceBeforeComment",
        department: Department::Layout,
        summary: "Checks for missing space between a token and a comment on the same line.",
        explanation: "\
```ruby
# bad
1 + 1# this operation does ...

# good
1 + 1 # this operation does ...
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let source = ctx.source().bytes();
        let offenses: Vec<Span> = ctx
            .comments()
            .iter()
            .filter_map(|comment| {
                let start = comment.span.start;
                if start == 0 {
                    return None;
                }
                let prev = source[(start - 1) as usize];
                if matches!(prev, b'\n' | b'\r' | b' ' | b'\t') {
                    return None;
                }
                Some(comment.span)
            })
            .collect();
        for span in offenses {
            ctx.report_with_fix(
                &Self::META,
                span,
                MSG,
                Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::insert(span.start, b" ".to_vec())],
                },
            );
        }
    }
}
