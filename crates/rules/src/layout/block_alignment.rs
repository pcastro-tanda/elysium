//! `Layout/BlockAlignment`, ported from RuboCop's
//! `lib/rubocop/cop/layout/block_alignment.rb`.
//!
//! Upstream walks `[node, *node.ancestors]` of a whitequark `block` node.
//! Prism's tree differs in three ways that this port has to undo before the
//! walk means the same thing:
//!
//! * whitequark wraps a call and its literal block in one `block` node whose
//!   source range covers both; in Prism the `BlockNode` is a *field* of the
//!   call, and it is the call's own span that covers both. So the check runs
//!   when entering the *owner* (`CallNode`/`SuperNode`/`ForwardingSuperNode`,
//!   or a `LambdaNode`, which whitequark also models as a `block`), and that
//!   owner is the head of the lineage.
//! * Prism inserts nodes whitequark has no counterpart for -- `ProgramNode`,
//!   `ArgumentsNode`, the `BlockNode` itself, and a `StatementsNode` around
//!   every single-statement body. Those are *transparent*: they are dropped
//!   from the lineage so that a `def`/`send`/assignment parent is still seen
//!   as the immediate parent, exactly as upstream sees it. A `StatementsNode`
//!   holding two or more statements is whitequark's `begin`, so it stays.
//! * a call that owns a block is whitequark's `block` node, which never
//!   matches `block_end_align_target?`; a call without one is a `send`, which
//!   can. Safe navigation is `csend` upstream and so never matches either.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `EnforcedStyleAlignWith`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    /// `end` may align with either anchor.
    Either,
    /// `end` aligns with the start of the line the `do`/`{` is on.
    StartOfBlock,
    /// `end` aligns with the start of the line the expression started on.
    StartOfLine,
}

/// The facts about an ancestor that [`linter::NodeInfo`] (kind plus span)
/// cannot supply. Pushed on `enter` and popped on `leave` for exactly the
/// subscribed kinds, tagged with the depth the node sits at so it can be
/// matched back to its `ancestors()` slot.
#[derive(Debug, Clone, Copy)]
enum Detail {
    /// A call (or lambda) that owns a literal block: whitequark's `block`.
    BlockOwner,
    /// A method dispatch without a literal block: whitequark's `send` or,
    /// when safe navigation is used, `csend`.
    Send {
        /// The method is `<<`.
        shovel: bool,
        /// The method is `[]`.
        index: bool,
        /// The dispatch carries an `=`, so `MethodDispatchNode#setter_method?`
        /// -- and with it the bare `assignment?` alternative -- holds.
        setter: bool,
        /// Safe navigation: `csend`, which the `(send ...)` alternatives of
        /// the pattern do not match.
        csend: bool,
        /// Span of the receiver, for `equal?(%1)`.
        receiver: Option<Span>,
    },
    /// A real upstream node that can never be an alignment target parent.
    Opaque,
    /// A Prism-only wrapper with no upstream counterpart.
    Transparent,
    /// `op_asgn` or `masgn`, carrying the `lhs` `find_lhs_node` unwraps to.
    Lhs {
        /// Span of the assignment's left-hand side.
        span: Span,
        /// True for `masgn` (never `disqualified_parent?`).
        masgn: bool,
    },
}

/// One entry of the reconstructed whitequark lineage.
#[derive(Debug, Clone, Copy)]
struct Link {
    /// The node's source range.
    span: Span,
    /// Which arm of `block_end_align_target?` the node can satisfy.
    logical: Logical,
}

/// The node types `block_end_align_target?` distinguishes.
#[derive(Debug, Clone, Copy)]
enum Logical {
    /// `block` / `numblock` / `itblock`.
    Block,
    /// `send` or `csend`.
    Send {
        /// The method is `<<`.
        shovel: bool,
        /// The method is `[]`.
        index: bool,
        /// The dispatch carries an `=` (`setter_method?`).
        setter: bool,
        /// Safe navigation.
        csend: bool,
        /// Span of the receiver.
        receiver: Option<Span>,
    },
    /// `assignment?` other than `op_asgn`/`masgn`.
    Assignment,
    /// `op_asgn`, carrying its `lhs`.
    OpAsgn {
        /// Span of the assignment target.
        lhs: Span,
    },
    /// `masgn`, carrying its `mlhs`.
    Masgn {
        /// Span of the multiple-assignment target list.
        lhs: Span,
    },
    /// `def` / `defs`.
    Def,
    /// `splat`.
    Splat,
    /// `and` / `or`.
    AndOr,
    /// Anything else.
    Other,
}

