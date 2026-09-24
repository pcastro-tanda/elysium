//! `Style/AccessorGrouping`, ported from RuboCop's
//! `lib/rubocop/cop/style/accessor_grouping.rb` plus the `VisibilityHelp`
//! mixin it includes.
//!
//! Prism always wraps a class/module/singleton-class body that has at least
//! one statement in a `StatementsNode` (see `Layout/EmptyLinesAroundClassBody`'s
//! doc comment for the same observation), even when it holds exactly one
//! statement -- unlike whitequark's AST, which leaves a lone statement
//! unwrapped. RuboCop's `class_send_elements` special-cases that unwrapped
//! shape (`class_def.send_type?` / `class_def.def_type?`); here the direct
//! children of the body's `StatementsNode` are simply scanned uniformly for
//! `CallNode`s, no special-casing needed.
//!
//! Likewise, `groupable_accessor?`'s `block_type?` unwrap (peeling a Sorbet
//! `sig { ... }` statement open to reach the `sig` call it wraps) does not
//! apply either: Prism attaches a block directly to the `CallNode` it
//! belongs to (`CallNode::block`) instead of wrapping the call in a separate
//! block node, so a preceding `sig { ... }` statement already *is* the `sig`
//! `CallNode` itself -- its span already runs through the block's closing
//! `}`/`end`, which is exactly what the line-gap check needs.

use linter::{
    Applicability, CommentInfo, ConfigDefault, ConfigOption, Context, Department, Edit, Fix,
    FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Grouped,
    Separated,
}

/// Checks for grouping of accessors in `class`/`module`/`class << self` bodies.
#[derive(Debug, Clone)]
pub struct AccessorGrouping {
    style: Style,
}

