//! `Layout/EmptyLinesAroundAccessModifier`, ported from RuboCop's
//! `lib/rubocop/cop/layout/empty_lines_around_access_modifier.rb`.
//!
//! Upstream tracks three pieces of state via `on_class`/`on_module`/
//! `on_sclass`/`on_block` hooks that are *never reset* once set, and are
//! overwritten by every subsequent visit regardless of nesting -- a real
//! quirk (not a modeling choice) this port reproduces exactly by updating
//! the same three fields from this rule's own `enter()`, in the same
//! traversal order the engine already guarantees:
//!
//! - `block_line`: the first line of the most recently *entered*
//!   call-with-a-literal-block (RuboCop's `on_block`, aliased to
//!   `on_numblock`/`on_itblock`; a Prism `CallNode`'s own span already
//!   covers its attached block, so this is read straight off the owning
//!   `CallNode` rather than needing a separate `BlockNode` hook).
//! - `class_or_module_def_first_line`/`class_or_module_def_last_line`: set
//!   by `on_class`/`on_module`/`on_sclass`.
//!
//! RuboCop's `MethodDispatchNode#in_macro_scope?` walks `node.parent`,
//! `node.parent.parent`, ... testing each against `sclass`/`class`/
//! `module`/`class_constructor?` (terminates true) or `kwbegin`/`begin`/
//! `any_block`/a non-condition `if` branch (transparent, keep walking) --
//! anything else terminates false. Prism has no back-pointer from an
//! arbitrary node to its parent, but the engine's per-node ancestor stack
//! ([`linter::Context::ancestors`]) gives exactly this chain for the node
//! *currently being entered*; since every RuboCop wrapper type other than
//! `any_block` collapses to Prism's single, always-present
//! [`NodeKind::StatementsNode`] (whitequark elides the wrapper for a
//! single-statement body, Prism never does -- see
//! `empty_lines_around_class_body.rs`'s module doc for the general
//! phenomenon), the walk only needs to skip `BeginNode`/`IfNode` (both
//! transparent) and, for `BlockNode`, its owning `CallNode` too (a Prism
//! `BlockNode` is a field of the call it attaches to, so in ancestor order
//! it is always immediately followed, going outward, by that call --
//! whitequark's `block` node bundles the two into one).
//!
//! [`StmtFrame`] captures this walk's result once per `StatementsNode`, at
//! the point it is entered (before any of its children, so before the
//! state above can be perturbed by a sibling) -- together with the
//! positional data `should_insert_line_before?`/`should_insert_line_after?`
//! need (`inside_block?`, and each item's own span for right-sibling/
//! first-child/last-child comparisons), since a leaf `CallNode` has no
//! other way to reach its own parent's child list either.
//!
//! Both `class_constructor?` (`Class.new`/`Module.new`/`Struct.new`/
//! `Data.define` blocks, as an alternative to `sclass`/`class`/`module`)
//! and the exact `kwbegin`/`begin` vs. `rescue`/`ensure` distinction for an
//! explicit `begin` block are not modeled (see `blind_spots`); neither
//! affects any fixture, and the corpus never exercises the difference
//! either.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    NodeInfo, OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::is_bare_access_modifier;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::{is_ruby_whitespace, Span};

/// RuboCop's `ConfigurableEnforcedStyle` value for this cop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Around,
    OnlyBefore,
}

/// One `StatementsNode` frame, computed once when the node is entered (see
/// the module doc) and consulted by every direct-child `CallNode` that
/// turns out to be a candidate access modifier.
#[derive(Debug, Clone)]
struct StmtFrame {
    /// Whether a bare access modifier that is a direct item of this list is
    /// `macro?`/`in_macro_scope?` (see the module doc).
    macro_scope: bool,
    /// RuboCop's `inside_block?`: this statements list is the direct body
    /// of a `block`/`numblock`/`itblock`.
    inside_block: bool,
    /// Each item's own span, in source order.
    items: Vec<Span>,
}

