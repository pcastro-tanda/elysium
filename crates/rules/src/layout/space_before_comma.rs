//! `Layout/SpaceBeforeComma`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_before_comma.rb` plus the
//! `SpaceBeforePunctuation` mixin (`lib/rubocop/cop/mixin/
//! space_before_punctuation.rb`) it includes. See `space_punctuation`'s
//! module docs for how the mixin is approximated without a token stream.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_source::Span;

use crate::layout::space_punctuation::{each_punctuation, peer_brace_style_is, prev_on_line};

/// RuboCop's `SpaceBeforePunctuation::MSG`, formatted with `kind` = `"comma"`.
const MSG: &str = "Space found before comma.";

/// No spaces before commas.
#[derive(Debug, Clone, Default)]
pub struct SpaceBeforeComma {
    /// `Layout/SpaceInsideBlockBraces`'s `EnforcedStyle == 'space'` --
    /// RuboCop's `space_required_after_lcurly?`.
    require_space_after_lcurly: bool,
}

impl Rule for SpaceBeforeComma {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceBeforeComma",
        department: Department::Layout,
        summary: "No spaces before commas.",
        explanation: "\
```ruby
# bad
[1 , 2 , 3]
a(1 , 2)
each { |a , b| }

# good
[1, 2, 3]
a(1, 2)
each { |a, b| }
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "\
Reproduces RuboCop's token-stream-based mixin by scanning raw bytes for `,`
outside every opaque span (see `space_punctuation`'s module docs); this
reaches every case the cop's own logic reaches.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            require_space_after_lcurly: peer_brace_style_is(
                options,
                "Layout/SpaceInsideBlockBraces",
                "space",
            ),
        })
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let require_space_after_lcurly = self.require_space_after_lcurly;
        let mut offenses = Vec::new();
        each_punctuation(ctx, b',', |pos| {
            let Some((prev_end, prev_byte)) = prev_on_line(ctx, pos) else { return };
            if prev_end == pos {
                return;
            }
            if prev_byte == b'{' && require_space_after_lcurly {
                return;
            }
            offenses.push(Span::new(prev_end, pos));
        });
        for span in offenses {
            ctx.report_with_fix(
                &Self::META,
                span,
                MSG,
                Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(span)] },
            );
        }
    }
}