impl Rule for AccessorGrouping {
    const META: RuleMeta = RuleMeta {
        name: "Style/AccessorGrouping",
        department: Department::Style,
        summary: "Checks for grouping of accessors in `class` and `module` bodies.",
        explanation: "\
By default it enforces accessors to be placed in grouped declarations, but it
can be configured to enforce separating them in multiple declarations.

If there is a method call before the accessor method it is always allowed,
as it might be intended, e.g. for a Sorbet `sig` block. If there is an
RBS::Inline annotation comment (`#: String`) just after the accessor method
it is always allowed too.

```ruby
# bad
class Foo
  attr_reader :bar
  attr_reader :baz
end

# good
class Foo
  attr_reader :bar, :baz
end
```

```ruby
# EnforcedStyle: separated
# bad
class Foo
  attr_reader :bar, :baz
end

# good
class Foo
  attr_reader :bar
  attr_reader :baz
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ClassNode, NodeKind::ModuleNode, NodeKind::SingletonClassNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("grouped"),
            allowed: &["grouped", "separated"],
            doc: "Whether accessors of the same kind should be grouped into one \
                  declaration, or each placed in its own.",
        }],
        blind_spots: "\
Comment attachment for `separated`'s per-argument comment carry-over is a
direct port of `Parser::Source::Comment::Associator`'s leading/decorating
rules specialised to a flat list of leaf arguments (symbols/splats): a
comment inside a compound argument expression (unlikely for an attr-list
element) is not independently re-attached to that argument's own
sub-expressions. `same_line?` between an RBS::Inline comment and the
preceding statement compares first lines only, matching every case this cop
can actually see (the preceding statement is always a single accessor call
or access-modifier/macro call).",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "separated" => Style::Separated,
            _ => Style::Grouped,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let body = match node {
            Node::ClassNode { .. } => node.as_class_node().expect("kind matched").body(),
            Node::ModuleNode { .. } => node.as_module_node().expect("kind matched").body(),
            Node::SingletonClassNode { .. } => {
                node.as_singleton_class_node().expect("kind matched").body()
            }
            _ => return,
        };
        let Some(stmts_node) = body.as_ref().and_then(Node::as_statements_node) else { return };
        let stmts: Vec<Node<'_>> = stmts_node.body().iter().collect();
        for i in 0..stmts.len() {
            let Some(call) = accessor_call(&stmts[i]) else { continue };
            self.check(ctx, &stmts, i, &call);
        }
    }
}

impl AccessorGrouping {
    /// RuboCop's `check`.
    fn check(&self, ctx: &mut Context<'_>, stmts: &[Node<'_>], i: usize, call: &CallNode<'_>) {
        if previous_line_comment(ctx, call) || !groupable_accessor(ctx, stmts, i) {
            return;
        }
        let offense = match self.style {
            Style::Grouped => groupable_sibling_accessors(ctx, stmts, i).len() > 1,
            Style::Separated => arg_count(call) > 1,
        };
        if !offense {
            return;
        }
        let message = match self.style {
            Style::Grouped => format!("Group together all `{}` attributes.", method_name(call)),
            Style::Separated => format!("Use one attribute per `{}`.", method_name(call)),
        };
        let fix = self.build_fix(ctx, stmts, i, call);
        ctx.report_with_fix(&Self::META, call.location().span(), message, fix);
    }

    /// RuboCop's `autocorrect`.
    fn build_fix(
        &self,
        ctx: &Context<'_>,
        stmts: &[Node<'_>],
        i: usize,
        call: &CallNode<'_>,
    ) -> Fix {
        match self.style {
            Style::Grouped => match preferred_accessors_grouped(ctx, stmts, i) {
                Some(text) => Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::replace(call.location().span(), text.into_bytes())],
                },
                None => Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::delete(span_with_leading_space_removed(
                        ctx,
                        call.location().span(),
                    ))],
                },
            },
            Style::Separated => {
                let (text, span) = separate_accessors(ctx, call);
                Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::replace(span, text.into_bytes())],
                }
            }
        }
    }
}

/// RuboCop's `SendNode#attribute_accessor?`: a receiver-less call to
/// `attr_reader`/`attr_writer`/`attr_accessor`/`attr` with at least one
/// argument.
fn accessor_call<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    let call = node.as_call_node()?;
    if is_attribute_accessor(&call) {
        Some(call)
    } else {
        None
    }
}

fn is_attribute_accessor(call: &CallNode<'_>) -> bool {
    call.receiver().is_none()
        && matches!(
            call.name().as_slice(),
            b"attr_reader" | b"attr_writer" | b"attr_accessor" | b"attr"
        )
        && arg_count(call) > 0
}

fn method_name(call: &CallNode<'_>) -> String {
    String::from_utf8_lossy(call.name().as_slice()).into_owned()
}

fn arg_count(call: &CallNode<'_>) -> usize {
    call.arguments().map_or(0, |a| a.arguments().len())
}

/// RuboCop's `VisibilityHelp#access_modifier?`, narrowed to what
/// `MethodDispatchNode#access_modifier?` actually needs here: a
/// receiver-less call to `public`/`protected`/`private`/`module_function`,
/// bare or with arguments (`private def foo; end`, `private :foo`).
fn is_access_modifier(call: &CallNode<'_>) -> bool {
    call.receiver().is_none()
        && matches!(
            call.name().as_slice(),
            b"public" | b"protected" | b"private" | b"module_function"
        )
}

/// RuboCop's `VisibilityHelp::VISIBILITY_SCOPES` bare-call matcher (`(send
/// nil? {:private :protected :public})`), used by `node_visibility` --
/// deliberately narrower than [`is_access_modifier`] (excludes
/// `module_function` and requires zero arguments).
fn is_visibility_block(call: &CallNode<'_>) -> bool {
    call.receiver().is_none()
        && arg_count(call) == 0
        && matches!(call.name().as_slice(), b"public" | b"protected" | b"private")
}

/// RuboCop's `VisibilityHelp#node_visibility`, restricted to the
/// `node_visibility_from_visibility_block` path: accessor macros are never
/// `def`s, so the `node_visibility_from_visibility_inline` path (which only
/// applies to `def_type?` nodes) never matches here.
fn node_visibility(stmts: &[Node<'_>], i: usize) -> &'static [u8] {
    for stmt in stmts[..i].iter().rev() {
        if let Some(call) = stmt.as_call_node() {
            if is_visibility_block(&call) {
                return match call.name().as_slice() {
                    b"private" => b"private",
                    b"protected" => b"protected",
                    _ => b"public",
                };
            }
        }
    }
    b"public"
}

/// RuboCop's `previous_line_comment?`.
fn previous_line_comment(ctx: &Context<'_>, call: &CallNode<'_>) -> bool {
    let line = ctx.line_col(call.location().span().start).line;
    if line < 2 {
        return false;
    }
    is_comment_line(ctx.line_text(line - 1))
}

/// RuboCop's `Util#comment_line?`: `/^\s*#/`.
fn is_comment_line(text: &[u8]) -> bool {
    text.iter().find(|b| !b.is_ascii_whitespace()).copied() == Some(b'#')
}

fn last_line(ctx: &Context<'_>, span: Span) -> u32 {
    ctx.line_col(span.end.saturating_sub(1).max(span.start)).line
}

/// Whether a comment starting with `` #: `` (an `RBS::Inline` annotation) sits on
/// `span`'s own line -- RuboCop's `same_line?(c, previous_expression) &&
/// c.text.start_with?('#:')` check in `groupable_accessor?`.
fn has_rbs_inline_comment(ctx: &Context<'_>, span: Span) -> bool {
    let line = last_line(ctx, span);
    ctx.comments().iter().any(|c| c.line == line && ctx.text(c.span).starts_with(b"#:"))
}

/// RuboCop's `groupable_accessor?`.
fn groupable_accessor(ctx: &Context<'_>, stmts: &[Node<'_>], i: usize) -> bool {
    let Some(prev) = i.checked_sub(1).map(|j| &stmts[j]) else { return true };
    let Some(prev_call) = prev.as_call_node() else { return true };
    if has_rbs_inline_comment(ctx, prev_call.location().span()) {
        return false;
    }
    if is_attribute_accessor(&prev_call) || is_access_modifier(&prev_call) {
        return true;
    }
    let this_call = stmts[i].as_call_node().expect("accessor is a call");
    let gap = ctx.line_col(this_call.location().span().start).line
        - last_line(ctx, prev_call.location().span());
    gap > 1
}

/// RuboCop's `groupable_sibling_accessor?`.
fn is_groupable_sibling(ctx: &Context<'_>, stmts: &[Node<'_>], i: usize, j: usize) -> bool {
    let Some(call_j) = accessor_call(&stmts[j]) else { return false };
    let call_i = stmts[i].as_call_node().expect("accessor is a call");
    if call_j.name().as_slice() != call_i.name().as_slice() {
        return false;
    }
    if node_visibility(stmts, j) != node_visibility(stmts, i) {
        return false;
    }
    groupable_accessor(ctx, stmts, j) && !previous_line_comment(ctx, &call_j)
}

/// RuboCop's `groupable_sibling_accessors`, in source order (including `i`
/// itself, since `send_node.parent.each_child_node(:send).select { ... }`
/// enumerates every matching sibling, `send_node` included).
fn groupable_sibling_accessors(ctx: &Context<'_>, stmts: &[Node<'_>], i: usize) -> Vec<usize> {
    (0..stmts.len()).filter(|&j| is_groupable_sibling(ctx, stmts, i, j)).collect()
}