impl Logical {
    /// `find_lhs_node`'s single unwrapping step.
    fn lhs(self) -> Option<Span> {
        match self {
            Self::OpAsgn { lhs } | Self::Masgn { lhs } => Some(lhs),
            _ => None,
        }
    }
}

/// The spans the check needs from the block itself.
#[derive(Debug, Clone, Copy)]
struct Parts {
    /// `node.loc.begin`: the `do` or `{`.
    do_loc: Span,
    /// `node.loc.end`: the `end` or `}`.
    end_loc: Span,
    /// `node.send_node.selector || node.send_node.source_range`.
    selector: Span,
}

/// Node kinds the rule subscribes to: the four block owners, plus every kind
/// whose [`Detail`] cannot be derived from kind and span alone.
const KINDS: &[NodeKind] = &[
    NodeKind::CallNode,
    NodeKind::SuperNode,
    NodeKind::ForwardingSuperNode,
    NodeKind::LambdaNode,
    NodeKind::StatementsNode,
    NodeKind::MultiWriteNode,
    NodeKind::LocalVariableOperatorWriteNode,
    NodeKind::InstanceVariableOperatorWriteNode,
    NodeKind::ClassVariableOperatorWriteNode,
    NodeKind::GlobalVariableOperatorWriteNode,
    NodeKind::ConstantOperatorWriteNode,
    NodeKind::ConstantPathOperatorWriteNode,
    NodeKind::CallOperatorWriteNode,
    NodeKind::IndexOperatorWriteNode,
];

/// True when `pos` falls inside `span` (start-inclusive, end-exclusive).
fn contains(span: Span, pos: u32) -> bool {
    span.start <= pos && pos < span.end
}

/// Ruby's `\s`, as it applies within a single line.
fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | 0x0b | 0x0c)
}

/// `/\S.*/.match(line)`: the column of the line's first non-space character
/// and the span running from there to the end of the line. `None` for a
/// blank line, which upstream would blow up on.
fn first_non_space(ctx: &Context<'_>, line: u32) -> Option<(u32, Span)> {
    let span = ctx.line_span(line);
    let index = ctx.line_text(line).iter().position(|byte| !is_space(*byte))?;
    // Only spaces and tabs can precede it, so the byte index is also the
    // character column.
    let column = u32::try_from(index).ok()?;
    Some((column, Span::new(span.start + column, span.end)))
}

/// The first line of `span`'s source, as `loc.source.lines.to_a.first.chomp`
/// produces it for an offense message.
fn first_line_of(ctx: &Context<'_>, span: Span) -> String {
    let line = ctx.line_col(span.start).line;
    let end = span.end.min(ctx.line_span(line).end);
    String::from_utf8_lossy(ctx.text(Span::new(span.start, end))).into_owned()
}

/// `format_source_line_column`.
fn format_source_line_column(source: &str, line: u32, column: u32) -> String {
    format!("`{source}` at {line}, {column}")
}

/// The block's `do`/`{`, `end`/`}` and its dispatch selector, for each of the
/// node shapes whitequark models as a `block`.
fn parts(node: &Node<'_>) -> Option<Parts> {
    match node.kind() {
        NodeKind::CallNode => {
            let call = node.as_call_node()?;
            let block = call.block()?.as_block_node()?;
            let selector = call
                .message_loc()
                .map_or_else(|| ruby_ast::ext::call_span_excluding_block(&call), |l| l.span());
            Some(Parts {
                do_loc: block.opening_loc().span(),
                end_loc: block.closing_loc().span(),
                selector,
            })
        }
        NodeKind::SuperNode => {
            let sup = node.as_super_node()?;
            let block = sup.block()?.as_block_node()?;
            Some(Parts {
                do_loc: block.opening_loc().span(),
                end_loc: block.closing_loc().span(),
                selector: sup.keyword_loc().span(),
            })
        }
        NodeKind::ForwardingSuperNode => {
            let sup = node.as_forwarding_super_node()?;
            let block = sup.block()?;
            Some(Parts {
                do_loc: block.opening_loc().span(),
                end_loc: block.closing_loc().span(),
                selector: node.span(),
            })
        }
        NodeKind::LambdaNode => {
            let lambda = node.as_lambda_node()?;
            Some(Parts {
                do_loc: lambda.opening_loc().span(),
                end_loc: lambda.closing_loc().span(),
                selector: lambda.operator_loc().span(),
            })
        }
        _ => None,
    }
}

