//! `Style/Next`, ported from RuboCop's `lib/rubocop/cop/style/next.rb` plus
//! the `MinBodyLength` mixin it includes.
//!
//! Whitequark represents both `if` and `unless` as one `:if`-typed node
//! (branches swapped for `unless`), and elides the `begin` wrapper for a
//! single-statement body. Prism instead gives `if`/`unless` their own
//! [`NodeKind::IfNode`]/[`NodeKind::UnlessNode`], and always wraps a body in
//! a [`NodeKind::StatementsNode`], single statement or not. [`CondBranch`]
//! normalizes both node kinds into one shape (own span, keyword span,
//! predicate, optional `then`, body statements, optional `end` -- `None`
//! means modifier form) so the rest of the port reads uniformly; the last
//! (or, for a single-statement body, only) statement of the enclosing
//! block/loop's own [`StatementsNode`] is checked directly, without needing
//! upstream's `*_, condition = *body` reconstruction.
//!
//! Upstream's `if_else_children?`/`if_without_else?` pair rejects a
//! candidate that itself has *any* trailing clause -- another `elsif` link
//! or a real `else` alike (RuboCop-AST's `Node#else?` is `loc?(:else)`,
//! true for either, since whitequark's location map for a chained `elsif`
//! sets the outer node's own `:else` slot to that link's keyword) -- or
//! whose single-statement body's one statement is itself an `if`/`unless`
//! with such a clause. [`is_disqualified_by_else`] mirrors both checks over
//! Prism's `subsequent`/`else_clause`/single-item `statements` shapes.
//!
//! `on_block`/`on_numblock`/`on_itblock` collapse onto one Prism
//! [`NodeKind::CallNode`] check (block param shape never matters here);
//! `on_while`/`on_until`/`on_for` become their own three `NodeKind`s.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{IfNode, NodeList, StatementsNode, UnlessNode};
use ruby_ast::{each_descendant, ext, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Use `next` to skip iteration.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    SkipModifierIfs,
    Always,
}

/// RuboCop-AST's `MethodIdentifierPredicates::ENUMERATOR_METHODS`, plus the
/// `each_*` prefix check `enumerator_method?` also allows.
fn is_enumerator_method(name: &[u8]) -> bool {
    matches!(
        name,
        b"collect"
            | b"collect_concat"
            | b"detect"
            | b"downto"
            | b"each"
            | b"find"
            | b"find_all"
            | b"find_index"
            | b"inject"
            | b"loop"
            | b"map!"
            | b"map"
            | b"reduce"
            | b"reject"
            | b"reject!"
            | b"reverse_each"
            | b"select"
            | b"select!"
            | b"times"
            | b"upto"
    ) || name.starts_with(b"each_")
}

/// A normalized `if`/`unless` node -- see the module doc.
struct CondBranch<'pr> {
    /// The node itself, for a heredoc-descendant scan during autocorrect
    /// and for [`is_disqualified_by_else`].
    node: Node<'pr>,
    node_span: Span,
    keyword_span: Span,
    predicate_span: Span,
    then_keyword_span: Option<Span>,
    statements: Option<StatementsNode<'pr>>,
    /// `None` means modifier form (RuboCop-AST's `modifier_form?`).
    end_keyword_span: Option<Span>,
    is_unless: bool,
}

impl<'pr> CondBranch<'pr> {
    fn from_node(node: &Node<'pr>) -> Option<Self> {
        match node.kind() {
            NodeKind::IfNode => node.as_if_node().and_then(|n| Self::from_if(&n)),
            NodeKind::UnlessNode => node.as_unless_node().map(|n| Self::from_unless(&n)),
            _ => None,
        }
    }

    /// `None` for a ternary (no `if`/`then`/`end` keywords at all).
    fn from_if(n: &IfNode<'pr>) -> Option<Self> {
        let keyword_span = n.if_keyword_loc()?.span();
        Some(Self {
            node: n.as_node(),
            node_span: n.as_node().span(),
            keyword_span,
            predicate_span: n.predicate().span(),
            then_keyword_span: n.then_keyword_loc().map(|l| l.span()),
            statements: n.statements(),
            end_keyword_span: n.end_keyword_loc().map(|l| l.span()),
            is_unless: false,
        })
    }

    fn from_unless(n: &UnlessNode<'pr>) -> Self {
        Self {
            node: n.as_node(),
            node_span: n.as_node().span(),
            keyword_span: n.keyword_loc().span(),
            predicate_span: n.predicate().span(),
            then_keyword_span: n.then_keyword_loc().map(|l| l.span()),
            statements: n.statements(),
            end_keyword_span: n.end_keyword_loc().map(|l| l.span()),
            is_unless: true,
        }
    }
}

