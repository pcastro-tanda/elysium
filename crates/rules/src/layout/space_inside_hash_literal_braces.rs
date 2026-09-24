//! `Layout/SpaceInsideHashLiteralBraces`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_inside_hash_literal_braces.rb` plus the
//! `SurroundingSpace` mixin it includes.
//!
//! RuboCop's `check`/`check_whitespace_only_hash` work off `processed_source`'s
//! lexer token stream (`token1.space_after?`, `token1.line < token2.line`,
//! `token2.comment?`). Prism hands us spans, not tokens, so this rule instead
//! scans the raw bytes strictly between a hash literal's own `opening_loc`
//! and `closing_loc` -- a region the single traversal already bounds exactly,
//! with no need to re-walk anything:
//!
//! - Every byte is ASCII whitespace (`u8::is_ascii_whitespace`, matching
//!   `SINGLE_SPACE_REGEXP`/`\s` closely enough for real Ruby source) up to
//!   the closing brace: this is RuboCop's `is_empty_braces` case, checked
//!   once (`tokens.size == 2`). A `\n` anywhere in that run puts the closing
//!   brace on a different source line, which is exactly the condition under
//!   which RuboCop's own `check` bails out via `token1.line < token2.line`
//!   before ever computing `is_empty_braces` -- so this rule skips the
//!   ordinary check too, leaving only the `EnforcedStyleForEmptyBraces`
//!   sweep below (RuboCop's `check_whitespace_only_hash`) to fire.
//! - Otherwise the first/last non-whitespace byte marks where real content
//!   starts/ends. A `#` there is a same-line comment (unambiguous: nothing
//!   but whitespace can precede a comment inside a hash literal in valid
//!   Ruby), matching `token2.comment?`'s skip. A `\n` in the whitespace run
//!   leading up to it means the content is on another line, matching
//!   `token1.line < token2.line`'s skip for that side specifically (RuboCop
//!   checks each side independently, so a hash can skip one side and still
//!   be checked on the other). A `{`/`}` sitting immediately at that
//!   boundary is RuboCop's `is_same_braces` (adjacent hash/block braces,
//!   e.g. `{{ a: 1 } => value}`), which only matters for `compact` style.
//!
//! The `ConfigurableEnforcedStyle` mixin's `ambiguous_style_detected`/
//! `unexpected_style_detected` bookkeeping (feeds `--auto-gen-config`) has
//! no effect on offenses or messages and is not ported.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`, with `%<problem>s` already split into its two halves.
fn message(inside_what: &str, problem: &str) -> String {
    format!("Space inside {inside_what} {problem}.")
}

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Space,
    NoSpace,
    Compact,
}

/// Checks that braces used for hash literals (and hash patterns) have or
/// don't have surrounding space depending on configuration.
#[derive(Debug, Clone)]
pub struct SpaceInsideHashLiteralBraces {
    style: Style,
    /// `EnforcedStyleForEmptyBraces == 'no_space'` (the default).
    empty_no_space: bool,
}

impl SpaceInsideHashLiteralBraces {
    /// RuboCop's `expect_space?`.
    fn expect_space(&self, is_same_braces: bool, is_empty_braces: bool) -> bool {
        if is_same_braces && self.style == Style::Compact {
            false
        } else if is_empty_braces {
            !self.empty_no_space
        } else {
            self.style != Style::NoSpace
        }
    }

    /// RuboCop's `incorrect_style_detected` plus `autocorrect`: `brace` is
    /// this hash's own opening (`is_left`) or closing brace span; `space` is
    /// the whitespace run actually found on that side (used verbatim as the
    /// removal range, or as the empty-braces span when `is_empty_braces`).
    #[allow(clippy::too_many_arguments)]
    fn report(
        ctx: &mut Context<'_>,
        brace: Span,
        space: Span,
        is_left: bool,
        is_empty_braces: bool,
        expect_space: bool,
    ) {
        let inside_what = if is_empty_braces {
            "empty hash literal braces"
        } else if is_left {
            "{"
        } else {
            "}"
        };
        let problem = if expect_space { "missing" } else { "detected" };
        let range = if expect_space { brace } else { space };
        let edit = if expect_space {
            let at = if is_left { brace.end } else { brace.start };
            Edit::insert(at, b" ".to_vec())
        } else {
            Edit::delete(space)
        };
        ctx.report_with_fix(
            &Self::META,
            range,
            message(inside_what, problem),
            Fix { applicability: Applicability::Safe, edits: vec![edit] },
        );
    }