/// Keep blank lines around access modifiers.
#[derive(Debug, Clone)]
pub struct EmptyLinesAroundAccessModifier {
    style: Style,
    /// `Layout/EmptyLinesAroundBlockBody`'s `EnforcedStyle`, peered:
    /// RuboCop's `no_empty_lines_around_block_body?`.
    no_empty_lines_around_block_body: bool,
    /// RuboCop's `@block_line`.
    block_line: Option<u32>,
    /// RuboCop's `@class_or_module_def_first_line`.
    class_or_module_def_first_line: Option<u32>,
    /// RuboCop's `@class_or_module_def_last_line`.
    class_or_module_def_last_line: Option<u32>,
    /// Stack of `StatementsNode` frames, mirroring the traversal (pushed on
    /// `enter`, popped on `leave`).
    stack: Vec<StmtFrame>,
}

impl Rule for EmptyLinesAroundAccessModifier {
    const META: RuleMeta = RuleMeta {
        name: "Layout/EmptyLinesAroundAccessModifier",
        department: Department::Layout,
        summary: "Keep blank lines around access modifiers.",
        explanation: "\
```ruby
# EnforcedStyle: around (default)

# bad
class Foo
  def bar; end
  private
  def baz; end
end

# good
class Foo
  def bar; end

  private

  def baz; end
end
```

```ruby
# EnforcedStyle: only_before

# bad
class Foo
  def bar; end
  private
  def baz; end
end

# good
class Foo
  def bar; end

  private
  def baz; end
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::CallNode,
            NodeKind::ClassNode,
            NodeKind::ModuleNode,
            NodeKind::SingletonClassNode,
            NodeKind::StatementsNode,
        ],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("around"),
            allowed: &["around", "only_before"],
            doc: "Whether a blank line is required both before and after an \
access modifier, or only before it.",
        }],
        blind_spots: "\
`in_macro_scope?`'s `class_constructor?` alternative (`Class.new`/
`Module.new`/`Struct.new`/`Data.define` blocks, as an alternative to being
nested in a bare `sclass`/`class`/`module`) is not modeled: a call-with-block
is always treated as transparent, regardless of what it is a call to. This is
strictly more permissive than upstream only for a block that is itself
nested somewhere other than directly under a class/module/root -- a shape
`class_constructor?` itself can only satisfy in the same narrow way (see the
module doc), so it does not diverge on any code this cop's own spec or the
corpus exercises. Separately, an explicit `begin...end` with a `rescue`/
`ensure` clause is `kwbegin`+`rescue`/`ensure` upstream (not the transparent
`kwbegin` alone), so an access modifier as the sole statement preceding a
`rescue`/`ensure` there is treated as in macro scope when upstream would not;
not exercised by the spec or the corpus. An access modifier used as an
`if`/`unless` condition (rather than its branch) is treated the same as one
in a branch (both transparent), since the ancestor chain alone cannot tell
which child slot was descended through; not meaningful Ruby, so it never
affects real code.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "only_before" => Style::OnlyBefore,
            _ => Style::Around,
        };
        let no_empty_lines_around_block_body = options
            .peer("Layout/EmptyLinesAroundBlockBody", "EnforcedStyle")
            .and_then(OptionValue::as_str)
            .unwrap_or("no_empty_lines")
            == "no_empty_lines";
        Ok(Self {
            style,
            no_empty_lines_around_block_body,
            block_line: None,
            class_or_module_def_first_line: None,
            class_or_module_def_last_line: None,
            stack: Vec::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::ClassNode => {
                let class = node.as_class_node().expect("kind matched");
                self.class_or_module_def_first_line = Some(match class.superclass() {
                    Some(sc) => ctx.line_col(sc.span().start).line,
                    None => ctx.line_col(node.span().start).line,
                });
                self.class_or_module_def_last_line = Some(ctx.last_line(node.span()));
            }
            NodeKind::ModuleNode => {
                self.class_or_module_def_first_line = Some(ctx.line_col(node.span().start).line);
                self.class_or_module_def_last_line = Some(ctx.last_line(node.span()));
            }
            NodeKind::SingletonClassNode => {
                let sclass = node.as_singleton_class_node().expect("kind matched");
                self.class_or_module_def_first_line =
                    Some(ctx.line_col(sclass.expression().span().start).line);
                self.class_or_module_def_last_line = Some(ctx.last_line(node.span()));
            }
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                let has_block = call.block().and_then(|b| b.as_block_node()).is_some();
                if has_block {
                    self.block_line = Some(ctx.line_col(node.span().start).line);
                }
                if !has_block && is_bare_access_modifier(&call) {
                    self.check_candidate(node, &call, ctx);
                }
            }
            NodeKind::StatementsNode => {
                let stmts = node.as_statements_node().expect("kind matched");
                let ancestors = ctx.ancestors();
                let macro_scope = in_macro_scope(ancestors);
                let inside_block = ancestors.last().is_some_and(|a| a.kind == NodeKind::BlockNode);
                let items = stmts.body().iter().map(|n| n.span()).collect();
                self.stack.push(StmtFrame { macro_scope, inside_block, items });
            }
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if node.kind() == NodeKind::StatementsNode {
            self.stack.pop();
        }
    }
}