/// Whether `node` (an `if`/`unless`) has *any* trailing clause at all --
/// another `elsif` link or a real `else` alike. RuboCop-AST's `Node#else?`
/// (`loc?(:else)`) is true for either: whitequark's location map for a
/// chained `elsif` sets the outer node's own `:else` slot to that `elsif`'s
/// keyword, not only to a genuine trailing `else` keyword.
fn has_any_clause(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::IfNode => node.as_if_node().is_some_and(|n| n.subsequent().is_some()),
        NodeKind::UnlessNode => node.as_unless_node().is_some_and(|n| n.else_clause().is_some()),
        _ => false,
    }
}

/// RuboCop's `if_without_else?` combined with `if_else_children?`: `branch`
/// itself must have no trailing `elsif`/`else` at all, and neither may its
/// single-statement body's one statement (when that statement is itself an
/// `if`/`unless`) -- upstream's `each_child_node(:if).any?(&:else?)` reaches
/// exactly that one direct child.
fn is_disqualified_by_else(branch: &CondBranch<'_>) -> bool {
    if has_any_clause(&branch.node) {
        return true;
    }
    if let Some(stmts) = branch.statements {
        let body = stmts.body();
        if body.len() == 1 {
            if let Some(only) = body.first() {
                if has_any_clause(&only) {
                    return true;
                }
            }
        }
    }
    false
}

/// RuboCop's `EXIT_TYPES.include?(node.if_branch.type)`: the branch's body
/// is exactly one `break`/`return` statement.
fn is_exit_body(branch: &CondBranch<'_>) -> bool {
    let Some(stmts) = branch.statements else { return false };
    let body = stmts.body();
    if body.len() != 1 {
        return false;
    }
    body.first()
        .is_some_and(|only| matches!(only.kind(), NodeKind::BreakNode | NodeKind::ReturnNode))
}

/// RuboCop-AST's `Node#left_sibling` on the offending node, restricted to
/// what `consecutive_conditionals?` needs: whether the statement right
/// before the last one (the only case this cop's `offending_node` is ever
/// the last item of a multi-statement body) is itself an `if`/`unless`.
fn previous_is_conditional(items: &NodeList<'_>) -> bool {
    if items.len() < 2 {
        return false;
    }
    let mut previous = None;
    let mut current = items.first();
    for item in items.iter().skip(1) {
        previous = current;
        current = Some(item);
    }
    previous.is_some_and(|p| matches!(p.kind(), NodeKind::IfNode | NodeKind::UnlessNode))
}

/// The content span of a heredoc node (its body, excluding the closing
/// delimiter line), for [`reindent_lines`]'s exclusion set. `None` for a
/// non-heredoc or a heredoc missing an expected location.
fn heredoc_content_span(node: &Node<'_>) -> Option<Span> {
    if !ext::is_heredoc(node) {
        return None;
    }
    match node.kind() {
        NodeKind::StringNode => node.as_string_node().map(|n| n.content_loc().span()),
        NodeKind::XStringNode => node.as_x_string_node().map(|n| n.content_loc().span()),
        NodeKind::InterpolatedStringNode => {
            let n = node.as_interpolated_string_node()?;
            Some(Span::new(n.opening_loc()?.span().end, n.closing_loc()?.span().start))
        }
        NodeKind::InterpolatedXStringNode => {
            let n = node.as_interpolated_x_string_node()?;
            Some(Span::new(n.opening_loc().span().end, n.closing_loc().span().start))
        }
        _ => None,
    }
}

/// Index of the first non-space/tab byte, or the text's length if none.
fn indent_of(text: &[u8]) -> u32 {
    u32::try_from(text.iter().take_while(|&&b| b == b' ' || b == b'\t').count()).unwrap_or(u32::MAX)
}

fn is_blank(text: &[u8]) -> bool {
    text.iter().all(|&b| b == b' ' || b == b'\t' || b == b'\r')
}