    /// RuboCop's `on_hash`/`on_hash_pattern`: `opening`/`closing` are the
    /// hash's own brace spans (already confirmed to be literal `{`/`}`).
    fn check(&self, ctx: &mut Context<'_>, opening: Span, closing: Span) {
        let content = ctx.text(Span::new(opening.end, closing.start));

        match content.iter().position(|b| !b.is_ascii_whitespace()) {
            None => {
                // `is_empty_braces`, checked once. Skip entirely if a `\n`
                // separates the braces onto different lines (RuboCop's
                // `token1.line < token2.line` bail-out, checked before
                // `is_empty_braces` is even computed).
                if !content.contains(&b'\n') {
                    let expect_space = self.expect_space(false, true);
                    let has_space = !content.is_empty();
                    if has_space != expect_space {
                        Self::report(
                            ctx,
                            opening,
                            Span::new(opening.end, closing.start),
                            true,
                            true,
                            expect_space,
                        );
                    }
                }
            }
            Some(first) => {
                if content[first] == b'#' {
                    // Comment directly inside the braces: RuboCop's
                    // `token2.comment?` skip; never reached for the trailing
                    // side (see module docs).
                } else if !content[..first].contains(&b'\n') {
                    let same_braces = content[first] == b'{';
                    let expect_space = self.expect_space(same_braces, false);
                    let has_space = first > 0;
                    if has_space != expect_space {
                        let space =
                            Span::new(opening.end, opening.end + u32::try_from(first).unwrap_or(0));
                        Self::report(ctx, opening, space, true, false, expect_space);
                    }
                }

                let last =
                    content.iter().rposition(|b| !b.is_ascii_whitespace()).expect("first is Some");
                let trailing = &content[last + 1..];
                if !trailing.contains(&b'\n') {
                    let same_braces = content[last] == b'}';
                    let expect_space = self.expect_space(same_braces, false);
                    let has_space = !trailing.is_empty();
                    if has_space != expect_space {
                        let space = Span::new(
                            closing.start - u32::try_from(trailing.len()).unwrap_or(0),
                            closing.start,
                        );
                        Self::report(ctx, closing, space, false, false, expect_space);
                    }
                }
            }
        }

        // RuboCop's `check_whitespace_only_hash`, run unconditionally
        // whenever the empty style wants no space; any offense it finds at
        // the same span/message as the block above is an exact duplicate
        // the engine already dedups.
        if self.empty_no_space && !content.is_empty() && content.iter().all(u8::is_ascii_whitespace)
        {
            let region = Span::new(opening.end, closing.start);
            ctx.report_with_fix(
                &Self::META,
                region,
                message("empty hash literal braces", "detected"),
                Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(region)] },
            );
        }
    }
}

/// True when `bytes` is exactly the single expected brace character.
fn is_lone_byte(bytes: &[u8], expected: u8) -> bool {
    matches!(bytes, [b] if *b == expected)
}