/// True when any of the `(args ...)` children of a block's parameter list
/// covers `pos`; `numblock`/`itblock` parameters are not arguments upstream.
fn block_params_contain(params: Option<Node<'_>>, pos: u32) -> bool {
    let Some(params) = params else { return false };
    let Some(block_params) = params.as_block_parameters_node() else { return false };
    for local in &block_params.locals() {
        if contains(local.span(), pos) {
            return true;
        }
    }
    let Some(list) = block_params.parameters() else { return false };
    for group in [list.requireds(), list.optionals(), list.posts(), list.keywords()] {
        for param in &group {
            if contains(param.span(), pos) {
                return true;
            }
        }
    }
    [list.rest(), list.keyword_rest(), list.block().map(|b| b.as_node())]
        .into_iter()
        .flatten()
        .any(|param| contains(param.span(), pos))
}

/// `(node.send_node.arguments + node.arguments).any? { ... }`.
fn begins_inside_argument(node: &Node<'_>, pos: u32) -> bool {
    let (call_args, block_params) = match node.kind() {
        NodeKind::CallNode => {
            let Some(call) = node.as_call_node() else { return false };
            let params = call.block().and_then(|b| b.as_block_node()).and_then(|b| b.parameters());
            (call.arguments(), params)
        }
        NodeKind::SuperNode => {
            let Some(sup) = node.as_super_node() else { return false };
            let params = sup.block().and_then(|b| b.as_block_node()).and_then(|b| b.parameters());
            (sup.arguments(), params)
        }
        NodeKind::ForwardingSuperNode => {
            let Some(sup) = node.as_forwarding_super_node() else { return false };
            (None, sup.block().and_then(|b| b.parameters()))
        }
        NodeKind::LambdaNode => {
            let Some(lambda) = node.as_lambda_node() else { return false };
            (None, lambda.parameters())
        }
        _ => return false,
    };
    if let Some(args) = call_args {
        for arg in &args.arguments() {
            if contains(arg.span(), pos) {
                return true;
            }
        }
    }
    block_params_contain(block_params, pos)
}

/// RuboCop's `inside_parentheses?`: more `(`/`[` than `)`/`]` between the
/// start of the block expression and `pos`.
///
/// There is no token stream here, so this counts raw bytes outside
/// [`Context::opaque_spans`] instead.
fn inside_parentheses(ctx: &Context<'_>, from: u32, pos: u32) -> bool {
    if pos <= from {
        return false;
    }
    let mut depth: i32 = 0;
    let bytes = ctx.text(Span::new(from, pos));
    for (index, byte) in bytes.iter().enumerate() {
        if !matches!(byte, b'(' | b'[' | b')' | b']') {
            continue;
        }
        let offset = from + u32::try_from(index).unwrap_or(0);
        if ctx.in_opaque_span(offset) {
            continue;
        }
        depth += if matches!(byte, b'(' | b'[') { 1 } else { -1 };
    }
    depth > 0
}

/// Checks whether the end keywords are aligned properly for `do`/`end`
/// blocks.
#[derive(Debug, Clone, Default)]
pub struct BlockAlignment {
    /// `EnforcedStyleAlignWith`.
    style: Option<Style>,
    /// Per-node [`Detail`], paired with the depth of the node it describes.
    stack: Vec<(usize, Detail)>,
    /// Scratch buffer for the reconstructed lineage, reused across blocks.
    lineage: Vec<Link>,
}

