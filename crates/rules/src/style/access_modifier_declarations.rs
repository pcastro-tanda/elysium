//! `Style/AccessModifierDeclarations`, ported from RuboCop's
//! `lib/rubocop/cop/style/access_modifier_declarations.rb`.
//!
//! Prism always wraps a body that has at least one statement in a
//! `StatementsNode`, even when it holds exactly one -- unlike whitequark's
//! AST, which leaves a lone statement unwrapped and instead makes its
//! *container* (`class`, `module`, `def`, a block, an `if`/`unless`
//! modifier, or nothing at all for the whole program) the node's `parent`.
//! This rule hooks every `StatementsNode` directly and, only when it holds
//! exactly one statement, recovers that elided whitequark parent by looking
//! at the `StatementsNode`'s own immediate parent kind: `BlockNode` (the
//! `any_block` case upstream's `allowed?` checks), `IfNode`/`UnlessNode`
//! (upstream's `if_type?` check, which covers both since whitequark models
//! `unless` as a reversed `if`), `ProgramNode` (upstream's `parent.nil?`,
//! the whole program is a single statement), or anything else (a real,
//! non-elidable parent). A hash-literal value (`bar(key: private)`) never
//! reaches this rule at all: it is never an element of any
//! `StatementsNode.body`, matching upstream's `parent.type?(:pair)` skip
//! for free. When a `StatementsNode` holds more than one statement it is
//! whitequark's `begin`/`kwbegin`, never elided, and is scanned directly for
//! sibling `def`s and other access-modifier calls.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CallNode, DefNode};
use ruby_ast::{ext, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `GROUP_STYLE_MESSAGE`/`INLINE_STYLE_MESSAGE`.
fn message(style: Style, name: &str) -> String {
    match style {
        Style::Group => format!("`{name}` should not be inlined in method definitions."),
        Style::Inline => format!("`{name}` should be inlined in method definitions."),
    }
}

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Group,
    Inline,
}

/// The whitequark parent an elided single-statement `StatementsNode` stands
/// in for. `Other` covers both a real non-nil, non-`if` whitequark parent
/// (`class`/`module`/`def` bodies, etc.) and a genuine multi-statement
/// `begin`, since neither ever needs special-casing below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EffParent {
    /// The whole program is this one statement: whitequark's `parent.nil?`.
    Root,
    /// Elided into an `if`/`unless` modifier.
    If,
    /// A real parent, or a genuine (non-elided) multi-statement `begin`.
    Other,
}

/// Checks style of how access modifiers are used.
#[derive(Debug, Clone)]
pub struct AccessModifierDeclarations {
    style: Style,
    allow_symbols: bool,
    allow_attrs: bool,
    allow_alias_method: bool,
}