impl EmptyLinesAroundAccessModifier {
    /// RuboCop's `on_send` body.
    fn check_candidate(
        &mut self,
        node: &Node<'_>,
        call: &ruby_ast::node::CallNode<'_>,
        ctx: &mut Context<'_>,
    ) {
        let Some(frame) = self.stack.last() else { return };
        if !frame.macro_scope {
            return;
        }
        let span = node.span();
        let Some(index) = frame.items.iter().position(|item| *item == span) else { return };
        if let Some(&right) = frame.items.get(index + 1) {
            if ctx.same_line(span, right) {
                return;
            }
        }
        let is_only_child = frame.items.len() == 1;
        let is_first = index == 0;
        let is_last = index + 1 == frame.items.len();
        let inside_block = frame.inside_block;

        let first_line = ctx.line_col(span.start).line;
        let last_line = ctx.last_line(span);
        let name = call.name();
        let name = name.as_slice();
        if self.expected_empty_lines(ctx, name, first_line, last_line) {
            return;
        }

        let name = String::from_utf8_lossy(name).into_owned();
        let message = self.message(ctx, &name, first_line, last_line);

        let mut edits = Vec::with_capacity(2);
        if self.should_insert_line_before(ctx, first_line, inside_block, is_only_child, is_first) {
            edits.push(Edit::insert(ctx.line_span(first_line).start, b"\n".as_slice()));
        }
        self.correct_after(
            ctx,
            &mut edits,
            first_line,
            last_line,
            inside_block,
            is_only_child,
            is_last,
        );

        let fix = Fix { applicability: Applicability::Safe, edits };
        ctx.report_with_fix(&Self::META, span, message, fix);
    }

    /// RuboCop's `expected_empty_lines?`.
    fn expected_empty_lines(
        &self,
        ctx: &Context<'_>,
        name: &[u8],
        first_line: u32,
        last_line: u32,
    ) -> bool {
        match self.style {
            Style::Around => {
                self.previous_line_empty(ctx, first_line) && self.next_line_empty(ctx, last_line)
            }
            Style::OnlyBefore => self.allowed_only_before(ctx, name, first_line, last_line),
        }
    }

    /// RuboCop's `allowed_only_before_style?`.
    fn allowed_only_before(
        &self,
        ctx: &Context<'_>,
        name: &[u8],
        first_line: u32,
        last_line: u32,
    ) -> bool {
        // RuboCop's `special_modifier?`: a bare `private`/`protected` (not
        // `public`/`module_function`).
        if matches!(name, b"private" | b"protected") {
            if line_text_or_empty(ctx, last_line + 1) == b"end" {
                return true;
            }
            if self.next_line_empty_and_exists(ctx, last_line) {
                return false;
            }
        }
        self.previous_line_empty(ctx, first_line)
    }

