//! `Layout/LineEndStringConcatenationIndentation`, ported from RuboCop's
//! `lib/rubocop/cop/layout/line_end_string_concatenation_indentation.rb`
//! plus the `Alignment`/`AlignmentCorrector` machinery
//! (`lib/rubocop/cop/mixin/alignment.rb`,
//! `lib/rubocop/cop/correctors/alignment_corrector.rb`) it uses for its
//! autocorrect.
//!
//! Prism represents a backslash-continued string concatenation
//! (`'a' \` / `'b'`) exactly like whitequark's `dstr`: a top-level
//! [`NodeKind::InterpolatedStringNode`] whose `parts()` are each a complete
//! `StringNode`/`InterpolatedStringNode` for one concatenated literal (even
//! a plain, non-interpolated part), not a flattened list of raw string
//! content -- confirmed against Prism's own dump for
//! `puts 'a' \` / `  "#{b}" \` / `     "#{c}"`, which nests exactly like
//! `parser`'s `s(:dstr, s(:str, "a"), s(:dstr, ...), s(:dstr, ...))`. So
//! [`strings_concatenated_with_backslash`]/[`check_aligned`]/
//! [`check_indented`] are a direct, unmodified port of `on_dstr`'s own
//! logic over `parts()`.
//!
//! `always_indented?`'s whitelist (`PARENT_TYPES_FOR_INDENTED = [nil,
//! :block, :begin, :def, :defs, :if]`) relies on whitequark's
//! single-statement-body elision: a concatenated string that is the *sole*
//! statement of a `def`/`defs`/`block`/`if` branch (or of the whole
//! top-level program) has that construct as its whitequark `parent`
//! directly, and one that is *one of several* statements anywhere -- or
//! that is the sole, explicitly-parenthesized expression of a `(...)`
//! group, which whitequark also wraps in the very same `:begin` node type
//! -- has the `:begin` wrapper as its parent. Prism never elides -- every
//! body is a [`NodeKind::StatementsNode`] regardless of how many
//! statements it holds, and gives explicit parens their own
//! [`NodeKind::ParenthesesNode`] wrapping that `StatementsNode` -- so the
//! Prism-side equivalent, precomputed once per `StatementsNode` in
//! [`LineEndStringConcatenationIndentation::record_statements`], is:
//! "this `StatementsNode` holds 2+ statements" (always matches, like
//! `:begin`) OR "it holds exactly 1 and its *own* parent is a
//! `BlockNode`/`LambdaNode` (a `->` literal, which whitequark also models
//! as a `block` node wrapping the `lambda` send)/`DefNode`/`IfNode`/
//! `ParenthesesNode`, or it has none (the top-level program)".
//! A dstr whose immediate Prism parent is not a `StatementsNode` at all
//! (an argument, a hash value, an array element, ...) is never
//! "always indented", matching every other whitequark parent type being
//! absent from the whitelist.
//!
//! `base_column`'s `grandparent&.pair_type?` (whitequark's hash-pair
//! check, relevant only when `children[0]`'s *own* parent -- the dstr
//! node -- is itself a hash value) becomes a direct
//! `NodeKind::AssocNode` test on the dstr's immediate Prism parent, read
//! straight off [`Context::ancestors`] (no extra bookkeeping needed, since
//! it is not a whitequark-elision case).
//!
//! The autocorrect ports `AlignmentCorrector.correct`'s single-line case
//! only (every child here is single-line by construction, see
//! `strings_concatenated_with_backslash`): a positive `column_delta` is
//! `' ' * column_delta` inserted immediately before the child's own first
//! byte (guarded, as upstream is, against that byte being a `\n`, which
//! can never actually happen for a string/interpolated-string child); a
//! negative one deletes exactly `column_delta.abs` bytes immediately
//! before it, but only when every one of them is a plain space or tab --
//! otherwise (not enough leading whitespace to remove) the offense is
//! still reported, with no fix, exactly like upstream's silently-skipped
//! `corrector.remove`.

