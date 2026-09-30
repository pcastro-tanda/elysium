//! `Layout/DefEndAlignment`, ported from RuboCop's
//! `lib/rubocop/cop/layout/def_end_alignment.rb` plus the
//! `EndKeywordAlignment` mixin it includes.

use std::collections::HashSet;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Fix, FixAvailability,
    NodeInfo, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use super::end_keyword_alignment::{align_end, end_is_aligned, misalignment_message, uses_tabs};

/// RuboCop's `EnforcedStyleAlignWith`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    /// Align `end` with the start of the line the `def` keyword is on.
    StartOfLine,
    /// Align `end` with the `def` keyword.
    Def,
}

/// Align ends corresponding to defs correctly.
#[derive(Debug, Clone)]
pub struct DefEndAlignment {
    style: Style,
    tabs: bool,
    /// RuboCop's `ignore_node`: `def`s whose `end` a wrapping
    /// `private def ...` call has already accounted for.
    ignored: HashSet<u32>,
}

impl Rule for DefEndAlignment {
    const META: RuleMeta = RuleMeta {
        name: "Layout/DefEndAlignment",
        department: Department::Layout,
        summary: "Align ends corresponding to defs correctly.",
        explanation: "\
```ruby
# bad
private def foo
            end

# good (start_of_line)
private def foo
end

# good (def)
private def foo
        end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode, NodeKind::CallNode],
        config: &[ConfigOption {
            name: "EnforcedStyleAlignWith",
            default: ConfigDefault::Str("start_of_line"),
            allowed: &["start_of_line", "def"],
            doc: "Whether `end` lines up with the start of the line the `def` keyword is on \
                  (`start_of_line`, so with a `private`/`public` modifier when there is one) \
                  or with the `def` keyword itself (`def`).",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyleAlignWith")? {
            "def" => Style::Def,
            _ => Style::StartOfLine,
        };
        Ok(Self { style, tabs: uses_tabs(options), ignored: HashSet::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::DefNode { .. } => {
                let def = node.as_def_node().expect("kind matched");
                let keyword = Span::new(node.span().start, node.span().start + 3);
                let Some(end) = def.end_keyword_loc() else { return };
                self.check(ctx, node.span(), keyword, end.span(), ctx.parent());
            }
            Node::CallNode { .. } => {
                let call = node.as_call_node().expect("kind matched");
                if call.is_safe_navigation() {
                    return;
                }
                if def_modifier(&call).is_none() {
                    return;
                }
                let Some(method_def) = first_def(node) else { return };
                let def = method_def.as_def_node().expect("kind matched");
                let def_span = method_def.span();
                let def_keyword = Span::new(def_span.start, def_span.start + 3);
                let Some(end) = def.end_keyword_loc() else {
                    self.ignored.insert(def_span.start);
                    return;
                };
                let align_with = match self.style {
                    Style::Def => def_keyword,
                    Style::StartOfLine => Span::new(node.span().start, def_keyword.end),
                };
                self.check_alignment(ctx, def_span, end.span(), align_with, Some(node.span()));
                self.ignored.insert(def_span.start);
            }
            _ => {}
        }
    }
}

impl DefEndAlignment {
    /// RuboCop's `EndKeywordAlignment#check_end_kw_in_node`: the alignment
    /// target is the `def` keyword whatever the style, since a bare `def`
    /// starts its own line anyway.
    fn check(
        &mut self,
        ctx: &mut Context<'_>,
        def_span: Span,
        keyword: Span,
        end_loc: Span,
        parent: Option<NodeInfo>,
    ) {
        let modifier = parent.and_then(|p| {
            // Prism wraps a call's arguments in an `ArgumentsNode`; RuboCop's
            // `node.parent` for `private def foo` is the call itself.
            if p.kind == NodeKind::ArgumentsNode {
                ctx.ancestors().len().checked_sub(2).and_then(|i| ctx.ancestors().get(i)).copied()
            } else {
                Some(p)
            }
        });
        let modifier = modifier.filter(|p| p.kind == NodeKind::CallNode).map(|p| p.span);
        self.check_alignment(ctx, def_span, end_loc, keyword, modifier);
    }

    /// RuboCop's `check_end_kw_alignment` plus `autocorrect`.
    fn check_alignment(
        &mut self,
        ctx: &mut Context<'_>,
        def_span: Span,
        end_loc: Span,
        align_with: Span,
        send_parent: Option<Span>,
    ) {
        if self.ignored.contains(&def_span.start) {
            return;
        }
        if end_is_aligned(ctx, align_with, end_loc) {
            return;
        }
        let message = misalignment_message(ctx, end_loc, align_with);
        let target = match (self.style, send_parent) {
            (Style::StartOfLine, Some(parent)) => parent,
            _ => def_span,
        };
        let column = ctx.line_col(target.start).column;
        match align_end(ctx, end_loc, column, self.tabs) {
            Some(edit) => ctx.report_with_fix(
                &Self::META,
                end_loc,
                message,
                Fix { applicability: Applicability::Safe, edits: vec![edit] },
            ),
            None => ctx.report(&Self::META, end_loc, message),
        }
    }
}

/// `rubocop-ast`'s `MethodDispatchNode#def_modifier`: the `def` this
/// receiver-less call is a modifier for, following a chain of further
/// receiver-less calls (`private foo def bar; end`).
fn def_modifier<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    if call.receiver().is_some() {
        return None;
    }
    let arg = call.arguments()?.arguments().iter().next()?;
    match arg {
        Node::DefNode { .. } => Some(arg),
        Node::CallNode { .. } => {
            let inner = arg.as_call_node().expect("kind matched");
            // Whitequark wraps a call carrying a literal block in a `block`
            // node, which is not a `send` and so ends the chain.
            if inner.is_safe_navigation() || has_literal_block(&inner) {
                return None;
            }
            def_modifier(&inner)
        }
        _ => None,
    }
}

/// Whether `call` carries a `do`/`{}` block (as opposed to a `&blk`
/// argument, which Prism also reports through `block`).
fn has_literal_block(call: &CallNode<'_>) -> bool {
    matches!(call.block(), Some(Node::BlockNode { .. }))
}

/// RuboCop's `node.each_descendant(:any_def).first`, pre-order.
fn first_def<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    let mut found: Option<Node<'pr>> = None;
    for_each_child(node, |child| {
        if found.is_some() {
            return;
        }
        if let Node::DefNode { .. } = child {
            found = Some(*child);
        } else if let Some(inner) = first_def(child) {
            found = Some(inner);
        }
    });
    found
}