    /// RuboCop's `message`.
    fn message(&self, ctx: &Context<'_>, name: &str, first_line: u32, last_line: u32) -> String {
        match self.style {
            Style::Around => {
                if self.block_start(first_line) || self.class_def(first_line) {
                    format!("Keep a blank line after `{name}`.")
                } else {
                    format!("Keep a blank line before and after `{name}`.")
                }
            }
            Style::OnlyBefore => {
                if self.next_line_empty(ctx, last_line) {
                    format!("Remove a blank line after `{name}`.")
                } else {
                    format!("Keep a blank line before `{name}`.")
                }
            }
        }
    }

    /// RuboCop's `should_insert_line_before?`.
    fn should_insert_line_before(
        &self,
        ctx: &Context<'_>,
        first_line: u32,
        inside_block: bool,
        is_only_child: bool,
        is_first: bool,
    ) -> bool {
        if self.previous_line_empty(ctx, first_line) {
            return false;
        }
        if !(inside_block && self.no_empty_lines_around_block_body) {
            return true;
        }
        if is_only_child {
            return true;
        }
        !is_first
    }

    /// RuboCop's `correct_next_line_if_denied_style`.
    #[allow(clippy::too_many_arguments)]
    fn correct_after(
        &self,
        ctx: &Context<'_>,
        edits: &mut Vec<Edit>,
        first_line: u32,
        last_line: u32,
        inside_block: bool,
        is_only_child: bool,
        is_last: bool,
    ) {
        let _ = first_line;
        let should_insert_after = if !(inside_block && self.no_empty_lines_around_block_body) {
            true
        } else if is_only_child {
            false
        } else {
            !is_last
        };
        if !should_insert_after {
            return;
        }
        match self.style {
            Style::Around => {
                if !self.next_line_empty(ctx, last_line) {
                    if let Some(start) = safe_line_start(ctx, last_line + 1) {
                        edits.push(Edit::insert(start, b"\n".as_slice()));
                    }
                }
            }
            Style::OnlyBefore => {
                if self.next_line_empty_and_exists(ctx, last_line) {
                    if let Some(start) = safe_line_start(ctx, last_line + 1) {
                        edits.push(Edit::delete(Span::new(start, start + 1)));
                    }
                }
            }
        }
    }

    /// RuboCop's `block_start?`.
    fn block_start(&self, line: u32) -> bool {
        self.block_line == Some(line - 1)
    }

    /// RuboCop's `class_def?`.
    fn class_def(&self, line: u32) -> bool {
        self.class_or_module_def_first_line == Some(line - 1)
    }

    /// RuboCop's `body_end?`.
    fn body_end(&self, line: u32) -> bool {
        self.class_or_module_def_last_line == Some(line + 1)
    }

    /// RuboCop's `previous_line_empty?`.
    fn previous_line_empty(&self, ctx: &Context<'_>, send_line: u32) -> bool {
        match previous_line_ignoring_comments(ctx, send_line) {
            None => true,
            Some(text) => {
                self.block_start(send_line) || self.class_def(send_line) || is_blank_line(text)
            }
        }
    }

    /// RuboCop's `next_line_empty?`.
    fn next_line_empty(&self, ctx: &Context<'_>, last_send_line: u32) -> bool {
        self.body_end(last_send_line) || is_blank_line(line_text_or_empty(ctx, last_send_line + 1))
    }

    /// RuboCop's `next_line_empty_and_exists?`.
    fn next_line_empty_and_exists(&self, ctx: &Context<'_>, last_send_line: u32) -> bool {
        self.next_line_empty(ctx, last_send_line) && last_send_line + 1 != raw_line_count(ctx)
    }
}