use std::collections::HashMap;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG_ALIGN`.
const MSG_ALIGN: &str = "Align parts of a string concatenated with backslash.";
/// RuboCop's `MSG_INDENT`.
const MSG_INDENT: &str = "Indent the first part of a string concatenated with backslash.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Aligned,
    Indented,
}

/// Checks the indentation of the next line after a line that ends with a
/// string literal and a backslash.
#[derive(Debug, Clone)]
pub struct LineEndStringConcatenationIndentation {
    style: Style,
    indentation_width: i64,
    /// Whether a [`NodeKind::StatementsNode`] (keyed by its own span)
    /// satisfies RuboCop's `PARENT_TYPES_FOR_INDENTED` whitelist -- see the
    /// module docs.
    statements_always_indented: HashMap<Span, bool>,
}

impl Rule for LineEndStringConcatenationIndentation {
    const META: RuleMeta = RuleMeta {
        name: "Layout/LineEndStringConcatenationIndentation",
        department: Department::Layout,
        summary: "Checks the indentation of the next line after a line that ends with a string \
                   literal and a backslash.",
        explanation: "\
If `EnforcedStyle: aligned` is set, the concatenated string parts shall be aligned with the
first part. There are some exceptions, such as implicit return values, where the concatenated
string parts shall be indented regardless of `EnforcedStyle` configuration.

If `EnforcedStyle: indented` is set, it's the second line that shall be indented one step more
than the first line. Lines 3 and forward shall be aligned with line 2.

```ruby
# bad
def some_method
  'x' \\
  'y' \\
  'z'
end

my_hash = {
  first: 'a message' \\
    'in two parts'
}

# good
def some_method
  'x' \\
    'y' \\
    'z'
end
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::StatementsNode, NodeKind::InterpolatedStringNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("aligned"),
                allowed: &["aligned", "indented"],
                doc: "The indentation style to enforce for a backslash-continued string \
                      concatenation.",
            },
            ConfigOption {
                name: "IndentationWidth",
                default: ConfigDefault::Nil,
                allowed: &[],
                doc: "Overrides `Layout/IndentationWidth`'s configured width for this cop \
                      alone, for `EnforcedStyle: indented`.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "indented" => Style::Indented,
            _ => Style::Aligned,
        };
        let indentation_width = options
            .get("IndentationWidth")
            .and_then(OptionValue::as_int)
            .or_else(|| {
                options.peer("Layout/IndentationWidth", "Width").and_then(OptionValue::as_int)
            })
            .unwrap_or(2);
        Ok(Self { style, indentation_width, statements_always_indented: HashMap::new() })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.statements_always_indented.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::StatementsNode => self.record_statements(node, ctx),
            NodeKind::InterpolatedStringNode => self.check_dstr(node, ctx),
            _ => {}
        }
    }
}

impl LineEndStringConcatenationIndentation {
    /// Precomputes whether this `StatementsNode` satisfies
    /// `PARENT_TYPES_FOR_INDENTED` -- see the module docs.
    fn record_statements(&mut self, node: &Node<'_>, ctx: &Context<'_>) {
        let stmts = node.as_statements_node().expect("kind matched");
        let count = stmts.body().iter().count();
        // RuboCop's `PARENT_TYPES_FOR_INDENTED` lists `:if` only, because
        // whitequark represents `unless` as a swapped-branch `:if` node and
        // folds an `elsif`/`else` branch into the same `:if` node's own
        // `else_branch` slot -- so a sole-statement `unless` or `else`
        // branch has that same `:if`-typed node as its whitequark parent
        // too. Prism keeps `UnlessNode` and `ElseNode` as their own kinds,
        // so both need listing here for the same effective match.
        let always_indented = count >= 2
            || matches!(
                ctx.parent().map(|p| p.kind),
                None | Some(
                    NodeKind::ProgramNode
                        | NodeKind::BlockNode
                        | NodeKind::LambdaNode
                        | NodeKind::DefNode
                        | NodeKind::IfNode
                        | NodeKind::UnlessNode
                        | NodeKind::ElseNode
                        | NodeKind::ParenthesesNode
                )
            );
        self.statements_always_indented.insert(node.span(), always_indented);
    }

