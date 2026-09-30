//! `Layout/AccessModifierIndentation`, ported from RuboCop's
//! `lib/rubocop/cop/layout/access_modifier_indentation.rb` plus the `Alignment` mixin
//! (`lib/rubocop/cop/mixin/alignment.rb`) and `AlignmentCorrector`
//! (`lib/rubocop/cop/correctors/alignment_corrector.rb`) it uses for autocorrection.
//!
//! Upstream hooks `on_class`/`on_sclass`/`on_module`/`on_block` (all aliased to the same
//! handler) and only inspects a body that is whitequark's `begin_type?` -- i.e. two or more
//! statements. Prism always wraps a body in a [`NodeKind::StatementsNode`], even for a single
//! statement, so the equivalent guard here is `body.len() > 1`; a body that is not a
//! `StatementsNode` at all (`nil`, or an implicit `BeginNode` wrapper for a class/module/block
//! whose body has `rescue`/`ensure`) is skipped the same way whitequark's non-`begin_type?`
//! bodies are.
//!
//! A `BlockNode`'s `closing_loc` (its `end` keyword, or its `}` for a brace block) stands in for
//! `node.loc.end` on the other three node kinds.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::is_bare_access_modifier;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Indent,
    Outdent,
}

/// Checks indentation of private/protected visibility modifiers.
#[derive(Debug, Clone)]
pub struct AccessModifierIndentation {
    style: Style,
    /// `Alignment#configured_indentation_width`: this cop's own `IndentationWidth`, else
    /// `Layout/IndentationWidth`'s `Width`, else 2.
    indentation_width: i64,
}

impl Rule for AccessModifierIndentation {
    const META: RuleMeta = RuleMeta {
        name: "Layout/AccessModifierIndentation",
        department: Department::Layout,
        summary: "Checks indentation of private/protected visibility modifiers.",
        explanation: "\
Bare access modifiers (those not applying to specific methods) should be
indented as deep as method definitions, or as deep as the `class`/`module`
keyword, depending on configuration.

```ruby
# EnforcedStyle: indent (default)

# bad
class Plumbus
private
  def smooth; end
end

# good
class Plumbus
  private
  def smooth; end
end
```

```ruby
# EnforcedStyle: outdent

# bad
class Plumbus
  private
  def smooth; end
end

# good
class Plumbus
private
  def smooth; end
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::ClassNode,
            NodeKind::ModuleNode,
            NodeKind::SingletonClassNode,
            NodeKind::BlockNode,
        ],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("indent"),
                allowed: &["outdent", "indent"],
                doc: "The indentation style used to align access modifiers.",
            },
            ConfigOption {
                name: "IndentationWidth",
                default: ConfigDefault::Nil,
                allowed: &[],
                doc: "\
Number of spaces to use for indentation. Defaults to `Layout/IndentationWidth`'s `Width`.",
            },
        ],
        blind_spots: "\
RuboCop's `ConfigurableEnforcedStyle` auto-style-detection bookkeeping \
(`correct_style_detected`/`opposite_style_detected`/`unrecognized_style_detected`, used only by \
`rubocop --auto-gen-config`) is not replicated; it never changes whether an offense is reported \
or how it is fixed.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "outdent" => Style::Outdent,
            _ => Style::Indent,
        };
        let indentation_width = options
            .get("IndentationWidth")
            .and_then(OptionValue::as_int)
            .or_else(|| {
                options.peer("Layout/IndentationWidth", "Width").and_then(OptionValue::as_int)
            })
            .unwrap_or(2);
        Ok(Self { style, indentation_width })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let (end_span, body) = match node {
            Node::ClassNode { .. } => {
                let class = node.as_class_node().expect("kind matched");
                (class.end_keyword_loc().span(), class.body())
            }
            Node::ModuleNode { .. } => {
                let module = node.as_module_node().expect("kind matched");
                (module.end_keyword_loc().span(), module.body())
            }
            Node::SingletonClassNode { .. } => {
                let sclass = node.as_singleton_class_node().expect("kind matched");
                (sclass.end_keyword_loc().span(), sclass.body())
            }
            Node::BlockNode { .. } => {
                let block = node.as_block_node().expect("kind matched");
                (block.closing_loc().span(), block.body())
            }
            _ => return,
        };
        let Some(body) = body else { return };
        let Some(statements) = body.as_statements_node() else { return };
        let list = statements.body();
        if list.len() <= 1 {
            return;
        }
        for child in &list {
            let Some(call) = child.as_call_node() else { continue };
            if !is_bare_access_modifier(&call) {
                continue;
            }
            let modifier_span = call.as_node().span();
            if ctx.same_line(node.span(), modifier_span) {
                continue;
            }
            self.check_modifier(ctx, modifier_span, end_span);
        }
    }
}

impl AccessModifierIndentation {
    /// RuboCop's `check_modifier`.
    fn check_modifier(&self, ctx: &mut Context<'_>, modifier_span: Span, end_span: Span) {
        let offset = i64::from(ctx.display_column(modifier_span.start))
            - i64::from(ctx.display_column(end_span.start));
        let expected_indent_offset =
            if self.style == Style::Outdent { 0 } else { self.indentation_width };
        let column_delta = expected_indent_offset - offset;
        if column_delta == 0 {
            return;
        }
        let style_name = match self.style {
            Style::Indent => "Indent",
            Style::Outdent => "Outdent",
        };
        let text = String::from_utf8_lossy(ctx.text(modifier_span));
        let message = format!("{style_name} access modifiers like `{text}`.");
        match build_shift_fix(ctx, modifier_span, column_delta) {
            Some(fix) => ctx.report_with_fix(&Self::META, modifier_span, message, fix),
            None => ctx.report(&Self::META, modifier_span, message),
        }
    }
}

/// RuboCop's `AlignmentCorrector.correct`: shifts every physical line of `span` by
/// `column_delta` columns. Returns `None` when nothing could be safely edited (a `=begin`/`=end`
/// block comment inside the range, or the shift was blocked by a taboo range/whitespace
/// mismatch); a bare access modifier is always a single-line node, so no heredoc taboo range can
/// ever intersect it.
fn build_shift_fix(ctx: &Context<'_>, span: Span, column_delta: i64) -> Option<Fix> {
    let delta = i32::try_from(column_delta).unwrap_or(0);
    let edits = linter::shift_lines(ctx, span, delta, &[]);
    if edits.is_empty() {
        None
    } else {
        Some(Fix { applicability: Applicability::Safe, edits })
    }
}
