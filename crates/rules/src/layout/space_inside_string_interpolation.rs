//! `Layout/SpaceInsideStringInterpolation`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_inside_string_interpolation.rb` plus the
//! `Interpolation` and `SurroundingSpace` mixins it includes.
//!
//! RuboCop's `Interpolation` mixin fires `on_interpolation` for every `begin`
//! node inside a `dstr`/`xstr`/`dsym`/`regexp` -- i.e. every `#{...}` part.
//! Prism represents every `#{...}` the same way, as a single
//! [`NodeKind::EmbeddedStatementsNode`], with no distinct node per containing
//! literal kind, so subscribing to that one kind covers all four cases
//! directly (see `Lint/RedundantStringCoercion`'s identical mapping).
//!
//! # Multiline guard
//!
//! `begin_node.multiline?` (`rubocop-ast`'s generic `Node#multiline?`,
//! `line_count > 1`) is `Context::is_single_line` on the whole node's own
//! span (`#{` through `}` inclusive): identical to comparing first/last
//! line, and it subsumes the mixin's separate `token.comment?` skip too --
//! a `#` before the closing brace always pushes it onto a later physical
//! line, which this same check already bails out on.
//!
//! # Empty/whitespace-only interpolation
//!
//! `empty_brackets?` compares lexer *token* indices, and RuboCop's lexer
//! does not tokenize whitespace of any kind -- so `"#{ }"` or `"#{ \t }"`
//! is just as "empty" as `"#{}"` and produces no offense in either style.
//! This is reproduced by treating the raw bytes strictly between the
//! opening `#{` and closing `}` as empty whenever every byte in it is ASCII
//! whitespace (which, given the multiline guard already ran, can only be
//! runs of space/tab or other rare non-newline whitespace).
//!
//! # Offense boundaries
//!
//! Once content is confirmed non-empty and single-line, `extra_space?`
//! only tests the single byte immediately after `#{`/before `}` against
//! `SINGLE_SPACE_REGEXP` (`/[ \t]/` -- space or tab, not general `\s`), and
//! `reposition` then walks that same `[ \t]` class outward to build the
//! actual offense/removal range. This is exactly a leading/trailing
//! space-or-tab run over `content`, computed once per side.
//!
//! # Style branches
//!
//! - `no_space` (default): a leading/trailing space-or-tab run is itself
//!   the offense range, autocorrected by deleting it (`SpaceCorrector
//!   .remove_space`).
//! - `space`: a *missing* run on a side is the offense, with the offense
//!   range being the delimiter token itself (`side: :none` in
//!   `space_offense`, which leaves `side_space_range`'s range untouched),
//!   autocorrected by inserting a single space just inside that delimiter
//!   (`SpaceCorrector.add_space`). A side that already has *any* run is
//!   left alone (excess spacing is `Layout/ExtraSpace`'s job, matching the
//!   spec's own comment to that effect).
//!
//! Each side is independent (RuboCop's `start_ok`/`end_ok` linkage is never
//! used by this cop, and `autocorrect_with_disable_uncorrectable?` is a
//! disable-directive feature this port does not have), so this reports each
//! side as its own offense with its own single-edit fix, exactly mirroring
//! `Layout/SpaceInsideHashLiteralBraces`'s per-side reporting.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Space,
    NoSpace,
}

/// Checks for whitespace within string interpolations.
#[derive(Debug, Clone)]
pub struct SpaceInsideStringInterpolation {
    style: Style,
}

impl SpaceInsideStringInterpolation {
    /// RuboCop's `MSG`, `'%<command>s space inside string interpolation.'`.
    fn message(command: &str) -> String {
        format!("{command} space inside string interpolation.")
    }

    /// RuboCop's `no_space_offenses`/`space_offenses`, restricted to one
    /// side: `delimiter` is this side's own `#{`/`}` span; `run` is the
    /// leading (left side) or trailing (right side) space-or-tab run
    /// abutting it, if any.
    fn check_side(&self, ctx: &mut Context<'_>, delimiter: Span, run: Option<Span>, is_left: bool) {
        match self.style {
            Style::NoSpace => {
                let Some(run) = run else { return };
                ctx.report_with_fix(
                    &Self::META,
                    run,
                    Self::message("Do not use"),
                    Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(run)] },
                );
            }
            Style::Space => {
                if run.is_some() {
                    return;
                }
                let at = if is_left { delimiter.end } else { delimiter.start };
                ctx.report_with_fix(
                    &Self::META,
                    delimiter,
                    Self::message("Use"),
                    Fix {
                        applicability: Applicability::Safe,
                        edits: vec![Edit::insert(at, b" ".to_vec())],
                    },
                );
            }
        }
    }
}

impl Rule for SpaceInsideStringInterpolation {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceInsideStringInterpolation",
        department: Department::Layout,
        summary: "Checks for whitespace within string interpolations.",
        explanation: "\
```ruby
# EnforcedStyle: no_space (default)

# bad
var = \"This is the #{ space } example\"

# good
var = \"This is the #{no_space} example\"
```

```ruby
# EnforcedStyle: space

# bad
var = \"This is the #{no_space} example\"

# good
var = \"This is the #{ space } example\"
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::EmbeddedStatementsNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("no_space"),
            allowed: &["space", "no_space"],
            doc: "Whether string interpolation requires or forbids surrounding space.",
        }],
        blind_spots: "\
Reproduces RuboCop's lexer-token-based `empty_brackets?`/`extra_space?` by
scanning raw bytes between the `#{`/`}` delimiter spans rather than walking a
token stream (see the module docs for the exact mapping); this reaches every
case the cop's own logic reaches, but treats `[ \\t]` runs the same way
upstream's `SINGLE_SPACE_REGEXP` does, so a lone vertical tab or form feed
directly abutting a delimiter is not recognized as space (an extremely
unlikely real-world input, matching an existing documented deviation in
`Layout/SpaceInsideHashLiteralBraces`).",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style =
            if options.style("EnforcedStyle")? == "space" { Style::Space } else { Style::NoSpace };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let n = node.as_embedded_statements_node().expect("kind matched");
        let opening = n.opening_loc().span();
        let closing = n.closing_loc().span();

        if !ctx.is_single_line(Span::new(opening.start, closing.end)) {
            return;
        }

        let content = ctx.text(Span::new(opening.end, closing.start));
        if content.iter().all(u8::is_ascii_whitespace) {
            // `empty_brackets?`: no lexer tokens between the delimiters,
            // which is true of whitespace-only content too.
            return;
        }

        let is_run_byte = |b: u8| b == b' ' || b == b'\t';
        let left_len = content.iter().take_while(|&&b| is_run_byte(b)).count();
        let left_run = (left_len > 0)
            .then(|| Span::new(opening.end, opening.end + u32::try_from(left_len).unwrap_or(0)));

        let right_len = content.iter().rev().take_while(|&&b| is_run_byte(b)).count();
        let right_run = (right_len > 0).then(|| {
            Span::new(closing.start - u32::try_from(right_len).unwrap_or(0), closing.start)
        });

        self.check_side(ctx, opening, left_run, true);
        self.check_side(ctx, closing, right_run, false);
    }
}