    /// RuboCop's `on_dstr`.
    fn check_dstr(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let dstr = node.as_interpolated_string_node().expect("kind matched");
        let parts: Vec<Node<'_>> = dstr.parts().iter().collect();
        if parts.len() < 2 || ctx.is_single_line(node.span()) {
            return;
        }
        if !parts
            .iter()
            .all(|c| matches!(c.kind(), NodeKind::StringNode | NodeKind::InterpolatedStringNode))
        {
            return;
        }
        if parts.iter().any(|c| !ctx.is_single_line(c.span())) {
            return;
        }

        let always_indented =
            ctx.ancestors().last().is_some_and(|p| p.kind == NodeKind::StatementsNode)
                && ctx
                    .ancestors()
                    .last()
                    .and_then(|p| self.statements_always_indented.get(&p.span))
                    .copied()
                    .unwrap_or(false);

        if self.style == Style::Aligned && !always_indented {
            Self::check_aligned(ctx, &parts, 1);
        } else {
            self.check_indented(ctx, &parts);
            Self::check_aligned(ctx, &parts, 2);
        }
    }

    /// RuboCop's `check_aligned`.
    fn check_aligned(ctx: &mut Context<'_>, parts: &[Node<'_>], start_index: usize) {
        if start_index == 0 || start_index > parts.len() {
            return;
        }
        let mut base_column = i64::from(ctx.line_col(parts[start_index - 1].span().start).column);
        for child in &parts[start_index..] {
            let child_column = i64::from(ctx.line_col(child.span().start).column);
            let delta = base_column - child_column;
            if delta != 0 {
                report(ctx, child, delta, MSG_ALIGN);
            }
            base_column = child_column;
        }
    }

    /// RuboCop's `check_indented`.
    fn check_indented(&self, ctx: &mut Context<'_>, parts: &[Node<'_>]) {
        let base = Self::base_column(ctx, parts[0].span());
        let child_column = i64::from(ctx.line_col(parts[1].span().start).column);
        let delta = base + self.indentation_width - child_column;
        if delta != 0 {
            report(ctx, &parts[1], delta, MSG_INDENT);
        }
    }

    /// RuboCop's `base_column`.
    fn base_column(ctx: &Context<'_>, first_part_span: Span) -> i64 {
        if let Some(parent) = ctx.ancestors().last() {
            if parent.kind == NodeKind::AssocNode {
                return i64::from(ctx.line_col(parent.span.start).column);
            }
        }
        let line = ctx.line_col(first_part_span.start).line;
        let text = ctx.line_text(line);
        i64::try_from(text.iter().position(|&b| !b.is_ascii_whitespace()).unwrap_or(0)).unwrap_or(0)
    }
}

/// RuboCop's `add_offense_and_correction`/`autocorrect`
/// (`AlignmentCorrector.correct`, single-line case -- see the module
/// docs).
fn report(ctx: &mut Context<'_>, child: &Node<'_>, delta: i64, message: &'static str) {
    let span = child.span();
    let mut edits = Vec::new();
    if delta > 0 {
        if ctx.text(Span::new(span.start, span.start + 1)) != b"\n" {
            let count = usize::try_from(delta).unwrap_or(0);
            edits.push(Edit::insert(span.start, vec![b' '; count]));
        }
    } else {
        let count = u32::try_from(-delta).unwrap_or(0);
        if count <= span.start {
            let delete_span = Span::new(span.start - count, span.start);
            if ctx.text(delete_span).iter().all(|&b| b == b' ' || b == b'\t') {
                edits.push(Edit::delete(delete_span));
            }
        }
    }
    if edits.is_empty() {
        ctx.report(&LineEndStringConcatenationIndentation::META, span, message);
    } else {
        ctx.report_with_fix(
            &LineEndStringConcatenationIndentation::META,
            span,
            message,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}