impl Rule for SpaceInsideHashLiteralBraces {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceInsideHashLiteralBraces",
        department: Department::Layout,
        summary: "Checks that braces used for hash literals have or don't have surrounding space \
                  depending on configuration. Hash pattern matching is handled in the same way.",
        explanation: "\
```ruby
# EnforcedStyle: space (default)
# The `space` style enforces that hash literals have surrounding space.

# bad
h = {a: 1, b: 2}
foo = {{ a: 1 } => { b: { c: 2 }}}

# good
h = { a: 1, b: 2 }
foo = { { a: 1 } => { b: { c: 2 } } }
```

```ruby
# EnforcedStyle: no_space
# The `no_space` style enforces that hash literals have no surrounding space.

# bad
h = { a: 1, b: 2 }
foo = {{ a: 1 } => { b: { c: 2 }}}

# good
h = {a: 1, b: 2}
foo = {{a: 1} => {b: {c: 2}}}
```

```ruby
# EnforcedStyle: compact
# The `compact` style normally requires a space inside hash braces, with the
# exception that successive left braces or right braces are collapsed
# together in nested hashes.

# bad
h = { a: { b: 2 } }
foo = { { a: 1 } => { b: { c: 2 } } }

# good
h = { a: { b: 2 }}
foo = {{ a: 1 } => { b: { c: 2 }}}
```

```ruby
# EnforcedStyleForEmptyBraces: no_space (default)
# The `no_space` EnforcedStyleForEmptyBraces style enforces that empty hash
# braces do not contain spaces.

# bad
foo = { }
bar = {    }
baz = {
}

# good
foo = {}
bar = {}
baz = {}
```

```ruby
# EnforcedStyleForEmptyBraces: space
# The `space` EnforcedStyleForEmptyBraces style enforces that empty hash
# braces contain space.

# bad
foo = {}

# good
foo = { }
foo = {    }
foo = {
}
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::HashNode, NodeKind::HashPatternNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("space"),
                allowed: &["space", "no_space", "compact"],
                doc: "Whether hash literal braces require, forbid, or (for `compact`) selectively \
                      collapse surrounding space.",
            },
            ConfigOption {
                name: "EnforcedStyleForEmptyBraces",
                default: ConfigDefault::Str("no_space"),
                allowed: &["space", "no_space"],
                doc: "Whether empty hash literal braces (`{}`) require or forbid a space between \
                      them.",
            },
        ],
        blind_spots: "\
Scans raw bytes between a hash literal's own opening/closing brace spans in
place of RuboCop's lexer token stream (see the module docs for the mapping);
this reaches every case the cop's `check`/`check_whitespace_only_hash`
reach, but two extremely unlikely inputs diverge from RuboCop's exact `\\s`:
`u8::is_ascii_whitespace` does not treat a lone vertical tab (`\\v`) as
whitespace, and a hash whose only interior content is itself only reachable
through a byte RuboCop's lexer would have tokenized differently is not
specially handled. `ambiguous_style_detected`/`unexpected_style_detected`
(feeds `--auto-gen-config`) is not ported; it never changes an offense or
its message.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "no_space" => Style::NoSpace,
            "compact" => Style::Compact,
            _ => Style::Space,
        };
        let empty_no_space = options.style("EnforcedStyleForEmptyBraces")? != "space";
        Ok(Self { style, empty_no_space })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let (opening, closing) = match node {
            Node::HashNode { .. } => {
                let n = node.as_hash_node().expect("kind matched");
                (n.opening_loc().span(), n.closing_loc().span())
            }
            Node::HashPatternNode { .. } => {
                let n = node.as_hash_pattern_node().expect("kind matched");
                let (Some(open), Some(close)) = (n.opening_loc(), n.closing_loc()) else { return };
                (open.span(), close.span())
            }
            _ => return,
        };
        if !is_lone_byte(ctx.text(opening), b'{') || !is_lone_byte(ctx.text(closing), b'}') {
            // Braceless hash literal (`KeywordHashNode`, handled by not
            // subscribing to that kind at all) or a `Const[...]`
            // bracket-style hash pattern sharing `HashPatternNode`'s
            // `opening_loc`/`closing_loc` fields with `[`/`]` instead of
            // `{`/`}`: RuboCop's own `tokens.first.left_brace? &&
            // tokens.last.right_curly_brace?` guard skips both the same way.
            return;
        }
        self.check(ctx, opening, closing);
    }
}