impl Rule for BlockAlignment {
    const META: RuleMeta = RuleMeta {
        name: "Layout/BlockAlignment",
        department: Department::Layout,
        summary: "Align block ends correctly.",
        explanation: "\
Checks whether the `end` keywords of `do`/`end` blocks (and the closing `}`
of brace blocks) are aligned properly, for the blocks whose `end` begins its
own line.

```ruby
# bad
foo.bar
  .each do
    baz
      end

# good (EnforcedStyleAlignWith: either, the default -- both are accepted)
foo.bar
  .each do
    baz
  end

foo.bar
  .each do
    baz
end
```

`start_of_block` requires alignment with the start of the line the `do` is
on, `start_of_line` with the start of the line the whole expression started
on. When the `do` or `{` sits on a continuation line of a parenthesised
argument list, the method dispatch line is the anchor instead of that
continuation line.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: KINDS,
        config: &[ConfigOption {
            name: "EnforcedStyleAlignWith",
            default: ConfigDefault::Str("either"),
            allowed: &["either", "start_of_block", "start_of_line"],
            doc: "Which line the block's `end` must line up with.",
        }],
        blind_spots: "\
`inside_parentheses?` counts raw `(`/`[`/`)`/`]` bytes outside string,
symbol, regexp and comment bodies rather than lexer tokens, so a `(`
delimiting a bare command argument list (`foo (a,` -- upstream's
`tLPAREN_ARG`, which it does not count) and the brackets of a `%w[...]`
literal are counted where upstream would not.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyleAlignWith")? {
            "start_of_block" => Style::StartOfBlock,
            "start_of_line" => Style::StartOfLine,
            _ => Style::Either,
        };
        Ok(Self { style: Some(style), ..Self::default() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let depth = ctx.depth();
        let detail = self.detail(node, ctx);
        self.stack.push((depth, detail));
    }

    fn leave(&mut self, _node: &Node<'_>, _ctx: &mut Context<'_>) {
        self.stack.pop();
    }
}

impl BlockAlignment {
    /// Classifies `node`, running the alignment check first when it owns a
    /// block.
    fn detail(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) -> Detail {
        match node.kind() {
            NodeKind::CallNode => {
                let Some(call) = node.as_call_node() else { return Detail::Opaque };
                if call.block().and_then(|b| b.as_block_node()).is_some() {
                    self.check(node, ctx);
                    return Detail::BlockOwner;
                }
                let name = call.name();
                Detail::Send {
                    shovel: name.as_slice() == b"<<",
                    index: name.as_slice() == b"[]",
                    setter: call.equal_loc().is_some(),
                    csend: call.is_safe_navigation(),
                    receiver: call.receiver().map(|r| r.span()),
                }
            }
            NodeKind::SuperNode => {
                let owns_block = node
                    .as_super_node()
                    .and_then(|s| s.block())
                    .and_then(|b| b.as_block_node())
                    .is_some();
                if owns_block {
                    self.check(node, ctx);
                    return Detail::BlockOwner;
                }
                Detail::Opaque
            }
            NodeKind::ForwardingSuperNode => {
                let owns_block = node.as_forwarding_super_node().and_then(|s| s.block()).is_some();
                if owns_block {
                    self.check(node, ctx);
                    return Detail::BlockOwner;
                }
                Detail::Opaque
            }
            NodeKind::LambdaNode => {
                self.check(node, ctx);
                Detail::BlockOwner
            }
            NodeKind::StatementsNode => {
                let single = node.as_statements_node().is_some_and(|s| {
                    let mut body = s.body().iter();
                    body.next().is_some() && body.next().is_none()
                });
                if single {
                    Detail::Transparent
                } else {
                    Detail::Opaque
                }
            }
            NodeKind::MultiWriteNode
            | NodeKind::ConstantPathOperatorWriteNode
            | NodeKind::CallOperatorWriteNode
            | NodeKind::IndexOperatorWriteNode => Self::lhs_detail(node),
            _ => {
                let span = variable_name_span(node).unwrap_or_else(|| node.span());
                Detail::Lhs { span, masgn: false }
            }
        }
    }

