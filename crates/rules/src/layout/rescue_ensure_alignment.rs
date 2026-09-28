//! `Layout/RescueEnsureAlignment`, ported from RuboCop's
//! `lib/rubocop/cop/layout/rescue_ensure_alignment.rb` plus the
//! `EndKeywordAlignment` mixin it borrows `start_line_range` from.
//!
//! Prism has no single node standing in for whitequark's polymorphic
//! "ancestor" search (`kwbegin`/`any_def`/`class`/`module`/`sclass`/
//! `any_block`, then possibly substituted by an enclosing assignment or
//! access-modifier wrapper). Rather than re-walking the tree from each
//! `rescue`/`ensure`, this rule keeps its own manual ancestor stack
//! (`self.stack`), pushed and popped in `enter`/`leave` for exactly the node
//! kinds that can play one of those roles -- so by the time a `RescueNode`
//! or `EnsureNode` is visited, `self.stack` already holds every enclosing
//! candidate in real tree order, and the "nearest ancestor of these types"
//! search from upstream becomes a linear scan from the top.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::{CallNode, MultiWriteNode, SuperNode};
use ruby_ast::{ext, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// One entry of the manual ancestor stack, holding only owned data (spans,
/// primitives) since `Node<'_>` cannot outlive a single `enter`/`leave`
/// call.
#[derive(Debug, Clone, Copy)]
enum Anchor {
    /// An explicit `begin...end` (`kwbegin_type?`), keeping the `begin`
    /// keyword's own span (both node start and `alignment_source`'s
    /// `node.loc.begin`).
    KwBegin { begin_kw: Span },
    /// `DefNode` (covers both `def foo` and `def self.foo`).
    Def { start: u32, name_end: u32 },
    /// `ClassNode` or `ModuleNode`.
    ClassLike { start: u32, path_end: u32 },
    /// `SingletonClassNode` (`class << self`).
    SClass { start: u32, expr_end: u32 },
    /// A method dispatch (`CallNode`/`SuperNode`/`ForwardingSuperNode`).
    /// Matches whitequark's `any_block` ancestor type when it owns a
    /// `do...end`/`{}` block (`block_opening` is `Some`) -- in Prism the
    /// block is a *field* of the call, the reverse of whitequark's tree
    /// where the `:block` node wraps the (blockless) call, so the call
    /// itself, not the `BlockNode`, is the ancestor whose own real parent
    /// (`below` in the stack) matters for the assignment/access-modifier
    /// substitutions. Without a block, a `Call` is still kept around for
    /// those substitutions: an attribute-writer assignment
    /// (`obj.attr = ...`) or a `def`-wrapping access modifier
    /// (`private def foo; end`).
    Call {
        start: u32,
        end_excluding_block: u32,
        message: Option<Span>,
        dot: Option<Span>,
        is_access_modifier: bool,
        is_attr_assignment: bool,
        receiver_end: Option<u32>,
        block_opening: Option<Span>,
    },
    /// A plain variable/constant assignment target (`lvasgn`, `ivasgn`,
    /// `op_asgn`, ... in whitequark terms).
    Assign { start: u32, name_end: u32 },
    /// `MultiWriteNode` (`masgn`).
    MultiAssign { start: u32, end: u32 },
}

fn is_container(a: &Anchor) -> bool {
    matches!(
        a,
        Anchor::KwBegin { .. }
            | Anchor::Def { .. }
            | Anchor::ClassLike { .. }
            | Anchor::SClass { .. }
            | Anchor::Call { block_opening: Some(_), .. }
    )
}

/// `assignment_node`'s target position, if `a` is one of the assignment
/// shapes: the node's own start (for the `same_line?` check) and the end of
/// its "name" text (for `alignment_source`).
fn assign_target(a: Anchor) -> Option<(u32, u32)> {
    match a {
        Anchor::Assign { start, name_end } => Some((start, name_end)),
        Anchor::MultiAssign { start, end } => Some((start, end)),
        Anchor::Call { start, is_attr_assignment: true, receiver_end: Some(end), .. } => {
            Some((start, end))
        }
        _ => None,
    }
}

/// `access_modifier_node`: only meaningful when the entry below a `Def` is
/// a qualifying wrapper call.
fn access_modifier_start(below: Option<Anchor>) -> Option<u32> {
    match below {
        Some(Anchor::Call { start, is_access_modifier: true, .. }) => Some(start),
        _ => None,
    }
}

/// RuboCop's `access_modifier?` plus `ALTERNATIVE_ACCESS_MODIFIERS`,
/// restricted to the shape this cop actually needs: a receiver-less call
/// whose sole argument is the `DefNode` it wraps.
fn call_is_access_modifier(call: &CallNode<'_>) -> bool {
    if call.receiver().is_some() {
        return false;
    }
    let Some(args) = call.arguments() else { return false };
    let mut iter = args.arguments().iter();
    let Some(only) = iter.next() else { return false };
    if iter.next().is_some() || only.kind() != NodeKind::DefNode {
        return false;
    }
    matches!(
        call.name().as_slice(),
        b"private"
            | b"protected"
            | b"public"
            | b"module_function"
            | b"public_class_method"
            | b"private_class_method"
    )
}

/// `call`'s own span, excluding any attached block -- `ext`'s helper stops
/// at the call's message/args/closing paren; the `super(...)` analog stops
/// at its `rparen`/args/keyword instead, since `SuperNode` has no shared
/// mixin with `CallNode` in Prism.
fn super_end_excluding_block(sup: &SuperNode<'_>) -> u32 {
    if let Some(rparen) = sup.rparen_loc() {
        return rparen.span().end;
    }
    if let Some(args) = sup.arguments() {
        if let Some(last) = args.arguments().iter().last() {
            return last.span().end;
        }
    }
    sup.keyword_loc().span().end
}

fn multi_write_end(m: &MultiWriteNode<'_>) -> u32 {
    let mut end = m.as_node().span().start;
    for n in &m.lefts() {
        end = end.max(n.span().end);
    }
    if let Some(rest) = m.rest() {
        end = end.max(rest.span().end);
    }
    for n in &m.rights() {
        end = end.max(n.span().end);
    }
    end
}

/// `name_loc` (or, for a qualified constant path target, its final
/// segment's `name_loc`) of every write-node kind this cop tracks as a
/// possible `assignment_node`.
fn write_name_end(node: &Node<'_>) -> Option<u32> {
    macro_rules! nl {
        ($accessor:ident) => {
            node.$accessor().map(|n| n.name_loc().span().end)
        };
    }
    macro_rules! target_nl {
        ($accessor:ident) => {
            node.$accessor().map(|n| n.target().name_loc().span().end)
        };
    }
    match node.kind() {
        NodeKind::LocalVariableWriteNode => nl!(as_local_variable_write_node),
        NodeKind::LocalVariableAndWriteNode => nl!(as_local_variable_and_write_node),
        NodeKind::LocalVariableOrWriteNode => nl!(as_local_variable_or_write_node),
        NodeKind::LocalVariableOperatorWriteNode => nl!(as_local_variable_operator_write_node),
        NodeKind::InstanceVariableWriteNode => nl!(as_instance_variable_write_node),
        NodeKind::InstanceVariableAndWriteNode => nl!(as_instance_variable_and_write_node),
        NodeKind::InstanceVariableOrWriteNode => nl!(as_instance_variable_or_write_node),
        NodeKind::InstanceVariableOperatorWriteNode => {
            nl!(as_instance_variable_operator_write_node)
        }
        NodeKind::ClassVariableWriteNode => nl!(as_class_variable_write_node),
        NodeKind::ClassVariableAndWriteNode => nl!(as_class_variable_and_write_node),
        NodeKind::ClassVariableOrWriteNode => nl!(as_class_variable_or_write_node),
        NodeKind::ClassVariableOperatorWriteNode => nl!(as_class_variable_operator_write_node),
        NodeKind::GlobalVariableWriteNode => nl!(as_global_variable_write_node),
        NodeKind::GlobalVariableAndWriteNode => nl!(as_global_variable_and_write_node),
        NodeKind::GlobalVariableOrWriteNode => nl!(as_global_variable_or_write_node),
        NodeKind::GlobalVariableOperatorWriteNode => nl!(as_global_variable_operator_write_node),
        NodeKind::ConstantWriteNode => nl!(as_constant_write_node),
        NodeKind::ConstantAndWriteNode => nl!(as_constant_and_write_node),
        NodeKind::ConstantOrWriteNode => nl!(as_constant_or_write_node),
        NodeKind::ConstantOperatorWriteNode => nl!(as_constant_operator_write_node),
        NodeKind::ConstantPathWriteNode => target_nl!(as_constant_path_write_node),
        NodeKind::ConstantPathAndWriteNode => target_nl!(as_constant_path_and_write_node),
        NodeKind::ConstantPathOrWriteNode => target_nl!(as_constant_path_or_write_node),
        NodeKind::ConstantPathOperatorWriteNode => target_nl!(as_constant_path_operator_write_node),
        _ => None,
    }
}

/// Builds the [`Anchor`] for every non-`Rescue`/`Ensure` subscribed kind
/// (`Rule::enter`'s fallback arm), one push site per structural/call/
/// write-node kind.
fn anchor_for(node: &Node<'_>) -> Option<Anchor> {
    match node.kind() {
        NodeKind::BeginNode => {
            let begin_kw = node.as_begin_node()?.begin_keyword_loc()?;
            Some(Anchor::KwBegin { begin_kw: begin_kw.span() })
        }
        NodeKind::DefNode => {
            let d = node.as_def_node()?;
            Some(Anchor::Def { start: d.as_node().span().start, name_end: d.name_loc().span().end })
        }
        NodeKind::ClassNode => {
            let c = node.as_class_node()?;
            Some(Anchor::ClassLike {
                start: c.as_node().span().start,
                path_end: c.constant_path().span().end,
            })
        }
        NodeKind::ModuleNode => {
            let m = node.as_module_node()?;
            Some(Anchor::ClassLike {
                start: m.as_node().span().start,
                path_end: m.constant_path().span().end,
            })
        }
        NodeKind::SingletonClassNode => {
            let s = node.as_singleton_class_node()?;
            Some(Anchor::SClass {
                start: s.as_node().span().start,
                expr_end: s.expression().span().end,
            })
        }
        NodeKind::LambdaNode => {
            let l = node.as_lambda_node()?;
            Some(Anchor::Call {
                start: node.span().start,
                end_excluding_block: l.operator_loc().span().end,
                message: Some(l.operator_loc().span()),
                dot: None,
                is_access_modifier: false,
                is_attr_assignment: false,
                receiver_end: None,
                block_opening: Some(l.opening_loc().span()),
            })
        }
        NodeKind::CallNode => {
            let c = node.as_call_node()?;
            let block_opening =
                c.block().and_then(|b| b.as_block_node()).map(|b| b.opening_loc().span());
            Some(Anchor::Call {
                start: c.as_node().span().start,
                end_excluding_block: ext::call_span_excluding_block(&c).end,
                message: c.message_loc().map(|l| l.span()),
                dot: c.call_operator_loc().map(|l| l.span()),
                is_access_modifier: call_is_access_modifier(&c),
                is_attr_assignment: c.equal_loc().is_some(),
                receiver_end: c.receiver().map(|r| r.span().end),
                block_opening,
            })
        }
        NodeKind::SuperNode => {
            let s = node.as_super_node()?;
            let block_opening =
                s.block().and_then(|b| b.as_block_node()).map(|b| b.opening_loc().span());
            Some(Anchor::Call {
                start: s.as_node().span().start,
                end_excluding_block: super_end_excluding_block(&s),
                message: Some(s.keyword_loc().span()),
                dot: None,
                is_access_modifier: false,
                is_attr_assignment: false,
                receiver_end: None,
                block_opening,
            })
        }
        NodeKind::ForwardingSuperNode => {
            let s = node.as_forwarding_super_node()?;
            let start = node.span().start;
            let keyword_len = u32::try_from("super".len()).unwrap_or(5);
            Some(Anchor::Call {
                start,
                end_excluding_block: start + keyword_len,
                message: Some(Span::new(start, start + keyword_len)),
                dot: None,
                is_access_modifier: false,
                is_attr_assignment: false,
                receiver_end: None,
                block_opening: s.block().map(|b| b.opening_loc().span()),
            })
        }
        NodeKind::MultiWriteNode => {
            let m = node.as_multi_write_node()?;
            Some(Anchor::MultiAssign { start: m.as_node().span().start, end: multi_write_end(&m) })
        }
        _ => write_name_end(node)
            .map(|name_end| Anchor::Assign { start: node.span().start, name_end }),
    }
}

/// The subscribed kinds: every write-node variant plus the structural
/// containers, method dispatches, and the two target keywords.
const KINDS: &[NodeKind] = &[
    NodeKind::RescueNode,
    NodeKind::EnsureNode,
    NodeKind::BeginNode,
    NodeKind::DefNode,
    NodeKind::ClassNode,
    NodeKind::ModuleNode,
    NodeKind::SingletonClassNode,
    NodeKind::BlockNode,
    NodeKind::LambdaNode,
    NodeKind::CallNode,
    NodeKind::SuperNode,
    NodeKind::ForwardingSuperNode,
    NodeKind::MultiWriteNode,
    NodeKind::LocalVariableWriteNode,
    NodeKind::LocalVariableAndWriteNode,
    NodeKind::LocalVariableOrWriteNode,
    NodeKind::LocalVariableOperatorWriteNode,
    NodeKind::InstanceVariableWriteNode,
    NodeKind::InstanceVariableAndWriteNode,
    NodeKind::InstanceVariableOrWriteNode,
    NodeKind::InstanceVariableOperatorWriteNode,
    NodeKind::ClassVariableWriteNode,
    NodeKind::ClassVariableAndWriteNode,
    NodeKind::ClassVariableOrWriteNode,
    NodeKind::ClassVariableOperatorWriteNode,
    NodeKind::GlobalVariableWriteNode,
    NodeKind::GlobalVariableAndWriteNode,
    NodeKind::GlobalVariableOrWriteNode,
    NodeKind::GlobalVariableOperatorWriteNode,
    NodeKind::ConstantWriteNode,
    NodeKind::ConstantAndWriteNode,
    NodeKind::ConstantOrWriteNode,
    NodeKind::ConstantOperatorWriteNode,
    NodeKind::ConstantPathWriteNode,
    NodeKind::ConstantPathAndWriteNode,
    NodeKind::ConstantPathOrWriteNode,
    NodeKind::ConstantPathOperatorWriteNode,
];

/// Checks whether the `rescue`/`ensure` keywords are aligned with the
/// construct they belong to, ported from RuboCop's `RescueEnsureAlignment`
/// cop plus the `EndKeywordAlignment` mixin's `start_line_range`.
#[derive(Debug, Clone, Default)]
pub struct RescueEnsureAlignment {
    stack: Vec<Anchor>,
    /// `Layout/BeginEndAlignment`'s `EnforcedStyleAlignWith`, read once per
    /// file: `'start_of_line'` replaces the alignment target's own start
    /// with the trimmed start of the line it begins on (`start_line_range`
    /// in `EndKeywordAlignment`), overriding every other alignment rule
    /// below, including the block-specific one.
    align_with_start_of_line: bool,
}

impl Rule for RescueEnsureAlignment {
    const META: RuleMeta = RuleMeta {
        name: "Layout/RescueEnsureAlignment",
        department: Department::Layout,
        summary: "Align rescues and ensures correctly.",
        explanation: "Checks whether the rescue and ensure keywords are aligned \
properly.\n\n```ruby\n# bad\nbegin\n  something\n  rescue\n  puts 'error'\nend\n\n# good\nbegin\n  something\nrescue\n  puts 'error'\nend\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: KINDS,
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let begin_end_enabled = options
            .peer("Layout/BeginEndAlignment", "Enabled")
            .and_then(linter::OptionValue::as_bool)
            != Some(false);
        let align_with_start_of_line = begin_end_enabled
            && options
                .peer("Layout/BeginEndAlignment", "EnforcedStyleAlignWith")
                .and_then(linter::OptionValue::as_str)
                == Some("start_of_line");
        Ok(Self { align_with_start_of_line, ..Self::default() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::RescueNode => {
                if let Some(r) = node.as_rescue_node() {
                    self.check(ctx, r.keyword_loc().span(), "rescue");
                }
            }
            NodeKind::EnsureNode => {
                if let Some(e) = node.as_ensure_node() {
                    self.check(ctx, e.ensure_keyword_loc().span(), "ensure");
                }
            }
            _ => {
                if let Some(anchor) = anchor_for(node) {
                    self.stack.push(anchor);
                }
            }
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        let pops = match node.kind() {
            NodeKind::BeginNode => {
                node.as_begin_node().is_some_and(|b| b.begin_keyword_loc().is_some())
            }
            NodeKind::DefNode
            | NodeKind::ClassNode
            | NodeKind::ModuleNode
            | NodeKind::SingletonClassNode
            | NodeKind::LambdaNode
            | NodeKind::CallNode
            | NodeKind::SuperNode
            | NodeKind::ForwardingSuperNode
            | NodeKind::MultiWriteNode => true,
            _ => write_name_end(node).is_some(),
        };
        if pops {
            self.stack.pop();
        }
    }
}

impl RescueEnsureAlignment {
    /// RuboCop's `check`: finds the alignment target for a `rescue`/
    /// `ensure` keyword and reports an offense unless it is already
    /// aligned (or deliberately exempted, e.g. a line-broken method chain).
    fn check(&self, ctx: &mut Context<'_>, keyword: Span, keyword_str: &str) {
        let Some(idx) = self.stack.iter().rposition(is_container) else { return };
        let container = self.stack[idx];
        let below = if idx == 0 { None } else { Some(self.stack[idx - 1]) };

        let (start, end) = match container {
            Anchor::KwBegin { begin_kw } => (begin_kw.start, begin_kw.end),
            Anchor::Call {
                start: call_start,
                end_excluding_block,
                message,
                dot,
                block_opening: Some(opening),
                ..
            } => {
                let do_line = ctx.line_col(opening.start).line;
                let kw_col = ctx.line_col(keyword.start).column;
                if aligned_with_line_break_method(ctx, do_line, kw_col, message, dot) {
                    return;
                }
                match below.and_then(assign_target) {
                    Some((a_start, a_end))
                        if ctx.line_col(a_start).line == ctx.line_col(call_start).line =>
                    {
                        (a_start, a_end)
                    }
                    _ => block_default(ctx, end_excluding_block, opening),
                }
            }
            Anchor::Def { start, name_end } => match below.and_then(assign_target) {
                Some((a_start, a_end))
                    if ctx.line_col(a_start).line == ctx.line_col(start).line =>
                {
                    (a_start, a_end)
                }
                _ => match access_modifier_start(below) {
                    Some(call_start) => (call_start, name_end),
                    None => (start, name_end),
                },
            },
            Anchor::ClassLike { start, path_end } => match below.and_then(assign_target) {
                Some((a_start, a_end))
                    if ctx.line_col(a_start).line == ctx.line_col(start).line =>
                {
                    (a_start, a_end)
                }
                _ => (start, path_end),
            },
            Anchor::SClass { start, expr_end } => match below.and_then(assign_target) {
                Some((a_start, a_end))
                    if ctx.line_col(a_start).line == ctx.line_col(start).line =>
                {
                    (a_start, a_end)
                }
                _ => (start, expr_end),
            },
            Anchor::Call { .. } | Anchor::Assign { .. } | Anchor::MultiAssign { .. } => return,
        };

        let start =
            if self.align_with_start_of_line { start_line_range_start(ctx, start) } else { start };
        let begin_lc = ctx.line_col(start);
        let kw_lc = ctx.line_col(keyword.start);
        if begin_lc.column == kw_lc.column || begin_lc.line == kw_lc.line {
            return;
        }
        let beginning = String::from_utf8_lossy(ctx.text(Span::new(start, end))).into_owned();
        let msg = format!(
            "`{keyword_str}` at {}, {} is not aligned with `{beginning}` at {}, {}.",
            kw_lc.line, kw_lc.column, begin_lc.line, begin_lc.column
        );
        ctx.report(&Self::META, keyword, msg);
    }
}

/// RuboCop's `aligned_with_line_break_method?`/`aligned_with_leading_dot?`:
/// a `rescue`/`ensure` that lines up with the leading dot or the selector
/// of a line-broken method chain is exempt, even if it does not line up
/// with the block's own alignment target.
fn aligned_with_line_break_method(
    ctx: &Context<'_>,
    do_line: u32,
    rescue_col: u32,
    message: Option<Span>,
    dot: Option<Span>,
) -> bool {
    if let Some(dot) = dot {
        let lc = ctx.line_col(dot.start);
        if lc.line == do_line && lc.column == rescue_col {
            return true;
        }
    }
    match message {
        Some(message) => {
            let lc = ctx.line_col(message.start);
            lc.line == do_line && lc.column == rescue_col
        }
        None => false,
    }
}

/// RuboCop's `alignment_location`'s `any_block_type?` branch: the first
/// non-whitespace character of the owning call's own last line, through
/// the end of the block's opening keyword (`do`/`{`).
fn block_default(ctx: &Context<'_>, call_end_excluding_block: u32, opening: Span) -> (u32, u32) {
    let last_byte = call_end_excluding_block.saturating_sub(1);
    let line = ctx.line_col(last_byte).line;
    let line_span = ctx.line_span(line);
    let text = ctx.text(line_span);
    let first_non_ws =
        u32::try_from(text.iter().position(|&b| b != b' ' && b != b'\t').unwrap_or(0)).unwrap_or(0);
    (line_span.start + first_non_ws, opening.end)
}

/// `EndKeywordAlignment#start_line_range`'s start bound: the first
/// non-whitespace character of the line containing `offset`.
fn start_line_range_start(ctx: &Context<'_>, offset: u32) -> u32 {
    let line = ctx.line_col(offset).line;
    let line_span = ctx.line_span(line);
    let text = ctx.text(line_span);
    let first_non_ws =
        u32::try_from(text.iter().position(|&b| b != b' ' && b != b'\t').unwrap_or(0)).unwrap_or(0);
    line_span.start + first_non_ws
}
