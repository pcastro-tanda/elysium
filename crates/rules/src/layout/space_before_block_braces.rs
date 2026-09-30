//! `Layout/SpaceBeforeBlockBraces`, ported from RuboCop's
//! `lib/rubocop/cop/layout/space_before_block_braces.rb` plus the
//! `RangeHelp` mixin (`lib/rubocop/cop/mixin/range_help.rb`) it includes.
//!
//! Prism gives every brace-delimited block (`{ }` with explicit `|params|`,
//! numbered `_1`/implicit `it` parameters, or none at all) the same
//! [`NodeKind::BlockNode`], and a stabby lambda (`->(x) { x }`) its own
//! [`NodeKind::LambdaNode`] -- RuboCop's `on_block`/`on_numblock`/`on_itblock`
//! all collapse onto that one node kind here, and lambdas (parsed as a
//! `:block` node wrapping a `(send nil :lambda)` in whitequark) fall out of
//! `on_block` there too; this port subscribes to both kinds and handles each
//! the same way, matching `Layout/SpaceInsideBlockBraces`'s own precedent.
//! `do`/`end` blocks are skipped exactly like RuboCop's `node.keywords?`
//! guard, by checking the opening delimiter's source text is `{`.
//!
//! RuboCop's `range_with_surrounding_space(left_brace)` (defaults:
//! `side: :both, newlines: true, whitespace: false, continuations: false`)
//! is [`Context::with_surrounding_space`] called with the matching
//! arguments; the cop only ever inspects whether that range's *start*
//! moved left of `left_brace`'s own start, which stands in for RuboCop's
//! `space_plus_brace.source.start_with?('{')` check.
//!
//! `conflict_with_block_delimiters?`'s `config.for_cop('Style/BlockDelimiters')
//! ['EnforcedStyle']` peer lookup goes through [`RuleOptions::peer`], which
//! (like every other peer-config read in this port) sees the fully merged
//! configuration -- including `Style/BlockDelimiters`'s own `config/
//! default.yml` default of `line_count_based` -- since both the fixture
//! harness and the real CLI build `RuleOptions` from a `LoadedConfig` that
//! already merged the embedded defaults in.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::{Side, Span};

const MISSING_MSG: &str = "Space missing to the left of {.";
const DETECTED_MSG: &str = "Space detected to the left of {.";

/// RuboCop's `EnforcedStyle`/`EnforcedStyleForEmptyBraces` (both share the
/// same two values).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Space,
    NoSpace,
}

/// Checks that the left block brace has or doesn't have space before it.
#[derive(Debug, Clone)]
pub struct SpaceBeforeBlockBraces {
    style: Style,
    empty_braces_style: Style,
    /// RuboCop's `conflict_with_block_delimiters?`, precomputed at
    /// configure time: `block_delimiters_style == 'line_count_based' &&
    /// style == :no_space`. Combined per-node with `node.multiline?`.
    conflicts_with_block_delimiters: bool,
}