    /// Builds the `Detail::Lhs` span for the write-node kinds handled
    /// outside `detail`'s main match.
    fn lhs_detail(node: &Node<'_>) -> Detail {
        match node.kind() {
            NodeKind::MultiWriteNode => {
                let Some(multi) = node.as_multi_write_node() else { return Detail::Opaque };
                let mut span: Option<Span> = None;
                let mut extend = |other: Span| {
                    span = Some(match span {
                        Some(cur) => Span::new(cur.start.min(other.start), cur.end.max(other.end)),
                        None => other,
                    });
                };
                if let (Some(lparen), Some(rparen)) = (multi.lparen_loc(), multi.rparen_loc()) {
                    extend(Span::new(lparen.span().start, rparen.span().end));
                } else {
                    for target in &multi.lefts() {
                        extend(target.span());
                    }
                    if let Some(rest) = multi.rest() {
                        extend(rest.span());
                    }
                    for target in &multi.rights() {
                        extend(target.span());
                    }
                }
                Detail::Lhs { span: span.unwrap_or_else(|| node.span()), masgn: true }
            }
            NodeKind::ConstantPathOperatorWriteNode => {
                let span = node
                    .as_constant_path_operator_write_node()
                    .map_or_else(|| node.span(), |n| n.target().as_node().span());
                Detail::Lhs { span, masgn: false }
            }
            NodeKind::CallOperatorWriteNode => {
                let span = node.as_call_operator_write_node().map_or_else(
                    || node.span(),
                    |n| {
                        let start =
                            n.receiver().map_or_else(|| node.span().start, |r| r.span().start);
                        let end = n.message_loc().map_or(node.span().end, |l| l.span().end);
                        Span::new(start, end)
                    },
                );
                Detail::Lhs { span, masgn: false }
            }
            NodeKind::IndexOperatorWriteNode => {
                let span = node.as_index_operator_write_node().map_or_else(
                    || node.span(),
                    |n| {
                        let start =
                            n.receiver().map_or_else(|| node.span().start, |r| r.span().start);
                        let end = n.closing_loc().span().end;
                        Span::new(start, end)
                    },
                );
                Detail::Lhs { span, masgn: false }
            }
            _ => unreachable!("lhs_detail only called for the four write-node kinds above"),
        }
    }

    /// RuboCop's `check_block_alignment`, driven from the block's owner.
    fn check(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(parts) = parts(node) else { return };
        if !ctx.begins_its_line(parts.end_loc) {
            return;
        }
        let style = self.style.unwrap_or(Style::Either);
        let end = ctx.line_col(parts.end_loc.start);

        self.build_lineage(ctx, node.span());
        let (index, lhs) = self.block_end_align_target(ctx);
        let start_span = if style == Style::StartOfLine {
            self.start_for_line_span(ctx, index, lhs)
        } else {
            lhs.unwrap_or_else(|| self.lineage[index].span)
        };
        let start = ctx.line_col(start_span.start);
        if start.column == end.column && style != Style::StartOfBlock {
            return;
        }

        // `compute_do_source_line_column`.
        let do_line = ctx.line_col(parts.do_loc.start).line;
        let Some((do_indent, _)) = first_non_space(ctx, do_line) else { return };
        let do_line_first_char = ctx.line_span(do_line).start + do_indent;
        let anchor_start = if inside_parentheses(ctx, node.span().start, do_line_first_char)
            && begins_inside_argument(node, do_line_first_char)
        {
            parts.selector.start
        } else {
            parts.do_loc.start
        };
        let anchor_line = ctx.line_col(anchor_start).line;
        let Some((anchor_indent, anchor_span)) = first_non_space(ctx, anchor_line) else { return };
        let permitted =
            anchor_indent == end.column || (style == Style::Either && do_indent == end.column);
        if permitted && style != Style::StartOfLine {
            return;
        }

        // `register_offense`.
        let anchor_source = String::from_utf8_lossy(ctx.text(anchor_span)).into_owned();
        let prefer = if style == Style::StartOfBlock {
            format_source_line_column(&anchor_source, anchor_line, anchor_indent)
        } else {
            format_source_line_column(&first_line_of(ctx, start_span), start.line, start.column)
        };
        let alt = if style == Style::Either
            && !(start.line == anchor_line && start.column == anchor_indent)
        {
            format!(" or {}", format_source_line_column(&anchor_source, anchor_line, anchor_indent))
        } else {
            String::new()
        };
        let current = format_source_line_column(
            &String::from_utf8_lossy(ctx.text(parts.end_loc)),
            end.line,
            end.column,
        );
        let message = format!("{current} is not aligned with {prefer}{alt}.");

        // `autocorrect`.
        let target_column = if style == Style::StartOfBlock {
            anchor_indent
        } else {
            let span = if style == Style::StartOfLine {
                start_span
            } else {
                self.start_for_line_span(ctx, index, lhs)
            };
            ctx.line_col(span.start).column
        };
        let edit = match target_column.cmp(&end.column) {
            std::cmp::Ordering::Greater => {
                let pad = usize::try_from(target_column - end.column).unwrap_or(0);
                Some(Edit::insert(parts.end_loc.start, vec![b' '; pad]))
            }
            std::cmp::Ordering::Less => Some(Edit::delete(Span::new(
                parts.end_loc.start - (end.column - target_column),
                parts.end_loc.start,
            ))),
            std::cmp::Ordering::Equal => None,
        };
        match edit {
            Some(edit) => ctx.report_with_fix(
                &Self::META,
                parts.end_loc,
                message,
                Fix { applicability: Applicability::Safe, edits: vec![edit] },
            ),
            None => ctx.report(&Self::META, parts.end_loc, message),
        }
    }

