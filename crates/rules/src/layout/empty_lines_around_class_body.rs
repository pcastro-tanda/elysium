//! `Layout/EmptyLinesAroundClassBody`, ported from RuboCop's
//! `lib/rubocop/cop/layout/empty_lines_around_class_body.rb` plus the
//! `EmptyLinesAroundBody` mixin (`lib/rubocop/cop/mixin/empty_lines_around_body.rb`)
//! it includes.
//!
//! The cop only hooks `on_class`/`on_sclass` -- a nested `module` is never
//! independently checked (that is `Layout/EmptyLinesAroundModuleBody`'s
//! job), even though the mixin's `constant_definition?`/`empty_line_required?`
//! matchers treat a `module` child the same as a `class` child when deciding
//! whether the *parent* is a bare namespace or whose first statement defers
//! a required blank line.
//!
//! Prism always wraps a multi-statement body in a [`NodeKind::StatementsNode`],
//! even when it holds exactly one statement -- unlike the whitequark AST
//! RuboCop's source is written against, where a single-statement body is
//! that statement node directly (no wrapping `begin` node). [`BodyShape`]
//! renormalizes this: a one-child `StatementsNode` unwraps to that child
//! (whitequark's non-`begin_type?` body), anything else keeps every child
//! (whitequark's `begin_type?` body).
//!
//! RuboCop's `add_offense` silently drops a second offense whose range is
//! byte-identical to one already reported by this cop in the same file (a
//! `Set` keyed on range). This happens in practice whenever a class body is
//! exactly one line long (the "beginning" and "ending" target line
//! coincide) under `EnforcedStyle: no_empty_lines`. Rather than
//! reimplementing that `Set`, this port relies on the engine's own
//! `(rule, span)`-exact dedup in `finish()`, which keeps the first-pushed
//! diagnostic for an identical span -- as long as offenses are pushed in
//! the same order RuboCop calls `add_offense` (beginning before ending,
//! `check_beginning` before `check_deferred_empty_line`), the outcome
//! matches.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    NoEmptyLines,
    EmptyLines,
    EmptyLinesExceptNamespace,
    EmptyLinesSpecial,
    BeginningOnly,
    EndingOnly,
}

/// The four "plain" styles `check_both` dispatches on directly -- distinct
/// from [`Style`] since `check_empty_lines_except_namespace`/
/// `check_empty_lines_special` also invoke it with a literal
/// `:no_empty_lines`/`:empty_lines`, independent of the configured style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BasicStyle {
    NoEmptyLines,
    EmptyLines,
    BeginningOnly,
    EndingOnly,
}

/// Whether one boundary (beginning or end) wants a blank line present
/// (`:empty_lines`) or forbidden (`:no_empty_lines`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Want {
    Empty,
    NoEmpty,
}