impl Rule for AccessModifierDeclarations {
    const META: RuleMeta = RuleMeta {
        name: "Style/AccessModifierDeclarations",
        department: Department::Style,
        summary: "Checks style of how access modifiers are used.",
        explanation: "\
Access modifiers should be declared to apply to a group of methods or inline
before each method, depending on configuration. `EnforcedStyle` covers only
method definitions; applications of visibility methods to symbols can be
controlled using `AllowModifiersOnSymbols`, and the visibility of `attr*`
methods using `AllowModifiersOnAttrs`.

@safety
Autocorrection is not safe, because the visibility of dynamically defined
methods can vary depending on the state determined by the group access
modifier.

```ruby
# EnforcedStyle: group (default)
# bad
class Foo
  private def bar; end
  private def baz; end
end

# good
class Foo
  private

  def bar; end
  def baz; end
end
```

```ruby
# EnforcedStyle: inline
# bad
class Foo
  private

  def bar; end
  def baz; end
end

# good
class Foo
  private def bar; end
  private def baz; end
end
```

```ruby
# AllowModifiersOnSymbols: true (default)
# good
class Foo
  private :bar, :baz
  private *%i[qux quux]
end
```

```ruby
# AllowModifiersOnAttrs: true (default)
# good
class Foo
  private attr_accessor :qux
end
```

```ruby
# AllowModifiersOnAliasMethod: true (default)
# good
class Foo
  private alias_method :qux, :foo
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::StatementsNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("group"),
                allowed: &["inline", "group"],
                doc: "Whether access modifiers should be inlined before each method, or \
                      applied to a group of methods.",
            },
            ConfigOption {
                name: "AllowModifiersOnSymbols",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Allow access modifiers to be used with a symbol (`private :foo`).",
            },
            ConfigOption {
                name: "AllowModifiersOnAttrs",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Allow access modifiers to be used inline with an `attr*` method.",
            },
            ConfigOption {
                name: "AllowModifiersOnAliasMethod",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Allow access modifiers to be used inline with `alias_method`.",
            },
        ],
        blind_spots: "\
`ast_with_comments`'s comment association is approximated by a line-based\n\
leading-comment walk (each contiguous comment-only line directly above a\n\
node), rather than a real token-position-aware associator; this only ever\n\
under- or over-includes comments in the deleted range of a correction (never\n\
in the inserted text, which only ever pulls the access-modifier call's own\n\
leading comments), and overlapping deletions are merged, so it cannot change\n\
a correction's output. RuboCop's own auto-style-detection (inferring\n\
`EnforcedStyle` from a mix of correct/incorrect usages when unconfigured) is\n\
not implemented; `EnforcedStyle` must be set explicitly.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "inline" => Style::Inline,
            _ => Style::Group,
        };
        Ok(Self {
            style,
            allow_symbols: options.bool("AllowModifiersOnSymbols"),
            allow_attrs: options.bool("AllowModifiersOnAttrs"),
            allow_alias_method: options.bool("AllowModifiersOnAliasMethod"),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(stmts_node) = node.as_statements_node() else { return };
        let stmts: Vec<Node<'_>> = stmts_node.body().iter().collect();
        let solo_parent_kind = if stmts.len() == 1 { ctx.parent().map(|p| p.kind) } else { None };
        for i in 0..stmts.len() {
            let Some(call) = stmts[i].as_call_node() else { continue };
            if !is_access_modifier(&call) {
                continue;
            }
            self.check(ctx, &stmts, i, &call, stmts.len() == 1, solo_parent_kind);
        }
    }
}

impl AccessModifierDeclarations {
    /// RuboCop's `on_send`.
    fn check(
        &self,
        ctx: &mut Context<'_>,
        stmts: &[Node<'_>],
        i: usize,
        call: &CallNode<'_>,
        is_solo: bool,
        solo_parent_kind: Option<NodeKind>,
    ) {
        if self.allowed(call, is_solo, solo_parent_kind) {
            return;
        }
        let eff_parent = if is_solo {
            match solo_parent_kind {
                None | Some(NodeKind::ProgramNode) => EffParent::Root,
                Some(NodeKind::IfNode | NodeKind::UnlessNode) => EffParent::If,
                _ => EffParent::Other,
            }
        } else {
            EffParent::Other
        };
        if !self.offense(call, stmts, i, eff_parent) {
            return;
        }

        let name = String::from_utf8_lossy(call.name().as_slice()).into_owned();
        let span = call.message_loc().map_or(call_span(call), |l| l.span());
        let fix = match self.style {
            Style::Group => autocorrect_group(ctx, stmts, call),
            Style::Inline => autocorrect_inline(ctx, stmts, i, call),
        };
        if fix.edits.is_empty() {
            ctx.report(&Self::META, span, message(self.style, &name));
        } else {
            ctx.report_with_fix(&Self::META, span, message(self.style, &name), fix);
        }
    }

    /// RuboCop's `allowed?`.
    fn allowed(
        &self,
        call: &CallNode<'_>,
        is_solo: bool,
        solo_parent_kind: Option<NodeKind>,
    ) -> bool {
        if is_solo && solo_parent_kind == Some(NodeKind::BlockNode) {
            return true;
        }
        (self.allow_symbols && access_modifier_with_symbol(call))
            || (self.allow_attrs && access_modifier_with_attr(call))
            || (self.allow_alias_method && access_modifier_with_alias_method(call))
    }

    /// RuboCop's `offense?`.
    fn offense(
        &self,
        call: &CallNode<'_>,
        stmts: &[Node<'_>],
        i: usize,
        eff_parent: EffParent,
    ) -> bool {
        match self.style {
            Style::Group => {
                let skip = match eff_parent {
                    EffParent::If => true,
                    EffParent::Root => access_modifier_with_symbol(call),
                    EffParent::Other => false,
                };
                if skip {
                    return false;
                }
                access_modifier_is_inlined(call)
                    && !self.right_siblings_same_inline_method(call, stmts, i)
            }
            Style::Inline => {
                !access_modifier_is_inlined(call) && !select_grouped_def_nodes(stmts, i).is_empty()
            }
        }
    }

    /// RuboCop's `right_siblings_same_inline_method?`.
    fn right_siblings_same_inline_method(
        &self,
        call: &CallNode<'_>,
        stmts: &[Node<'_>],
        i: usize,
    ) -> bool {
        stmts[i + 1..].iter().enumerate().any(|(offset, sibling)| {
            let Some(sib_call) = sibling.as_call_node() else { return false };
            let j = i + 1 + offset;
            self.correctable_group_offense(&sib_call, stmts, j)
                && sib_call.name().as_slice() == call.name().as_slice()
                && sib_call.arguments().is_some_and(|a| !a.arguments().is_empty())
                && !find_corresponding_def_nodes(&sib_call, stmts).is_empty()
        })
    }

    /// RuboCop's `correctable_group_offense?`, restricted to the group-style
    /// sibling scan that calls it (`self.style` is always `Group` there).
    fn correctable_group_offense(&self, call: &CallNode<'_>, stmts: &[Node<'_>], i: usize) -> bool {
        if !is_access_modifier(call) || self.allowed(call, false, None) {
            return false;
        }
        let _ = (stmts, i);
        access_modifier_is_inlined(call) && !find_corresponding_def_nodes(call, stmts).is_empty()
    }
}

/// `rubocop-ast`'s `MethodDispatchNode#access_modifier?`: a receiver-less
/// call to `private`/`protected`/`public`/`module_function`, bare or with
/// arguments.
fn is_access_modifier(call: &CallNode<'_>) -> bool {
    call.receiver().is_none()
        && matches!(
            call.name().as_slice(),
            b"private" | b"protected" | b"public" | b"module_function"
        )
}

/// RuboCop's `access_modifier_is_inlined?`.
fn access_modifier_is_inlined(call: &CallNode<'_>) -> bool {
    call.arguments().is_some_and(|a| !a.arguments().is_empty())
}

fn arg_list<'pr>(call: &CallNode<'pr>) -> Vec<Node<'pr>> {
    call.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default()
}