    /// Rebuilds `[node, *node.ancestors]` as whitequark would have had it.
    fn build_lineage(&mut self, ctx: &Context<'_>, head: Span) {
        self.lineage.clear();
        self.lineage.push(Link { span: head, logical: Logical::Block });
        let ancestors = ctx.ancestors();
        let mut cursor = self.stack.len();
        for index in (0..ancestors.len()).rev() {
            let info = ancestors[index];
            let mut detail = None;
            while cursor > 0 && self.stack[cursor - 1].0 > index {
                cursor -= 1;
            }
            if cursor > 0 && self.stack[cursor - 1].0 == index {
                detail = Some(self.stack[cursor - 1].1);
                cursor -= 1;
            }
            if let Some(logical) = logical_of(info.kind, detail) {
                self.lineage.push(Link { span: info.span, logical });
            }
        }
    }

    /// RuboCop's `block_end_align_target` followed by `find_lhs_node`:
    /// returns the lineage index of the target and, when the target is an
    /// `op_asgn`/`masgn`, the left-hand side to report instead.
    fn block_end_align_target(&self, ctx: &Context<'_>) -> (usize, Option<Span>) {
        let mut index = self.lineage.len() - 1;
        for i in 0..self.lineage.len() - 1 {
            let current = self.lineage[i];
            let parent = self.lineage[i + 1];
            let disqualified = !matches!(parent.logical, Logical::Masgn { .. })
                && ctx.line_col(parent.span.start).line != ctx.line_col(current.span.start).line;
            if disqualified || !is_align_target_parent(parent.logical, current.span) {
                index = i;
                break;
            }
        }
        (index, self.lineage[index].logical.lhs())
    }

    /// RuboCop's `start_for_line_node`: the outermost ancestor of the
    /// `start_of_block` target that begins on the same line as it.
    fn start_for_line_span(&self, ctx: &Context<'_>, index: usize, lhs: Option<Span>) -> Span {
        let start_span = lhs.unwrap_or_else(|| self.lineage[index].span);
        let start_line = ctx.line_col(start_span.start).line;
        let mut found = None;
        for j in (index + 1..self.lineage.len()).rev() {
            if ctx.line_col(self.lineage[j].span.start).line == start_line {
                found = Some(j);
                break;
            }
        }
        if found.is_none()
            && lhs.is_some()
            && ctx.line_col(self.lineage[index].span.start).line == start_line
        {
            found = Some(index);
        }
        match found {
            Some(j) => self.lineage[j].logical.lhs().unwrap_or(self.lineage[j].span),
            None => start_span,
        }
    }
}