/// A normalized view of a class/singleton-class body, mirroring
/// whitequark's `begin_type?` split (see the module doc comment).
enum BodyShape<'pr> {
    Single(Node<'pr>),
    Multi(Vec<Node<'pr>>),
}

/// Looks for empty lines around the bodies of `class`/`class << self` and
/// matches them against the configured style.
#[derive(Debug, Clone)]
pub struct EmptyLinesAroundClassBody {
    style: Style,
}

impl Rule for EmptyLinesAroundClassBody {
    const META: RuleMeta = RuleMeta {
        name: "Layout/EmptyLinesAroundClassBody",
        department: Department::Layout,
        summary: "Keeps track of empty lines around class bodies.",
        explanation: "\
```ruby
# EnforcedStyle: no_empty_lines (default)

# good
class Foo
  def bar
    # ...
  end
end
```

```ruby
# EnforcedStyle: empty_lines

# good
class Foo

  def bar
    # ...
  end

end
```

```ruby
# EnforcedStyle: empty_lines_except_namespace

# good
class Foo
  class Bar

    # ...

  end
end
```

```ruby
# EnforcedStyle: empty_lines_special

# good
class Foo

  def bar; end

end
```

```ruby
# EnforcedStyle: beginning_only

# good
class Foo

  def bar
    # ...
  end
end
```

```ruby
# EnforcedStyle: ending_only

# good
class Foo
  def bar
    # ...
  end

end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ClassNode, NodeKind::SingletonClassNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("no_empty_lines"),
            allowed: &[
                "empty_lines",
                "empty_lines_except_namespace",
                "empty_lines_special",
                "no_empty_lines",
                "beginning_only",
                "ending_only",
            ],
            doc: "The blank-line convention required around a class body.",
        }],
        blind_spots: "\
`comment_line?` (used by the `empty_lines_special` deferred-blank-line
check to skip back over full-line comments) approximates RuboCop's
`/^\\s*#/` with a plain space/tab scan, not full Ruby `\\s` (which also
matches form feed and vertical tab); this only differs on lines using those
control characters as indentation, which does not occur in practice.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "empty_lines" => Style::EmptyLines,
            "empty_lines_except_namespace" => Style::EmptyLinesExceptNamespace,
            "empty_lines_special" => Style::EmptyLinesSpecial,
            "beginning_only" => Style::BeginningOnly,
            "ending_only" => Style::EndingOnly,
            _ => Style::NoEmptyLines,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::ClassNode { .. } => {
                let class = node.as_class_node().expect("kind matched");
                // RuboCop's `node.parent_class.last_line if node.parent_class`:
                // a superclass expression can itself span multiple lines
                // (e.g. `class Foo < Struct.new(\n  ...\n)`), in which case
                // the body's "first line" for placement purposes is the
                // superclass's own last line, not the `class` keyword's.
                let adjusted_first_line =
                    class.superclass().map(|sc| ctx.last_line(sc.location().span()));
                self.check(ctx, node.location().span(), class.body(), adjusted_first_line);
            }
            Node::SingletonClassNode { .. } => {
                let sclass = node.as_singleton_class_node().expect("kind matched");
                self.check(ctx, node.location().span(), sclass.body(), None);
            }
            _ => {}
        }
    }
}

impl EmptyLinesAroundClassBody {
    /// RuboCop's `EmptyLinesAroundBody#check`.
    fn check(
        &self,
        ctx: &mut Context<'_>,
        node_span: Span,
        body: Option<Node<'_>>,
        adjusted_first_line: Option<u32>,
    ) {
        // RuboCop's `valid_body_style?`: an empty body is never flagged
        // unless the style is `no_empty_lines` (which forbids a blank
        // line even inside an otherwise-empty body).
        if body.is_none() && self.style != Style::NoEmptyLines {
            return;
        }
        if ctx.is_single_line(node_span) {
            return;
        }

        let first_line = adjusted_first_line.unwrap_or_else(|| ctx.line_col(node_span.start).line);
        let last_line = ctx.last_line(node_span);

        match self.style {
            Style::EmptyLinesExceptNamespace => {
                let Some(body) = body else { return };
                let shape = shape_of(body);
                if is_namespace(&shape, true) {
                    check_both(ctx, BasicStyle::NoEmptyLines, first_line, last_line);
                } else {
                    check_both(ctx, BasicStyle::EmptyLines, first_line, last_line);
                }
            }
            Style::EmptyLinesSpecial => {
                let Some(body) = body else { return };
                let shape = shape_of(body);
                if is_namespace(&shape, true) {
                    check_both(ctx, BasicStyle::NoEmptyLines, first_line, last_line);
                } else {
                    if first_child_requires_empty_line(&shape) {
                        check_beginning(ctx, Want::Empty, first_line);
                    } else {
                        check_beginning(ctx, Want::NoEmpty, first_line);
                        check_deferred_empty_line(ctx, &shape);
                    }
                    check_ending(ctx, Want::Empty, last_line);
                }
            }
            Style::NoEmptyLines => {
                check_both(ctx, BasicStyle::NoEmptyLines, first_line, last_line);
            }
            Style::EmptyLines => {
                check_both(ctx, BasicStyle::EmptyLines, first_line, last_line);
            }
            Style::BeginningOnly => {
                check_both(ctx, BasicStyle::BeginningOnly, first_line, last_line);
            }
            Style::EndingOnly => {
                check_both(ctx, BasicStyle::EndingOnly, first_line, last_line);
            }
        }
    }
}