/// RuboCop's `access_modifier_with_symbol?` node matcher: every argument is
/// a symbol literal, or the sole argument is a splat of a `%i[]` array
/// literal, a constant, or a method call.
fn access_modifier_with_symbol(call: &CallNode<'_>) -> bool {
    let args = arg_list(call);
    if args.is_empty() {
        return false;
    }
    if args.iter().all(|a| a.as_symbol_node().is_some()) {
        return true;
    }
    if args.len() == 1 {
        if let Some(expr) = args[0].as_splat_node().and_then(|s| s.expression()) {
            return is_percent_symbol_array(&expr)
                || matches!(expr.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode)
                || expr.as_call_node().is_some();
        }
    }
    false
}

/// RuboCop's `percent_symbol_array?`: an array literal opened with `%i(`/`%I(`.
fn is_percent_symbol_array(node: &Node<'_>) -> bool {
    node.as_array_node().is_some_and(|arr| {
        arr.opening_loc().is_some_and(|l| {
            let s = l.as_slice();
            s.starts_with(b"%i") || s.starts_with(b"%I")
        })
    })
}

/// RuboCop's `access_modifier_with_attr?` node matcher: the sole argument is
/// a receiver-less `attr`/`attr_reader`/`attr_writer`/`attr_accessor` call
/// with at least one argument of its own.
fn access_modifier_with_attr(call: &CallNode<'_>) -> bool {
    let args = arg_list(call);
    if args.len() != 1 {
        return false;
    }
    let Some(inner) = args[0].as_call_node() else { return false };
    inner.receiver().is_none()
        && matches!(
            inner.name().as_slice(),
            b"attr" | b"attr_reader" | b"attr_writer" | b"attr_accessor"
        )
        && inner.arguments().is_some_and(|a| !a.arguments().is_empty())
}