/// RuboCop's `reindentable_lines`/`reindent`: dedents every non-blank,
/// non-heredoc-body line strictly between the node's own line and its `end`
/// keyword's line down to the condition's own indentation, never indenting
/// further (RuboCop's `adjustment.positive?` guard).
fn reindent_lines(
    edits: &mut Vec<Edit>,
    branch: &CondBranch<'_>,
    end_span: Span,
    ctx: &Context<'_>,
) {
    let start_line = ctx.line_col(branch.node_span.start).line;
    let end_line = ctx.line_col(end_span.start).line;
    if start_line + 1 >= end_line {
        return;
    }

    let mut heredoc_ranges: Vec<(u32, u32)> = Vec::new();
    each_descendant(&branch.node, &mut |n| {
        if let Some(span) = heredoc_content_span(n) {
            let s = ctx.line_col(span.start).line;
            let e = ctx.line_col(span.end).line;
            if e > s {
                heredoc_ranges.push((s, e));
            }
        }
    });
    let in_heredoc = |line: u32| heredoc_ranges.iter().any(|&(s, e)| line >= s && line < e);

    let mut lines: Vec<u32> = Vec::new();
    for line in start_line + 1..end_line {
        if in_heredoc(line) || is_blank(ctx.line_text(line)) {
            continue;
        }
        lines.push(line);
    }
    let Some(actual_indent) = lines.iter().map(|&l| indent_of(ctx.line_text(l))).min() else {
        return;
    };
    let cond_line = ctx.line_col(branch.predicate_span.start).line;
    let target_indent = indent_of(ctx.line_text(cond_line));
    if actual_indent <= target_indent {
        return;
    }
    let delta = actual_indent - target_indent;
    for line in lines {
        let line_span = ctx.line_span(line);
        edits.push(Edit::delete(Span::new(line_span.start, line_span.start + delta)));
    }
}

/// RuboCop's `autocorrect_modifier`: replaces the whole modifier-form node
/// (which starts at its own body statement, not at the `if`/`unless`
/// keyword) with `next <inverse> <condition>` followed by the body
/// statement reindented to the node's own original column.
fn autocorrect_modifier(branch: &CondBranch<'_>, ctx: &Context<'_>) -> Fix {
    let cond_text = ctx.text(branch.predicate_span);
    let inverse: &[u8] = if branch.is_unless { b"if" } else { b"unless" };
    let body_span = branch.statements.map_or(branch.predicate_span, |s| s.location().span());
    let body_text = ctx.text(body_span);
    let column = ctx.line_col(branch.node_span.start).column;

    let mut replacement =
        Vec::with_capacity(6 + inverse.len() + cond_text.len() + body_text.len() + column as usize);
    replacement.extend_from_slice(b"next ");
    replacement.extend_from_slice(inverse);
    replacement.push(b' ');
    replacement.extend_from_slice(cond_text);
    replacement.push(b'\n');
    replacement.extend(std::iter::repeat_n(b' ', column as usize));
    replacement.extend_from_slice(body_text);

    Fix {
        applicability: Applicability::Safe,
        edits: vec![Edit::replace(branch.node_span, replacement)],
    }
}

/// RuboCop's `autocorrect_block`: inserts `next <inverse> <condition>`
/// before the node, removes the `if <condition>[ then]` prefix and the
/// dangling `end` (merging its line away, keeping exactly one newline),
/// then dedents the body to match.
fn autocorrect_block(branch: &CondBranch<'_>, ctx: &Context<'_>) -> Fix {
    let cond_text = ctx.text(branch.predicate_span).to_vec();
    let inverse: &[u8] = if branch.is_unless { b"if" } else { b"unless" };
    let mut next_code = Vec::with_capacity(6 + inverse.len() + cond_text.len());
    next_code.extend_from_slice(b"next ");
    next_code.extend_from_slice(inverse);
    next_code.push(b' ');
    next_code.extend_from_slice(&cond_text);

    let mut edits = vec![Edit::insert(branch.node_span.start, next_code)];

    let cond_delete_end = branch.then_keyword_span.map_or(branch.predicate_span.end, |s| s.end);
    edits.push(Edit::delete(Span::new(branch.node_span.start, cond_delete_end)));

    let end_span = branch.end_keyword_span.expect("block form");
    let end_line = ctx.line_col(end_span.start).line;
    let line_span = ctx.line_span(end_line);
    let trailing = ctx.text(Span::new(end_span.end, line_span.end));
    let mut begin_pos = line_span.start;
    if is_blank(trailing) {
        begin_pos = begin_pos.saturating_sub(1);
    }
    edits.push(Edit::delete(Span::new(begin_pos, end_span.end)));

    reindent_lines(&mut edits, branch, end_span, ctx);

    Fix { applicability: Applicability::Safe, edits }
}

/// Use `next` to skip iteration instead of a condition at the end.
#[derive(Debug, Clone)]
pub struct Next {
    style: Style,
    allow_consecutive_conditionals: bool,
    min_body_length: i64,
}