/// RuboCop's `check_both`.
fn check_both(ctx: &mut Context<'_>, style: BasicStyle, first_line: u32, last_line: u32) {
    match style {
        BasicStyle::BeginningOnly => {
            check_beginning(ctx, Want::Empty, first_line);
            check_ending(ctx, Want::NoEmpty, last_line);
        }
        BasicStyle::EndingOnly => {
            check_beginning(ctx, Want::NoEmpty, first_line);
            check_ending(ctx, Want::Empty, last_line);
        }
        BasicStyle::NoEmptyLines => {
            check_beginning(ctx, Want::NoEmpty, first_line);
            check_ending(ctx, Want::NoEmpty, last_line);
        }
        BasicStyle::EmptyLines => {
            check_beginning(ctx, Want::Empty, first_line);
            check_ending(ctx, Want::Empty, last_line);
        }
    }
}

/// RuboCop's `check_beginning`/`check_source`/`check_line` for the body's
/// first line: the target line is the one right after the class's own
/// first line (adjusted for a multi-line superclass, see `enter`).
fn check_beginning(ctx: &mut Context<'_>, want: Want, first_line: u32) {
    let target = first_line + 1;
    let blank = ctx.line_text(target).is_empty();
    match want {
        Want::NoEmpty if blank => report_extra(ctx, target, "beginning"),
        Want::Empty if !blank => report_missing(ctx, target, "beginning"),
        Want::NoEmpty | Want::Empty => {}
    }
}

/// RuboCop's `check_ending`/`check_source`/`check_line` for the body's last
/// line: the target line checked is the one right before `end`, but a
/// missing-blank-line offense is reported on the `end` line itself
/// (RuboCop's `check_line` offset quirk: `offset = 2` when the message
/// mentions `'end.'`).
fn check_ending(ctx: &mut Context<'_>, want: Want, last_line: u32) {
    let target = last_line - 1;
    let blank = ctx.line_text(target).is_empty();
    match want {
        Want::NoEmpty if blank => report_extra(ctx, target, "end"),
        Want::Empty if !blank => report_missing(ctx, last_line, "end"),
        Want::NoEmpty | Want::Empty => {}
    }
}

/// RuboCop's `check_deferred_empty_line`: when the body's first statement
/// doesn't itself require a leading blank line (a bare `def`/`class`/
/// `module`/access-modifier), but a *later* sibling does, a blank line is
/// still required directly before that sibling (skipping back over any
/// contiguous full-line comments), unless one is already there.
fn check_deferred_empty_line(ctx: &mut Context<'_>, shape: &BodyShape<'_>) {
    let Some(child) = first_empty_line_required_child(shape) else { return };
    let child_first_line = ctx.line_col(child.location().span().start).line;
    let prev_line = previous_line_ignoring_comments(ctx, child_first_line);
    if ctx.line_text(prev_line).is_empty() {
        return;
    }
    let target = prev_line + 1;
    let span = char_span(ctx, target);
    let msg = format!("Empty line missing before first {} definition", node_type_name(child));
    let fix = Fix {
        applicability: Applicability::Safe,
        edits: vec![Edit::insert(span.start, b"\n".as_slice())],
    };
    ctx.report_with_fix(&EmptyLinesAroundClassBody::META, span, msg, fix);
}

/// RuboCop's `previous_line_ignoring_comments`: the closest line at or
/// before `send_line - 1` that isn't a full-line comment, or line 1 if
/// every line up to the top of the file is a comment.
fn previous_line_ignoring_comments(ctx: &Context<'_>, send_line: u32) -> u32 {
    for candidate in (1..send_line.max(1)).rev() {
        if !ruby_source::is_comment_line(ctx.line_text(candidate)) {
            return candidate;
        }
    }
    1
}

/// An "Extra empty line detected" offense: `range` is the blank line's own
/// newline byte (a real 1-byte span whose end offset lands on the next
/// physical line, rendering as a zero-width caret), deleted to collapse
/// the blank line away.
fn report_extra(ctx: &mut Context<'_>, line: u32, desc: &str) {
    let span = char_span(ctx, line);
    let msg = format!("Extra empty line detected at class body {desc}.");
    let fix = Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(span)] };
    ctx.report_with_fix(&EmptyLinesAroundClassBody::META, span, msg, fix);
}