/// RuboCop's `access_modifier_with_alias_method?` node matcher: the sole
/// argument is a receiver-less two-argument `alias_method` call.
fn access_modifier_with_alias_method(call: &CallNode<'_>) -> bool {
    let args = arg_list(call);
    if args.len() != 1 {
        return false;
    }
    let Some(inner) = args[0].as_call_node() else { return false };
    inner.receiver().is_none()
        && inner.name().as_slice() == b"alias_method"
        && inner.arguments().is_some_and(|a| a.arguments().len() == 2)
}

/// RuboCop's `select_grouped_def_nodes`.
fn select_grouped_def_nodes<'pr>(stmts: &[Node<'pr>], i: usize) -> Vec<Node<'pr>> {
    let mut out = Vec::new();
    for n in &stmts[i + 1..] {
        if n.as_call_node().is_some_and(|c| ext::is_bare_access_modifier(&c)) {
            break;
        }
        if n.as_def_node().is_some() {
            out.push(*n);
        }
    }
    out
}

/// RuboCop's `find_corresponding_def_nodes`. `stmts` is the whole enclosing
/// statement list (not just `call`'s right siblings): a symbol-style
/// modifier's matching `def`s may come before it.
fn find_corresponding_def_nodes<'pr>(call: &CallNode<'pr>, stmts: &[Node<'pr>]) -> Vec<Node<'pr>> {
    if access_modifier_with_symbol(call) {
        let names: Vec<Vec<u8>> = arg_list(call)
            .iter()
            .filter_map(|a| a.as_symbol_node().map(|s| s.unescaped().to_vec()))
            .collect();
        let defs: Vec<Node<'pr>> = stmts
            .iter()
            .filter(|n| {
                n.as_def_node().is_some_and(|d: DefNode<'pr>| {
                    names.iter().any(|n| n.as_slice() == d.name().as_slice())
                })
            })
            .copied()
            .collect();
        if defs.len() == names.len() {
            defs
        } else {
            Vec::new()
        }
    } else {
        call.arguments().and_then(|a| a.arguments().iter().next()).into_iter().collect()
    }
}

/// A `CallNode`'s own span. `NodeExt::span` is only implemented for the
/// generic `Node` enum, not Prism's typed node structs.
fn call_span(call: &CallNode<'_>) -> Span {
    call.location().span()
}

/// RuboCop's `find_argument_less_modifier_node`: a sibling bare call to the
/// same method, anywhere in `stmts`.
fn find_argument_less_modifier_node(call: &CallNode<'_>, stmts: &[Node<'_>]) -> Option<Span> {
    stmts.iter().find_map(|n| {
        n.as_call_node()
            .filter(|c| {
                c.name().as_slice() == call.name().as_slice()
                    && c.arguments().is_none_or(|a| a.arguments().is_empty())
            })
            .map(|c| call_span(&c))
    })
}

/// RuboCop's (Comment) `ast_with_comments`, restricted to what this rule
/// needs: the contiguous run of comment-only lines directly above `span`.
fn leading_comments(ctx: &Context<'_>, span: Span) -> Vec<Span> {
    let mut out = Vec::new();
    let mut line = ctx.line_col(span.start).line;
    while line >= 2 {
        let prev = line - 1;
        let Some(comment) = ctx.comments().iter().find(|c| c.line == prev) else { break };
        out.push(comment.span);
        line = prev;
    }
    out.reverse();
    out
}

/// RuboCop's `range_with_comments_and_lines`.
fn removal_span(ctx: &Context<'_>, span: Span) -> Span {
    let leading = leading_comments(ctx, span);
    let start = leading.first().map_or(span.start, |c| c.start);
    ctx.whole_lines(Span::new(start, span.end))
}