impl Next {
    /// RuboCop's `allowed_modifier_if?`.
    fn allowed_modifier_if(&self, branch: &CondBranch<'_>, ctx: &Context<'_>) -> bool {
        match branch.end_keyword_span {
            None => self.style == Style::SkipModifierIfs,
            Some(end_span) => {
                let keyword_line = i64::from(ctx.line_col(branch.keyword_span.start).line);
                let end_line = i64::from(ctx.line_col(end_span.start).line);
                (end_line - keyword_line) <= self.min_body_length
            }
        }
    }

    /// RuboCop's `simple_if_without_break?`.
    fn simple_if_without_break(&self, branch: &CondBranch<'_>, ctx: &Context<'_>) -> bool {
        if is_disqualified_by_else(branch) {
            return false;
        }
        if self.allowed_modifier_if(branch, ctx) {
            return false;
        }
        !is_exit_body(branch)
    }

    /// RuboCop's `check`: examines the enclosing block/loop's own body,
    /// looking at its last statement (or its only one, for a
    /// single-statement body -- see the module doc).
    fn check(&self, body: Option<StatementsNode<'_>>, ctx: &mut Context<'_>) {
        let Some(body) = body else { return };
        let items = body.body();
        let candidate = match items.len() {
            0 => return,
            1 => items.first(),
            _ => items.last(),
        };
        let Some(candidate) = candidate else { return };
        let Some(branch) = CondBranch::from_node(&candidate) else { return };
        if !self.simple_if_without_break(&branch, ctx) {
            return;
        }
        if self.allow_consecutive_conditionals && previous_is_conditional(&items) {
            return;
        }

        let span = Span::new(branch.node_span.start, branch.predicate_span.end);
        let fix = if branch.end_keyword_span.is_none() {
            autocorrect_modifier(&branch, ctx)
        } else {
            autocorrect_block(&branch, ctx)
        };
        ctx.report_with_fix(&Self::META, span, MSG, fix);
    }
}

impl Rule for Next {
    const META: RuleMeta = RuleMeta {
        name: "Style/Next",
        department: Department::Style,
        summary: "Use `next` to skip iteration instead of a condition at the end.",
        explanation: "\
Use `next` to skip iteration instead of a condition at the end.

```ruby
# bad
[1, 2].each do |a|
  if a == 1
    puts a
  end
end

# good
[1, 2].each do |a|
  next unless a == 1
  puts a
end
```

With `EnforcedStyle: always` (default `skip_modifier_ifs`), a modifier `if`
at the end of an iteration is converted too:

```ruby
# bad
[1, 2].each do |a|
  puts a if a == 1
end

# good
[1, 2].each do |a|
  next unless a == 1
  puts a
end
```

With `AllowConsecutiveConditionals: true` (default `false`), a conditional
at the end of an iteration immediately preceded by another conditional at
the same depth is left alone:

```ruby
# good
[1, 2].each do |a|
  if a == 1
    puts a
  end
  if a == 2
    puts a
  end
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::WhileNode, NodeKind::UntilNode, NodeKind::ForNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("skip_modifier_ifs"),
                allowed: &["skip_modifier_ifs", "always"],
                doc: "Whether a modifier `if`/`unless` at the end of an iteration is exempt.",
            },
            ConfigOption {
                name: "MinBodyLength",
                default: ConfigDefault::Int(3),
                allowed: &[],
                doc: "Minimum number of lines a block-form `if`/`unless` body needs to trigger \
                      this cop.",
            },
            ConfigOption {
                name: "AllowConsecutiveConditionals",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Whether a conditional immediately preceded by another conditional at the \
                      same depth is exempt.",
            },
        ],
        blind_spots: "\
`MinBodyLength`'s upstream validation (must be a positive integer, else the \
cop raises) is not reproduced; a non-positive value is used as configured \
instead.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "always" => Style::Always,
            _ => Style::SkipModifierIfs,
        };
        Ok(Self {
            style,
            allow_consecutive_conditionals: options.bool("AllowConsecutiveConditionals"),
            min_body_length: options.int("MinBodyLength"),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => {
                let Some(call) = node.as_call_node() else { return };
                if !is_enumerator_method(call.name().as_slice()) {
                    return;
                }
                let Some(block_node) = call.block() else { return };
                let Some(block) = block_node.as_block_node() else { return };
                self.check(block.body().and_then(|b| b.as_statements_node()), ctx);
            }
            NodeKind::WhileNode => {
                let Some(n) = node.as_while_node() else { return };
                self.check(n.statements(), ctx);
            }
            NodeKind::UntilNode => {
                let Some(n) = node.as_until_node() else { return };
                self.check(n.statements(), ctx);
            }
            NodeKind::ForNode => {
                let Some(n) = node.as_for_node() else { return };
                self.check(n.statements(), ctx);
            }
            _ => {}
        }
    }
}