/// An "Empty line missing" offense: `range` is the target line's own first
/// character, with a newline inserted before it to add the required blank
/// line.
fn report_missing(ctx: &mut Context<'_>, line: u32, desc: &str) {
    let span = char_span(ctx, line);
    let msg = format!("Empty line missing at class body {desc}.");
    let fix = Fix {
        applicability: Applicability::Safe,
        edits: vec![Edit::insert(span.start, b"\n".as_slice())],
    };
    ctx.report_with_fix(&EmptyLinesAroundClassBody::META, span, msg, fix);
}

/// A real 1-byte span at the start of `line`, matching `Layout::EmptyLines`'
/// convention for a diagnostic-plus-edit range anchored on one line.
fn char_span(ctx: &Context<'_>, line: u32) -> Span {
    let start = ctx.line_span(line).start;
    Span::new(start, start + 1)
}

/// Renormalizes a Prism body (always a [`NodeKind::StatementsNode`] when
/// present, even for one statement) into whitequark's `begin_type?` split;
/// see the module doc comment.
fn shape_of(body: Node<'_>) -> BodyShape<'_> {
    if let Some(stmts) = body.as_statements_node() {
        let mut items: Vec<Node<'_>> = stmts.body().iter().collect();
        if items.len() == 1 {
            BodyShape::Single(items.pop().expect("len == 1"))
        } else {
            BodyShape::Multi(items)
        }
    } else {
        BodyShape::Single(body)
    }
}

/// RuboCop's `namespace?`.
fn is_namespace(shape: &BodyShape<'_>, with_one_child: bool) -> bool {
    match shape {
        BodyShape::Multi(children) => {
            if with_one_child {
                false
            } else {
                children.iter().all(is_constant_definition)
            }
        }
        BodyShape::Single(node) => is_constant_definition(node),
    }
}

/// RuboCop's `first_child_requires_empty_line?`.
fn first_child_requires_empty_line(shape: &BodyShape<'_>) -> bool {
    match shape {
        BodyShape::Multi(children) => children.first().is_some_and(is_empty_line_required),
        BodyShape::Single(node) => is_empty_line_required(node),
    }
}

/// RuboCop's `first_empty_line_required_child`.
fn first_empty_line_required_child<'a, 'pr>(shape: &'a BodyShape<'pr>) -> Option<&'a Node<'pr>> {
    match shape {
        BodyShape::Multi(children) => children.iter().find(|c| is_empty_line_required(c)),
        BodyShape::Single(node) => is_empty_line_required(node).then_some(node),
    }
}

/// RuboCop's `constant_definition?`: `{class module}`.
fn is_constant_definition(node: &Node<'_>) -> bool {
    matches!(node, Node::ClassNode { .. } | Node::ModuleNode { .. })
}

/// RuboCop's `empty_line_required?`:
/// `{any_def class module (send nil? {:private :protected :public})}`.
fn is_empty_line_required(node: &Node<'_>) -> bool {
    match node {
        Node::DefNode { .. } | Node::ClassNode { .. } | Node::ModuleNode { .. } => true,
        Node::CallNode { .. } => is_bare_access_modifier(node),
        _ => false,
    }
}

/// A receiver-less, argument-less call to `private`/`protected`/`public`
/// (RuboCop's inline `(send nil? {:private :protected :public})` pattern --
/// deliberately narrower than the general `bare_access_modifier?` helper,
/// which also allows `module_function`).
fn is_bare_access_modifier(node: &Node<'_>) -> bool {
    let call = node.as_call_node().expect("kind matched");
    if call.receiver().is_some() || call.arguments().is_some() {
        return false;
    }
    matches!(call.name().as_slice(), b"private" | b"protected" | b"public")
}

/// RuboCop's `node.type` as used by `deferred_message` -- whitequark's
/// `:def`/`:defs` split on whether the method has an explicit receiver
/// (`def self.foo`), plus `:class`/`:module`/`:send`.
fn node_type_name(node: &Node<'_>) -> &'static str {
    match node {
        Node::DefNode { .. } => {
            let def = node.as_def_node().expect("kind matched");
            if def.receiver().is_some() {
                "defs"
            } else {
                "def"
            }
        }
        Node::ClassNode { .. } => "class",
        Node::ModuleNode { .. } => "module",
        _ => "send",
    }
}
