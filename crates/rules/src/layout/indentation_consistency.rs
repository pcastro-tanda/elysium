//! `Layout/IndentationConsistency`, ported from RuboCop's
//! `lib/rubocop/cop/layout/indentation_consistency.rb` plus its `Alignment`
//! mixin and `AlignmentCorrector`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::is_bare_access_modifier;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Inconsistent indentation detected.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    /// Entities at the same logical depth share the same indentation,
    /// including `public`/`protected`/`private` markers themselves.
    Normal,
    /// Like `Normal`, but the members below a `protected`/`private` marker
    /// are indented one step further than the marker.
    IndentedInternalMethods,
}

/// Checks for inconsistent indentation, ported from RuboCop's
/// `IndentationConsistency` cop plus its `Alignment` mixin and
/// `AlignmentCorrector`.
#[derive(Debug, Clone)]
pub struct IndentationConsistency {
    style: Style,
    /// Spans already reported by this rule in this file, in visitation
    /// order (RuboCop's `@current_offenses`): an offense nested inside one
    /// already reported is emitted without a fix instead of colliding with
    /// the outer correction.
    reported: Vec<Span>,
}

impl Rule for IndentationConsistency {
    const META: RuleMeta = RuleMeta {
        name: "Layout/IndentationConsistency",
        department: Department::Layout,
        summary: "Keep indentation straight.",
        explanation: "\
Checks that entities at the same logical depth share the same indentation.
The `indented_internal_methods` style additionally requires that a bare
`protected`/`private`/`public`/`module_function` marker stay flush with the
surrounding methods, while the members below it are indented one step
further than the marker.

```ruby
# EnforcedStyle: normal (default)

# bad
class A
  def test
    puts 'hello'
     puts 'world'
  end
end

# good
class A
  def test
    puts 'hello'
    puts 'world'
  end

  protected

  def foo
  end
end
```

```ruby
# EnforcedStyle: indented_internal_methods

# bad
class A
  def test
    puts 'hello'
     puts 'world'
  end
end

# good
class A
  def test
    puts 'hello'
    puts 'world'
  end

  protected

    def foo
    end
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::StatementsNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("normal"),
            allowed: &["normal", "indented_internal_methods"],
            doc: "Whether `protected`/`private` markers must be indented \
                  like the surrounding methods (`normal`) or may sit flush \
                  with a deeper-indented internal section \
                  (`indented_internal_methods`).",
        }],
        blind_spots: "\
`bare_access_modifier?` does not replicate RuboCop's recursive
`in_macro_scope?` check (it does not confirm the call sits directly in a
class/module/top-level body through only `begin`/`kwbegin`/block/`if`
wrappers): any receiver-less, argument-less `public`/`protected`/`private`/
`module_function` call is treated as a group divider or excluded from
alignment, even one nested inside a `def` or loop body. That only narrows a
comparison into smaller groups (or drops one item entirely), never merges
two real groups into one, so it can only produce false negatives.
`private()`/`protected()` with empty parentheses are not recognized as bare
modifiers (RuboCop's AST does not distinguish them from a no-args call, but
our simpler receiver/arguments check does), which is a false-negative-only
divergence in the same direction.

Autocorrection's taboo-range protection (RuboCop's `AlignmentCorrector`
`inside_string_ranges`) only covers heredoc bodies; the interior of an
ordinary multi-line quoted string or `%`-literal that itself begins a
physical line inside a misaligned body is not separately protected. The
block-comment guard is a per-line `=begin` text match rather than resolving
actual `EmbDoc` comment nodes, matching this crate's other `Alignment`-based
cops.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "indented_internal_methods" => Style::IndentedInternalMethods,
            _ => Style::Normal,
        };
        Ok(Self { style, reported: Vec::new() })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.reported.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(statements) = node.as_statements_node() else { return };
        let body = statements.body();
        match self.style {
            Style::Normal => {
                let first_raw = body.first();
                let base_column = Self::base_column_for_normal_style(ctx, first_raw.as_ref());
                let items: Vec<Node<'_>> =
                    body.iter().filter(|child| !is_bare_access_modifier_node(child)).collect();
                self.check_alignment(ctx, &items, base_column);
            }
            Style::IndentedInternalMethods => {
                let mut groups: Vec<Vec<Node<'_>>> = vec![Vec::new()];
                for child in &body {
                    if is_bare_access_modifier_node(&child) {
                        groups.push(Vec::new());
                    } else {
                        groups.last_mut().expect("seeded with one group").push(child);
                    }
                }
                for group in groups {
                    self.check_alignment(ctx, &group, None);
                }
            }
        }
    }
}

impl IndentationConsistency {
    /// RuboCop's `base_column_for_normal_style`: `None` means "derive the
    /// base column from the first item being checked"; `Some` overrides it
    /// with the column of a bare access modifier that leads the raw
    /// (unfiltered) child list.
    fn base_column_for_normal_style(
        ctx: &Context<'_>,
        first_raw: Option<&Node<'_>>,
    ) -> Option<u32> {
        let first = first_raw?;
        if !is_bare_access_modifier_node(first) {
            return None;
        }
        let access_modifier_indent = ctx.display_column(first.span().start);
        let parent = ctx.parent()?;
        if parent.kind == NodeKind::ProgramNode {
            return Some(access_modifier_indent);
        }
        let parent_column = ctx.display_column(parent.span.start);
        (access_modifier_indent > parent_column).then_some(access_modifier_indent)
    }

    /// RuboCop's `Alignment#check_alignment` + `#each_bad_alignment`.
    fn check_alignment(
        &mut self,
        ctx: &mut Context<'_>,
        items: &[Node<'_>],
        base_column: Option<u32>,
    ) {
        let Some(first) = items.first() else { return };
        let base_column = base_column.unwrap_or_else(|| ctx.display_column(first.span().start));
        let mut prev_line: i64 = -1;
        for item in items {
            let span = item.span();
            let line = i64::from(ctx.line_col(span.start).line);
            if line > prev_line && ctx.begins_its_line(span) {
                let column_delta =
                    i64::from(base_column) - i64::from(ctx.display_column(span.start));
                if column_delta != 0 {
                    self.register_offense(ctx, item, column_delta);
                }
            }
            prev_line = line;
        }
    }

    /// RuboCop's `Alignment#register_offense`: an offense whose range falls
    /// inside one this rule already reported in this file is emitted
    /// without a fix (two rewrites of the same region in one pass cannot be
    /// handled; the next fix iteration finds it again).
    fn register_offense(&mut self, ctx: &mut Context<'_>, item: &Node<'_>, column_delta: i64) {
        let span = item.span();
        let nested = self.reported.iter().any(|reported| reported.contains(span));
        self.reported.push(span);
        if nested {
            ctx.report(&Self::META, span, MSG);
            return;
        }
        match build_fix(ctx, item, column_delta) {
            Some(fix) => ctx.report_with_fix(&Self::META, span, MSG, fix),
            None => ctx.report(&Self::META, span, MSG),
        }
    }
}

/// `is_bare_access_modifier` shape-checked against a generic `Node`, for the
/// call sites here that only have a raw AST child, not an already-narrowed
/// `CallNode`.
fn is_bare_access_modifier_node(node: &Node<'_>) -> bool {
    node.as_call_node().is_some_and(|call| is_bare_access_modifier(&call))
}

/// RuboCop's `AlignmentCorrector.correct`: shifts every physical line of
/// `item` by `column_delta` columns. Returns `None` when nothing could be
/// safely edited (a `=begin`/`=end` block comment inside the range, or
/// every line was blocked by a taboo range/whitespace mismatch).
fn build_fix(ctx: &Context<'_>, item: &Node<'_>, column_delta: i64) -> Option<Fix> {
    let span = item.span();
    let taboo = linter::heredoc_bodies(ctx, item);
    let delta = i32::try_from(column_delta).unwrap_or(0);
    let edits = linter::shift_lines(ctx, span, delta, &taboo);
    if edits.is_empty() {
        None
    } else {
        Some(Fix { applicability: Applicability::Safe, edits })
    }
}