/// RuboCop's `skip_for_grouping?`: group after constants.
fn skip_for_grouping(ctx: &Context<'_>, stmts: &[Node<'_>], i: usize) -> bool {
    let right = &stmts[i + 1..];
    let has_casgn = right
        .iter()
        .any(|n| matches!(n, Node::ConstantWriteNode { .. } | Node::ConstantPathWriteNode { .. }));
    if !has_casgn {
        return false;
    }
    (i + 1..stmts.len()).any(|j| {
        matches!(stmts[j], Node::CallNode { .. }) && is_groupable_sibling(ctx, stmts, i, j)
    })
}

/// RuboCop's `preferred_accessors` (grouped branch). `None` means "delete";
/// `Some` means "replace this node's own span with this text".
fn preferred_accessors_grouped(ctx: &Context<'_>, stmts: &[Node<'_>], i: usize) -> Option<String> {
    if skip_for_grouping(ctx, stmts, i) {
        return None;
    }
    let accessors = groupable_sibling_accessors(ctx, stmts, i);
    let first = *accessors.first().expect("i is groupable with itself");
    if i == first || skip_for_grouping(ctx, stmts, first) {
        Some(group_accessors(ctx, stmts, i, &accessors))
    } else {
        None
    }
}

/// RuboCop's `group_accessors`.
fn group_accessors(ctx: &Context<'_>, stmts: &[Node<'_>], i: usize, accessors: &[usize]) -> String {
    let call_i = stmts[i].as_call_node().expect("accessor is a call");
    let mut names: Vec<Vec<u8>> = Vec::new();
    for &j in accessors {
        let call_j = stmts[j].as_call_node().expect("accessor is a call");
        if let Some(args) = call_j.arguments() {
            for arg in &args.arguments() {
                let text = ctx.text(arg.span()).to_vec();
                if !names.contains(&text) {
                    names.push(text);
                }
            }
        }
    }
    let names: Vec<String> =
        names.into_iter().map(|b| String::from_utf8_lossy(&b).into_owned()).collect();
    format!("{} {}", method_name(&call_i), names.join(", "))
}

