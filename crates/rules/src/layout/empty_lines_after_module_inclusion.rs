//! `Layout/EmptyLinesAfterModuleInclusion`, ported from RuboCop's
//! `lib/rubocop/cop/layout/empty_lines_after_module_inclusion.rb`.
//!
//! Upstream's `next_line_node` guards against whitequark's single-statement
//! `begin`-elision: a lone `include Foo` inside an `if`/`rescue`/`ensure`
//! branch has that conditional/handler node itself as its whitequark
//! `parent`, so a naive `node.right_sibling` would wrongly return the
//! `else`/handler branch sitting in the same child slot array. Prism never
//! elides -- every branch body is a [`NodeKind::StatementsNode`], so this
//! call's real next statement (if any) is simply the next item of its own
//! `StatementsNode`, precomputed once per `StatementsNode` in
//! [`EmptyLinesAfterModuleInclusion::record_statements`] exactly like
//! `empty_line_after_guard_clause.rs`/`hash_syntax.rs` do for the same
//! upstream trap (see their module docs).
//!
//! `node.parent&.type?(:send, :any_block, :array)` has two more Prism
//! traps: an argument of another call sits under that call's own
//! [`NodeKind::ArgumentsNode`] in Prism (whitequark has no such wrapper, so
//! the argument's whitequark parent is the call node directly) --
//! [`effective_parent_kind`] skips over it. And `:any_block` (a `do...end`/
//! `{...}` block with no parens around its receiver's own arguments, e.g.
//! `extend Module.new do |a| ... end`, where the block binds to `extend`
//! rather than `Module.new`) makes whitequark's `block` node the *parent*
//! of the `extend` send; Prism instead makes the `BlockNode` a *child*
//! (`CallNode::block`) of the call it attaches to, so this is checked as
//! `call.block().is_some()` instead of an ancestor-kind test.

use std::collections::HashSet;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_directives::DirectiveKind;
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Add an empty line after module inclusion.";

/// RuboCop's `MODULE_INCLUSION_METHODS`.
fn is_module_inclusion_method(name: &[u8]) -> bool {
    matches!(name, b"include" | b"extend" | b"prepend")
}

/// Keeps track of empty lines after module inclusion methods.
#[derive(Debug, Clone, Default)]
pub struct EmptyLinesAfterModuleInclusion {
    /// Spans of `StatementsNode` items whose real next sibling exists and
    /// is *not* itself an allowed module-inclusion continuation (RuboCop's
    /// `require_empty_line?(next_line_node)` being `true`). An item whose
    /// span is absent here either has no next sibling or one that is
    /// itself allowed, so needs no following blank line.
    needs_check: HashSet<Span>,
}