impl Rule for SpaceBeforeBlockBraces {
    const META: RuleMeta = RuleMeta {
        name: "Layout/SpaceBeforeBlockBraces",
        department: Department::Layout,
        summary: "Checks that the left block brace has or doesn't have space before it.",
        explanation: "\
Checks that block braces have or don't have a space before the opening
brace depending on configuration.

```ruby
# bad (EnforcedStyle: space, the default)
foo.map{ |a|
  a.bar.to_s
}

# good (EnforcedStyle: space, the default)
foo.map { |a|
  a.bar.to_s
}

# bad (EnforcedStyle: no_space)
foo.map { |a|
  a.bar.to_s
}

# good (EnforcedStyle: no_space)
foo.map{ |a|
  a.bar.to_s
}

# bad (EnforcedStyleForEmptyBraces: space, the default)
7.times{}

# good (EnforcedStyleForEmptyBraces: space, the default)
7.times {}

# bad (EnforcedStyleForEmptyBraces: no_space)
7.times {}

# good (EnforcedStyleForEmptyBraces: no_space)
7.times{}
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::BlockNode, NodeKind::LambdaNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("space"),
                allowed: &["space", "no_space"],
                doc: "Whether the left block brace has space before it.",
            },
            ConfigOption {
                name: "EnforcedStyleForEmptyBraces",
                default: ConfigDefault::Str("space"),
                allowed: &["space", "no_space"],
                doc: "Whether empty braces have space before them.",
            },
        ],
        blind_spots: "\
`self.autocorrect_incompatible_with` (`Style::SymbolProc`) is not
replicated; it only matters when both cops run in the same fix pass.
`config_to_allow_offenses`/`handle_different_styles_for_empty_braces`
auto-config generation (the `--auto-gen-config` result when mixed styles
are used) is not ported: this engine has no config-generation pass.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "no_space" => Style::NoSpace,
            _ => Style::Space,
        };
        // RuboCop's `style_for_empty_braces`: an explicit `nil` config value
        // falls back to the cop's own overall `style`, not a fixed default.
        let empty_braces_style = match options.get("EnforcedStyleForEmptyBraces") {
            Some(OptionValue::Null) => style,
            _ => match options.style("EnforcedStyleForEmptyBraces")? {
                "no_space" => Style::NoSpace,
                _ => Style::Space,
            },
        };
        let block_delimiters_style = options
            .peer("Style/BlockDelimiters", "EnforcedStyle")
            .and_then(OptionValue::as_str)
            .unwrap_or("line_count_based");
        Ok(Self {
            style,
            empty_braces_style,
            conflicts_with_block_delimiters: style == Style::NoSpace
                && block_delimiters_style == "line_count_based",
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::BlockNode { .. } => {
                let block = node.as_block_node().expect("kind matched");
                self.check(ctx, block.opening_loc().span(), block.closing_loc().span());
            }
            Node::LambdaNode { .. } => {
                let lambda = node.as_lambda_node().expect("kind matched");
                self.check(ctx, lambda.opening_loc().span(), lambda.closing_loc().span());
            }
            _ => {}
        }
    }
}

impl SpaceBeforeBlockBraces {
    /// RuboCop's `on_block`, after the `node.keywords?` guard.
    fn check(&self, ctx: &mut Context<'_>, left_brace: Span, right_brace: Span) {
        if ctx.text(left_brace) != b"{" {
            return;
        }
        // RuboCop's `conflict_with_block_delimiters?`: `node.multiline?`
        // compares only the delimiters' own lines.
        if self.conflicts_with_block_delimiters && !ctx.same_line(left_brace, right_brace) {
            return;
        }

        let space_plus_brace = ctx.with_surrounding_space(left_brace, Side::Both, true, false);
        let used_style =
            if space_plus_brace.start == left_brace.start { Style::NoSpace } else { Style::Space };

        if left_brace.end == right_brace.start {
            self.check_empty(ctx, left_brace, space_plus_brace, used_style);
        } else {
            self.check_non_empty(ctx, left_brace, space_plus_brace, used_style);
        }
    }

    /// RuboCop's `check_empty`.
    fn check_empty(
        &self,
        ctx: &mut Context<'_>,
        left_brace: Span,
        space_plus_brace: Span,
        used_style: Style,
    ) {
        if self.empty_braces_style == used_style {
            return;
        }
        if self.empty_braces_style == Style::Space {
            report_missing(ctx, left_brace);
        } else {
            report_detected(ctx, Span::new(space_plus_brace.start, left_brace.start));
        }
    }

    /// RuboCop's `check_non_empty`.
    fn check_non_empty(
        &self,
        ctx: &mut Context<'_>,
        left_brace: Span,
        space_plus_brace: Span,
        used_style: Style,
    ) {
        if used_style == self.style {
            return;
        }
        if used_style == Style::Space {
            report_detected(ctx, Span::new(space_plus_brace.start, left_brace.start));
        } else {
            report_missing(ctx, left_brace);
        }
    }
}

/// RuboCop's `space_missing`/empty-braces `MISSING_MSG` branch: reports the
/// left brace itself, autocorrecting by inserting a space before it
/// (RuboCop's `autocorrect`: the reported range's source is never
/// whitespace here, so it always takes the `insert_before` branch).
fn report_missing(ctx: &mut Context<'_>, left_brace: Span) {
    let fix = Fix {
        applicability: Applicability::Safe,
        edits: vec![Edit::insert(left_brace.start, b" ".as_slice())],
    };
    ctx.report_with_fix(&SpaceBeforeBlockBraces::META, left_brace, MISSING_MSG, fix);
}

/// RuboCop's `space_detected`/empty-braces `DETECTED_MSG` branch: reports
/// the extra space before the brace, autocorrecting by removing it
/// (RuboCop's `autocorrect`: the reported range's source is always
/// whitespace here, so it always takes the `remove` branch).
fn report_detected(ctx: &mut Context<'_>, space: Span) {
    let fix = Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(space)] };
    ctx.report_with_fix(&SpaceBeforeBlockBraces::META, space, DETECTED_MSG, fix);
}