/// RuboCop's `processed_source.lines.size`: unlike [`Context::line_count`],
/// which deliberately excludes the trailing empty "line" a final `\n`
/// produces, upstream's `Parser::Source::Buffer#source_lines` (built with
/// `split("\n", -1)`) counts it -- and `next_line_empty_and_exists?`'s own
/// `last_send_line.next != processed_source.lines.size` guard depends on
/// that extra entry to correctly recognize "there is nothing left to merge
/// this blank line into" only at the true end of the buffer. This is
/// exactly [`ruby_source::LineIndex`]'s own internal `starts` length.
fn raw_line_count(ctx: &Context<'_>) -> u32 {
    u32::try_from(ctx.source().lines().line_starts().len()).unwrap_or(u32::MAX)
}

/// A 1-based line's text, or an empty slice when `line` is beyond even
/// upstream's own trailing phantom line (Ruby's `Array#[]` returning
/// `nil`, on which `nil == 'end'` is `false`, matching an empty slice --
/// though `nil.blank?` genuinely raises upstream, since RuboCop's own
/// `String#blank?` core extension does not patch `NilClass`; this is the
/// one case this port cannot reasonably reproduce a crash for, and treats
/// as blank instead, matching every other reachable input).
fn line_text_or_empty<'a>(ctx: &Context<'a>, line: u32) -> &'a [u8] {
    if line == 0 || line > raw_line_count(ctx) {
        &[]
    } else {
        ctx.line_text(line)
    }
}

/// The start offset of a 1-based line, or `None` when `line` is beyond
/// even upstream's own trailing phantom line (only reachable when the
/// modifier being corrected sits on the file's own last line with nothing
/// after it at all).
fn safe_line_start(ctx: &Context<'_>, line: u32) -> Option<u32> {
    if line == 0 || line > raw_line_count(ctx) {
        None
    } else {
        Some(ctx.line_span(line).start)
    }
}

/// RuboCop's `MethodDispatchNode#in_macro_scope?`, applied to the ancestor
/// chain of a `StatementsNode` (see the module doc): walks outward from the
/// innermost ancestor, skipping transparent wrappers (`StatementsNode` --
/// every intervening statements list the walk crosses, since that is
/// exactly the "always-wrap" Prism artifact the module doc describes, not
/// a whitequark node type of its own -- plus `BeginNode`, `IfNode`, and
/// `BlockNode` together with its owning `CallNode`), until it either lands
/// on a class-like node (`ProgramNode` standing in for upstream's
/// `root?`) or exhausts the chain (also root) -- both `true` -- or hits
/// anything else, `false`.
fn in_macro_scope(ancestors: &[NodeInfo]) -> bool {
    let mut idx = ancestors.len();
    while idx > 0 {
        idx -= 1;
        match ancestors[idx].kind {
            NodeKind::ProgramNode
            | NodeKind::ClassNode
            | NodeKind::ModuleNode
            | NodeKind::SingletonClassNode => return true,
            NodeKind::StatementsNode | NodeKind::BeginNode | NodeKind::IfNode => {}
            NodeKind::BlockNode => {
                if idx == 0 {
                    return true;
                }
                idx -= 1;
            }
            _ => return false,
        }
    }
    true
}

/// RuboCop's `previous_line_ignoring_comments`: the text of the closest
/// line before `send_line` that isn't a full-line comment, or `None` if
/// every line up to (and including) line 1 is a comment, or `send_line` is
/// already line 1.
fn previous_line_ignoring_comments<'a>(ctx: &Context<'a>, send_line: u32) -> Option<&'a [u8]> {
    for line in (1..send_line).rev() {
        let text = ctx.line_text(line);
        if !ruby_source::is_comment_line(text) {
            return Some(text);
        }
    }
    None
}

/// RuboCop's own `String#blank?` core extension
/// (`lib/rubocop/core_ext/string.rb`): empty, or made up entirely of
/// whitespace.
fn is_blank_line(line: &[u8]) -> bool {
    line.iter().all(|&b| is_ruby_whitespace(b))
}