/// Merges overlapping/touching spans, so nested removals (a `def` inside
/// the call that is also being removed) never produce overlapping edits.
fn merge_spans(mut spans: Vec<Span>) -> Vec<Span> {
    spans.sort_by_key(|s| (s.start, s.end));
    let mut out: Vec<Span> = Vec::with_capacity(spans.len());
    for span in spans {
        match out.last_mut() {
            Some(last) if span.start <= last.end => last.end = last.end.max(span.end),
            _ => out.push(span),
        }
    }
    out
}

/// RuboCop's `def_source`.
fn def_source(ctx: &Context<'_>, call: &CallNode<'_>, def_nodes: &[Node<'_>]) -> Vec<u8> {
    let mut parts: Vec<&[u8]> =
        leading_comments(ctx, call_span(call)).iter().map(|s| ctx.text(*s)).collect();
    parts.extend(def_nodes.iter().map(|d| ctx.text(d.span())));
    parts.join(&b"\n"[..])
}

/// RuboCop's `node.each_ancestor(:class, :module).first.loc.end`, as a byte
/// offset: a `class`/`module` node's span always ends immediately after its
/// closing `end` keyword.
fn nearest_class_or_module_end(ctx: &Context<'_>) -> Option<u32> {
    ctx.ancestors()
        .iter()
        .rev()
        .find(|a| matches!(a.kind, NodeKind::ClassNode | NodeKind::ModuleNode))
        .map(|a| a.span.end - 3)
}

/// RuboCop's `autocorrect_group_style`/`replace_defs`.
fn autocorrect_group(ctx: &Context<'_>, stmts: &[Node<'_>], call: &CallNode<'_>) -> Fix {
    let def_nodes = find_corresponding_def_nodes(call, stmts);
    if def_nodes.is_empty() {
        return Fix { applicability: Applicability::Unsafe, edits: Vec::new() };
    }
    let method_name = call.name().as_slice().to_vec();
    let source = def_source(ctx, call, &def_nodes);
    let mut edits = Vec::new();

    if let Some(bare_span) = find_argument_less_modifier_node(call, stmts) {
        let mut text = b"\n\n".to_vec();
        text.extend_from_slice(&source);
        edits.push(Edit::insert(bare_span.end, text));
    } else if let Some(end_offset) = nearest_class_or_module_end(ctx) {
        let mut text = method_name.clone();
        text.extend_from_slice(b"\n\n");
        text.extend_from_slice(&source);
        text.push(b'\n');
        edits.push(Edit::insert(end_offset, text));
    } else {
        let mut text = method_name;
        text.extend_from_slice(b"\n\n");
        text.extend_from_slice(&source);
        edits.push(Edit::replace(call_span(call), text));
        return Fix { applicability: Applicability::Unsafe, edits };
    }

    let mut remove: Vec<Span> = def_nodes.iter().map(|d| removal_span(ctx, d.span())).collect();
    remove.push(removal_span(ctx, call_span(call)));
    for span in merge_spans(remove) {
        edits.push(Edit::delete(span));
    }
    Fix { applicability: Applicability::Unsafe, edits }
}

/// RuboCop's `autocorrect_inline_style`.
fn autocorrect_inline(ctx: &Context<'_>, stmts: &[Node<'_>], i: usize, call: &CallNode<'_>) -> Fix {
    let mut edits = Vec::new();
    if stmts.len() > 1 {
        let next_start = stmts[i + 1].span().start;
        edits.push(Edit::delete(Span::new(call_span(call).start, next_start)));
    } else {
        edits.push(Edit::delete(removal_span(ctx, call_span(call))));
    }
    let method_name = call.name().as_slice();
    for def_node in select_grouped_def_nodes(stmts, i) {
        let mut text = method_name.to_vec();
        text.push(b' ');
        edits.push(Edit::insert(def_node.span().start, text));
    }
    Fix { applicability: Applicability::Unsafe, edits }
}