/// RuboCop's `range_with_surrounding_space(node.source_range, side: :left)`:
/// walks the span start left across the node's own leading indentation
/// (spaces/tabs), then across every immediately preceding newline (so a
/// deleted node's blank predecessor lines collapse away too).
fn span_with_leading_space_removed(ctx: &Context<'_>, span: Span) -> Span {
    let prefix = ctx.text(Span::new(0, span.start));
    let mut pos = prefix.len();
    while pos > 0 && matches!(prefix[pos - 1], b' ' | b'\t') {
        pos -= 1;
    }
    while pos > 0 && prefix[pos - 1] == b'\n' {
        pos -= 1;
    }
    Span::new(u32::try_from(pos).unwrap_or(0), span.end)
}

/// RuboCop's `separate_accessors` plus `range_with_trailing_argument_comment`:
/// returns the replacement text and the span it replaces (the call's own
/// span, extended to swallow a trailing comment on its last argument when
/// that comment is not already inside the call's own span, e.g. a
/// parenthesized call already includes it).
fn separate_accessors(ctx: &Context<'_>, call: &CallNode<'_>) -> (String, Span) {
    let args: Vec<Node<'_>> =
        call.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
    let comments = comments_for_args(ctx, &args);
    let call_span = call.location().span();
    let indent = " ".repeat(ctx.line_col(call_span.start).column as usize);
    let method = method_name(call);

    let first_arg_text = args.first().map(|a| ctx.text(a.span()));
    let mut lines: Vec<String> = Vec::new();
    for (k, arg) in args.iter().enumerate() {
        let mut arg_lines: Vec<String> = comments[k]
            .iter()
            .map(|s| String::from_utf8_lossy(ctx.text(*s)).into_owned())
            .collect();
        arg_lines.push(format!("{method} {}", String::from_utf8_lossy(ctx.text(arg.span()))));
        // RuboCop's `arg == node.first_argument`: whitequark `AST::Node#==`
        // is structural, not identity -- a later argument whose source
        // matches the first argument's (e.g. a repeated `:one`) is also
        // left unindented, exactly like the first argument itself.
        if k != 0 && Some(ctx.text(arg.span())) != first_arg_text {
            for line in &mut arg_lines {
                *line = format!("{indent}{line}");
            }
        }
        lines.extend(arg_lines);
    }

    let span = match comments.last().and_then(|c| c.last()) {
        Some(&last_comment) if last_comment.end > call_span.end => {
            Span::new(call_span.start, last_comment.end)
        }
        _ => call_span,
    };
    (lines.join("\n"), span)
}

/// A direct port of `Parser::Source::Comment::Associator`'s leading/
/// decorating rules (see the module doc comment), specialised to a flat
/// argument list: each argument gets every comment that ends at or before
/// its own start (leading), then every comment sharing its last line
/// (decorating), consumed once each in source order.
fn comments_for_args(ctx: &Context<'_>, args: &[Node<'_>]) -> Vec<Vec<Span>> {
    let comments: &[CommentInfo] = ctx.comments();
    let mut result = vec![Vec::new(); args.len()];
    let mut ci = 0;
    for (k, arg) in args.iter().enumerate() {
        let arg_span = arg.span();
        while ci < comments.len() && comments[ci].span.end <= arg_span.start {
            result[k].push(comments[ci].span);
            ci += 1;
        }
        let arg_last_line = last_line(ctx, arg_span);
        while ci < comments.len() && comments[ci].line == arg_last_line {
            result[k].push(comments[ci].span);
            ci += 1;
        }
    }
    result
}
