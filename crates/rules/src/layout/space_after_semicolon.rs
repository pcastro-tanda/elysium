//! `Layout/SpaceAfterSemicolon`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_after_semicolon.rb` plus the
//! `SpaceAfterPunctuation` mixin (`lib/rubocop/cop/mixin/
//! space_after_punctuation.rb`) it includes. See `space_punctuation`'s
//! module docs for how both are approximated without a token stream.
//!
//! This cop's own `space_missing?` override additionally exempts a
//! semicolon directly followed by another semicolon (`;;`) from ever being
//! reported -- upstream's `semicolon_sequence?` -- on top of the mixin's
//! own `allowed_type?`/rcurly-style exclusions.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeKind};
use ruby_source::Span;

use crate::layout::space_punctuation::{
    adjacent_after, each_punctuation, peer_brace_style_is, InterpolationClosers,
};

/// RuboCop's `SpaceAfterPunctuation::MSG`, formatted with `kind` = `"semicolon"`.
const MSG: &str = "Space missing after semicolon.";

/// Use spaces after semicolons.
#[derive(Debug, Clone, Default)]
pub struct SpaceAfterSemicolon {
    /// `Layout/SpaceInsideBlockBraces`'s `EnforcedStyle == 'no_space'` --
    /// RuboCop's `space_style_before_rcurly` override.
    forbid_rcurly_space: bool,
    closers: InterpolationClosers,
}

impl Rule for SpaceAfterSemicolon {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceAfterSemicolon",
        department: Department::Layout,
        summary: "Use spaces after semicolons.",
        explanation: "\
```ruby
# bad
x = 1;y = 2

# good
x = 1; y = 2
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::EmbeddedStatementsNode],
        config: &[],
        blind_spots: "\
Reproduces RuboCop's token-stream-based mixin by scanning raw bytes for `;`
outside every opaque span (see `space_punctuation`'s module docs); this
reaches every case the cop's own logic reaches except a semicolon directly
followed by the very end of a multi-byte percent-literal delimiter or other
lexer-only token shape that has no raw-byte tell of its own, none of which
occur adjacent to a semicolon in valid Ruby.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            forbid_rcurly_space: peer_brace_style_is(
                options,
                "Layout/SpaceInsideBlockBraces",
                "no_space",
            ),
            closers: InterpolationClosers::default(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.closers.clear();
    }

    fn enter(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        self.closers.record(node);
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let forbid_rcurly_space = self.forbid_rcurly_space;
        let closers = &self.closers;
        let mut offenses = Vec::new();
        each_punctuation(ctx, b';', |pos| {
            let Some((next_pos, next_byte)) = adjacent_after(ctx, pos) else { return };
            if next_byte == b';' {
                return;
            }
            let allowed = match next_byte {
                b')' | b']' | b'|' => true,
                b'}' if closers.contains(next_pos) => true,
                b'}' => forbid_rcurly_space,
                _ => false,
            };
            if allowed {
                return;
            }
            offenses.push(pos);
        });
        for pos in offenses {
            let span = Span::new(pos, pos + 1);
            ctx.report_with_fix(
                &Self::META,
                span,
                MSG,
                Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::insert(pos + 1, b" ".as_slice())],
                },
            );
        }
    }
}