impl Rule for EmptyLinesAfterModuleInclusion {
    const META: RuleMeta = RuleMeta {
        name: "Layout/EmptyLinesAfterModuleInclusion",
        department: Department::Layout,
        summary: "Keeps track of empty lines after module inclusion methods.",
        explanation: "\
Checks for an empty line after a module inclusion method (`extend`,
`include` and `prepend`), or a group of them.

```ruby
# bad
class Foo
  include Bar
  attr_reader :baz
end

# good
class Foo
  include Bar

  attr_reader :baz
end

# also good - multiple module inclusions grouped together
class Foo
  extend Bar
  include Baz
  prepend Qux
end
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::StatementsNode, NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.needs_check.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::StatementsNode => self.record_statements(node),
            NodeKind::CallNode => self.check_call(node, ctx),
            _ => {}
        }
    }
}

impl EmptyLinesAfterModuleInclusion {
    /// RuboCop's `right_sibling`/`require_empty_line?`, precomputed for
    /// every item but the last of one `StatementsNode`'s body.
    fn record_statements(&mut self, node: &Node<'_>) {
        let stmts = node.as_statements_node().expect("kind matched");
        let items: Vec<Node<'_>> = stmts.body().iter().collect();
        for pair in items.windows(2) {
            if !allowed_method(&pair[1]) {
                self.needs_check.insert(pair[0].span());
            }
        }
    }

    /// RuboCop's `on_send` plus `autocorrect`.
    fn check_call(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node().expect("kind matched");
        if call.receiver().is_some() {
            return;
        }
        if !is_module_inclusion_method(call.name().as_slice()) {
            return;
        }
        let Some(args) = call.arguments() else { return };
        if args.arguments().iter().next().is_none() {
            return;
        }
        if call.block().is_some() {
            return;
        }
        if matches!(effective_parent_kind(ctx), Some(NodeKind::CallNode | NodeKind::ArrayNode)) {
            return;
        }

        let span = node.span();
        let last_line = ctx.last_line(span);
        if next_line_empty_or_enable_directive_comment(ctx, last_line) {
            return;
        }
        if !self.needs_check.contains(&span) {
            return;
        }

        let mut node_range = ctx.whole_lines(span);
        let next_line = ctx.last_line(node_range) + 1;
        if let Some(comment_span) = enable_directive_comment_span(ctx, next_line) {
            node_range = comment_span;
        }

        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::insert(node_range.end, b"\n".to_vec())],
            },
        );
    }
}

/// RuboCop's `allowed_method?`: unwraps a modifier-form `if`/`unless`/
/// `while`/`until` wrapping a single statement to that statement, then
/// requires a bare (no receiver filtering needed here, upstream only
/// checks `method_name`) module-inclusion call.
fn allowed_method(node: &Node<'_>) -> bool {
    let body = modifier_body(node).unwrap_or(*node);
    let Some(call) = body.as_call_node() else { return false };
    is_module_inclusion_method(call.name().as_slice())
}

/// The sole statement of a modifier-form `if`/`unless`/`while`/`until`
/// (RuboCop-AST's `modifier_form?`: no explicit `end` keyword), if `node`
/// is one.
fn modifier_body<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    let statements = match node.kind() {
        NodeKind::IfNode => {
            let n = node.as_if_node()?;
            (n.if_keyword_loc().is_some() && n.end_keyword_loc().is_none())
                .then(|| n.statements())
                .flatten()
        }
        NodeKind::UnlessNode => {
            let n = node.as_unless_node()?;
            n.end_keyword_loc().is_none().then(|| n.statements()).flatten()
        }
        NodeKind::WhileNode => {
            let n = node.as_while_node()?;
            n.closing_loc().is_none().then(|| n.statements()).flatten()
        }
        NodeKind::UntilNode => {
            let n = node.as_until_node()?;
            n.closing_loc().is_none().then(|| n.statements()).flatten()
        }
        _ => None,
    }?;
    statements.body().iter().next()
}

/// RuboCop's `node.parent&.type?(:send, ...)`, skipping over the
/// `ArgumentsNode` wrapper Prism puts between a call and its arguments
/// (whitequark has none, so an argument's whitequark `parent` is the call
/// node directly). See the module docs.
fn effective_parent_kind(ctx: &Context<'_>) -> Option<NodeKind> {
    let ancestors = ctx.ancestors();
    let mut idx = ancestors.len();
    while idx > 0 {
        idx -= 1;
        if ancestors[idx].kind == NodeKind::ArgumentsNode {
            continue;
        }
        return Some(ancestors[idx].kind);
    }
    None
}

/// RuboCop's `next_line_empty_or_enable_directive_comment?`, called with
/// `node.last_line`: `line_empty?(line)` reads `processed_source[line]`,
/// which is a *0-based* Ruby array index into 1-based physical lines, so
/// `line_empty?(node.last_line)` actually tests physical line
/// `node.last_line + 1` -- the line right after the node, matching the
/// method's name. `enable_directive_comment?`/`comment_at_line` has no
/// such quirk (its lookup is keyed by real 1-based line numbers), so
/// `enable_directive_comment?(line + 1)` tests that same next line
/// properly; the second `line_empty?(line + 1)` then inherits the same
/// 0-based-index quirk a second time, testing physical line `line + 2`.
fn next_line_empty_or_enable_directive_comment(ctx: &Context<'_>, line: u32) -> bool {
    is_blank_line(ctx, line + 1)
        || (is_enable_directive_line(ctx, line + 1) && is_blank_line(ctx, line + 2))
}

/// RuboCop's `line_empty?`: a 1-based physical line past the end of the
/// file, or blank.
fn is_blank_line(ctx: &Context<'_>, line: u32) -> bool {
    line > ctx.line_count() || ctx.line_text(line).iter().all(u8::is_ascii_whitespace)
}

/// RuboCop's `enable_directive_comment?`: a `# rubocop:enable`/
/// `enable-next` directive comment on the given 1-based physical line
/// (`DirectiveComment#enabled?`).
fn is_enable_directive_line(ctx: &Context<'_>, line: u32) -> bool {
    ctx.directives().directives().iter().any(|d| {
        d.line == line && matches!(d.kind, DirectiveKind::Enable | DirectiveKind::EnableNext)
    })
}

/// The span of the `# rubocop:enable`/`enable-next` comment on the given
/// 1-based physical line, if any (RuboCop's
/// `processed_source.comment_at_line`, restricted to the one case
/// `autocorrect` uses it for).
fn enable_directive_comment_span(ctx: &Context<'_>, line: u32) -> Option<Span> {
    is_enable_directive_line(ctx, line)
        .then(|| ctx.comments().iter().find(|c| c.line == line).map(|c| c.span))
        .flatten()
}