/// `block_end_align_target?(parent, child)`.
fn is_align_target_parent(parent: Logical, child: Span) -> bool {
    match parent {
        Logical::Assignment
        | Logical::OpAsgn { .. }
        | Logical::Masgn { .. }
        | Logical::Def
        | Logical::Splat
        | Logical::AndOr => true,
        Logical::Send { shovel, index, setter, csend, receiver } => {
            setter || (!csend && (shovel || (!index && receiver == Some(child))))
        }
        Logical::Block | Logical::Other => false,
    }
}

/// Maps a Prism ancestor onto its whitequark counterpart. `None` means the
/// node is Prism-only and must not appear in the lineage at all.
fn logical_of(kind: NodeKind, detail: Option<Detail>) -> Option<Logical> {
    if matches!(kind, NodeKind::ProgramNode | NodeKind::ArgumentsNode | NodeKind::BlockNode) {
        return None;
    }
    match detail {
        Some(Detail::Transparent) => None,
        Some(Detail::BlockOwner) => Some(Logical::Block),
        Some(Detail::Opaque) => Some(Logical::Other),
        Some(Detail::Send { shovel, index, setter, csend, receiver }) => {
            Some(Logical::Send { shovel, index, setter, csend, receiver })
        }
        Some(Detail::Lhs { span, masgn: true }) => Some(Logical::Masgn { lhs: span }),
        Some(Detail::Lhs { span, masgn: false }) => Some(Logical::OpAsgn { lhs: span }),
        None => Some(match kind {
            NodeKind::DefNode => Logical::Def,
            NodeKind::SplatNode => Logical::Splat,
            NodeKind::AndNode | NodeKind::OrNode => Logical::AndOr,
            _ if is_plain_assignment(kind) => Logical::Assignment,
            _ => Logical::Other,
        }),
    }
}

/// The write-node kinds whitequark calls `lvasgn`/`ivasgn`/`cvasgn`/
/// `gvasgn`/`casgn`/`or_asgn`/`and_asgn`, all of which answer `assignment?`.
fn is_plain_assignment(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::LocalVariableWriteNode
            | NodeKind::LocalVariableAndWriteNode
            | NodeKind::LocalVariableOrWriteNode
            | NodeKind::InstanceVariableWriteNode
            | NodeKind::InstanceVariableAndWriteNode
            | NodeKind::InstanceVariableOrWriteNode
            | NodeKind::ClassVariableWriteNode
            | NodeKind::ClassVariableAndWriteNode
            | NodeKind::ClassVariableOrWriteNode
            | NodeKind::GlobalVariableWriteNode
            | NodeKind::GlobalVariableAndWriteNode
            | NodeKind::GlobalVariableOrWriteNode
            | NodeKind::ConstantWriteNode
            | NodeKind::ConstantAndWriteNode
            | NodeKind::ConstantOrWriteNode
            | NodeKind::ConstantPathWriteNode
            | NodeKind::ConstantPathAndWriteNode
            | NodeKind::ConstantPathOrWriteNode
            | NodeKind::CallAndWriteNode
            | NodeKind::CallOrWriteNode
            | NodeKind::IndexAndWriteNode
            | NodeKind::IndexOrWriteNode
    )
}

/// `name_loc` of the variable-flavoured `op_asgn` nodes, whose `lhs` is a
/// bare target node covering just the name.
fn variable_name_span(node: &Node<'_>) -> Option<Span> {
    match node.kind() {
        NodeKind::LocalVariableOperatorWriteNode => {
            Some(node.as_local_variable_operator_write_node()?.name_loc().span())
        }
        NodeKind::InstanceVariableOperatorWriteNode => {
            Some(node.as_instance_variable_operator_write_node()?.name_loc().span())
        }
        NodeKind::ClassVariableOperatorWriteNode => {
            Some(node.as_class_variable_operator_write_node()?.name_loc().span())
        }
        NodeKind::GlobalVariableOperatorWriteNode => {
            Some(node.as_global_variable_operator_write_node()?.name_loc().span())
        }
        NodeKind::ConstantOperatorWriteNode => {
            Some(node.as_constant_operator_write_node()?.name_loc().span())
        }
        _ => None,
    }
}
