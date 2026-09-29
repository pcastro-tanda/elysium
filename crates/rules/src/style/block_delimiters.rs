//! `Style/BlockDelimiters`, ported from RuboCop's
//! `lib/rubocop/cop/style/block_delimiters.rb`.
//!
//! # Prism shape
//!
//! whitequark wraps a call that carries a literal block in its own `block`
//! (or `numblock`/`itblock`) node, with the plain `send`/`csend` nested
//! *inside* it as a child; a chained call's "parent" of that wrapper is
//! whatever consumes the whole `foo { }` expression. Prism instead attaches
//! the block directly to the owning [`ruby_ast::node::CallNode`] via its own
//! `block` field, and that `CallNode`'s span already extends through the
//! block -- so the owning `CallNode` *is* the Prism equivalent of
//! whitequark's whole block node, and [`Context::ancestors`] gives its
//! parent for free one level further up than the [`NodeKind::BlockNode`]
//! itself sits. `numblock`/`itblock` need no separate handling here either:
//! Prism represents `_1`/`it`-style implicit parameters as ordinary
//! [`NodeKind::BlockNode`]s whose `parameters` field is a
//! `NumberedParametersNode`/`ItParametersNode` instead of a
//! `BlockParametersNode`, so `on_block`/`on_numblock`/`on_itblock` collapse
//! onto the same entry point.
//!
//! `CallNode::block()` also yields a `BlockArgumentNode` for an explicit
//! `&block` pass; every place this file reads it filters with
//! `as_block_node()` to exclude that shape, matching RuboCop-AST's
//! `any_block_type?`.
//!
//! Facts a live node handle can answer only while it is still on the stack
//! (a call's own name/arguments/parenthesization, a `StatementsNode`'s last
//! element, an `ArgumentsNode`'s last argument, a call's receiver) are
//! captured into small per-file caches as the single traversal reaches
//! them, keyed by span; by the time a block is checked, every one of its
//! ancestors has already contributed its cache entry. `owning` in
//! particular is keyed by the *block's* own span (not the call's), recorded
//! whenever a `CallNode` carrying one is entered, since the block itself
//! carries no back-pointer to its call.
//!
//! `get_blocks`'s `:hash`/`:pair` match arms become `KeywordHashNode`
//! (bare, braceless keyword arguments) and `AssocNode`; a braced
//! `HashNode` is the "has braces" case upstream returns out of without
//! recursing.

use std::collections::{HashMap, HashSet};

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::node::{BlockNode, CallNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

/// RuboCop-AST's `OPERATOR_METHODS` (private in `MethodIdentifierPredicates`).
const OPERATOR_METHODS: &[&[u8]] = &[
    b"|", b"^", b"&", b"<=>", b"==", b"===", b"=~", b">", b">=", b"<", b"<=", b"<<", b">>", b"+",
    b"-", b"*", b"/", b"%", b"**", b"~", b"+@", b"-@", b"!@", b"~@", b"[]", b"[]=", b"!", b"!=",
    b"!~", b"`",
];

const DEFAULT_PROCEDURAL_METHODS: &[&str] = &[
    "benchmark",
    "bm",
    "bmbm",
    "create",
    "each_with_object",
    "measure",
    "new",
    "realtime",
    "tap",
    "with_object",
];

const DEFAULT_FUNCTIONAL_METHODS: &[&str] = &["let", "let!", "subject", "watch"];

const DEFAULT_ALLOWED_METHODS: &[&str] = &["lambda", "proc", "it"];

const ALWAYS_BRACES_MESSAGE: &str = "Prefer `{...}` over `do...end` for blocks.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    LineCountBased,
    Semantic,
    BracesForChaining,
    AlwaysBraces,
}

impl Style {
    fn parse(value: &str) -> Self {
        match value {
            "semantic" => Self::Semantic,
            "braces_for_chaining" => Self::BracesForChaining,
            "always_braces" => Self::AlwaysBraces,
            _ => Self::LineCountBased,
        }
    }
}

/// Facts about the `CallNode` that owns one block, captured while that
/// call is still live (on entry), keyed by the block's own span.
#[derive(Debug, Clone)]
struct OwningFacts {
    /// The owning call's own span, which already extends through its
    /// block -- RuboCop's whitequark block-node span.
    call_span: Span,
    method_name: Vec<u8>,
    /// `send_node.arguments?`.
    has_args: bool,
    /// `send_node.parenthesized?`.
    parenthesized: bool,
}

/// RuboCop-AST's `assignment_method?`.
fn is_assignment_method(name: &[u8]) -> bool {
    name.ends_with(b"=") && !matches!(name, b"==" | b"===" | b"!=" | b"<=" | b">=")
}

/// `CallNode::closing_loc` text is `)` -- RuboCop-AST's `parenthesized?`
/// (`loc_is?(:end, ')')`).
fn is_parenthesized(call: &CallNode<'_>, ctx: &Context<'_>) -> bool {
    call.closing_loc().is_some_and(|loc| ctx.text(loc.span()) == b")")
}

/// RuboCop's `single_argument_operator_method?`: an operator-method call
/// with exactly one argument, itself a call carrying its own literal block
/// (whitequark's `block_type?` on that argument).
fn single_argument_operator_method(call: &CallNode<'_>) -> bool {
    if !OPERATOR_METHODS.contains(&call.name().as_slice()) {
        return false;
    }
    let Some(args) = call.arguments() else { return false };
    let list = args.arguments();
    list.len() == 1
        && list.first().is_some_and(|first| {
            first
                .as_call_node()
                .is_some_and(|c| c.block().and_then(|b| b.as_block_node()).is_some())
        })
}

/// RuboCop's `get_blocks`: recursively finds every block whose delimiter
/// style is ambiguous because it hangs off an unparenthesized argument,
/// inserting its span into `ignored`.
fn get_blocks(node: &Node<'_>, ignored: &mut HashSet<Span>) {
    if let Some(call) = node.as_call_node() {
        if let Some(block) = call.block().and_then(|b| b.as_block_node()) {
            ignored.insert(block.as_node().span());
            return;
        }
        if let Some(receiver) = call.receiver() {
            get_blocks(&receiver, ignored);
        }
        if let Some(args) = call.arguments() {
            for arg in &args.arguments() {
                get_blocks(&arg, ignored);
            }
        }
        return;
    }
    if let Some(hash) = node.as_keyword_hash_node() {
        for element in &hash.elements() {
            get_blocks(&element, ignored);
        }
        return;
    }
    if let Some(assoc) = node.as_assoc_node() {
        get_blocks(&assoc.key(), ignored);
        get_blocks(&assoc.value(), ignored);
    }
    // A braced `HashNode`, or anything else: RuboCop returns without
    // recursing (the braces already resolve the ambiguity).
}

/// `BlockNode::braces?`.
fn is_braces(block: &BlockNode<'_>, ctx: &Context<'_>) -> bool {
    ctx.text(block.opening_loc().span()).first() == Some(&b'{')
}

/// `BlockNode::multiline?`, which upstream overrides to compare only the
/// delimiters' own lines, not the whole node's span.
fn is_multiline(block: &BlockNode<'_>, ctx: &Context<'_>) -> bool {
    !ctx.same_line(block.opening_loc().span(), block.closing_loc().span())
}

/// `Node#assignment?`'s `EQUALS_ASSIGNMENTS`: a bare variable/constant
/// write, as opposed to a setter-method call (`CallNode`, already
/// `call_type?` on its own).
fn is_assignment_kind(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::LocalVariableWriteNode
            | NodeKind::InstanceVariableWriteNode
            | NodeKind::ClassVariableWriteNode
            | NodeKind::GlobalVariableWriteNode
            | NodeKind::ConstantWriteNode
            | NodeKind::ConstantPathWriteNode
            | NodeKind::MultiWriteNode
    )
}

fn whitespace_before(ctx: &Context<'_>, pos: u32) -> bool {
    pos > 0 && ctx.source().bytes().get((pos - 1) as usize).is_some_and(u8::is_ascii_whitespace)
}

fn whitespace_after(ctx: &Context<'_>, pos: u32, len: u32) -> bool {
    ctx.source().bytes().get((pos + len) as usize).is_some_and(u8::is_ascii_whitespace)
}

fn find_comment_on_line(ctx: &Context<'_>, line: u32) -> Option<Span> {
    ctx.comments().iter().find(|comment| comment.line == line).map(|comment| comment.span)
}

/// Walks `pos` left over a whitespace run, landing right after the last
/// non-whitespace byte.
fn skip_whitespace_backward(ctx: &Context<'_>, mut pos: u32) -> u32 {
    let bytes = ctx.source().bytes();
    while pos > 0 && bytes[(pos - 1) as usize].is_ascii_whitespace() {
        pos -= 1;
    }
    pos
}

/// The tight start of a `do...end` block's protected body, for the
/// "begin\n"/"\nend" autocorrect wrap: a `BeginNode`'s leading
/// `statements` span is always tight (unlike the `BeginNode` itself, see
/// [`BlockDelimiters::replace_do_end_with_braces`]); a bare modifier
/// `RescueModifierNode` body has no such quirk to begin with.
fn wrap_start(block: &BlockNode<'_>) -> Option<u32> {
    let body = block.body()?;
    if let Some(begin) = body.as_begin_node() {
        begin.statements().map(|stmts| stmts.as_node().span().start)
    } else {
        Some(body.span().start)
    }
}

/// Checks for uses of braces or do/end around single line or multi-line
/// blocks.
#[derive(Debug, Clone)]
pub struct BlockDelimiters {
    style: Style,
    allowed_methods: Vec<String>,
    allowed_patterns: Vec<Regex>,
    procedural_methods: HashSet<Vec<u8>>,
    functional_methods: HashSet<Vec<u8>>,
    allow_braces_on_procedural_oneliners: bool,
    braces_required_methods: Vec<String>,
    /// Blocks found by [`get_blocks`] hanging off an unparenthesized
    /// argument -- RuboCop's `ignore_node`.
    ignored: HashSet<Span>,
    /// Facts about the call owning each block, keyed by the block's own
    /// span.
    owning: HashMap<Span, OwningFacts>,
    /// A `CallNode`'s own span to its receiver's span, if any --
    /// supports `chained?`/`end_of_chain` without a live node handle.
    receiver_of: HashMap<Span, Span>,
    /// A `StatementsNode`'s own span to its last element's span.
    last_stmt_of: HashMap<Span, Span>,
    /// `StatementsNode` spans with exactly one element -- whitequark elides
    /// this wrapper entirely for a single-statement body, so any "parent"
    /// lookup must transparently skip over it to reach the real enclosing
    /// structure.
    singleton_stmt: HashSet<Span>,
    /// An `ArgumentsNode`'s own span to its last argument's span.
    last_arg_of: HashMap<Span, Span>,
}

impl BlockDelimiters {
    fn is_chained(&self, ctx: &Context<'_>, call_span: Span) -> bool {
        let ancestors = ctx.ancestors();
        ancestors.len() >= 2
            && ancestors[ancestors.len() - 2].kind == NodeKind::CallNode
            && self.receiver_of.get(&ancestors[ancestors.len() - 2].span) == Some(&call_span)
    }

    /// RuboCop's `end_of_chain`: climbs from `call_span` through every
    /// enclosing call it is the receiver of, returning the outermost link's
    /// span.
    fn end_of_chain_span(&self, ctx: &Context<'_>, mut current: Span) -> Span {
        let ancestors = ctx.ancestors();
        let mut idx = ancestors.len();
        loop {
            if idx < 2 {
                break;
            }
            let parent = ancestors[idx - 2];
            if parent.kind == NodeKind::CallNode
                && self.receiver_of.get(&parent.span) == Some(&current)
            {
                current = parent.span;
                idx -= 1;
            } else {
                break;
            }
        }
        current
    }

    /// RuboCop's `return_value_used?`, climbing through parenthesized
    /// groups and bridging two shapes whitequark has no wrapper for: a
    /// call's `ArgumentsNode`, and a single-statement `StatementsNode`
    /// (whitequark elides both -- arguments are direct children of the
    /// `send` node, and a lone statement has no `begin` wrapper at all).
    fn return_value_used(&self, ctx: &Context<'_>) -> bool {
        let ancestors = ctx.ancestors();
        let mut idx = ancestors.len();
        loop {
            if idx < 2 {
                return false;
            }
            let mut parent_idx = idx - 2;
            loop {
                let bridges = match ancestors[parent_idx].kind {
                    NodeKind::ArgumentsNode => true,
                    NodeKind::StatementsNode => {
                        self.singleton_stmt.contains(&ancestors[parent_idx].span)
                    }
                    _ => false,
                };
                if !bridges {
                    break;
                }
                if parent_idx == 0 {
                    return false;
                }
                parent_idx -= 1;
            }
            let parent = ancestors[parent_idx];
            match parent.kind {
                NodeKind::ParenthesesNode => {
                    idx = parent_idx + 1;
                }
                NodeKind::CallNode => return true,
                kind if is_assignment_kind(kind) => return true,
                _ => return false,
            }
        }
    }

    /// RuboCop's `return_value_of_scope?`, with the same single-statement
    /// `StatementsNode` bridging [`Self::return_value_used`] needs: a
    /// wrapper whitequark never introduces for a lone statement, so once
    /// one is bridged past, `call_span` was necessarily that wrapper's
    /// *entire* body -- trivially "the last child" of whatever encloses it,
    /// exactly like whitequark's flat, wrapper-less shape would report.
    fn return_value_of_scope(&self, ctx: &Context<'_>, call_span: Span) -> bool {
        let ancestors = ctx.ancestors();
        if ancestors.len() < 2 {
            return false;
        }
        let parent = ancestors[ancestors.len() - 2];
        if parent.kind == NodeKind::StatementsNode {
            if self.singleton_stmt.contains(&parent.span) {
                // Bridging past the elided wrapper lands wherever encloses
                // it -- true "return value of scope" position, *unless*
                // there is nothing further up at all: a lone top-level
                // program statement has no parent whatsoever in whitequark
                // either, so its "value" is never "used" by anything.
                return ancestors.len() >= 3
                    && ancestors[ancestors.len() - 3].kind != NodeKind::ProgramNode;
            }
            return self.last_stmt_of.get(&parent.span) == Some(&call_span);
        }
        match parent.kind {
            NodeKind::IfNode
            | NodeKind::UnlessNode
            | NodeKind::WhileNode
            | NodeKind::UntilNode
            | NodeKind::CaseNode
            | NodeKind::CaseMatchNode
            | NodeKind::AndNode
            | NodeKind::OrNode
            | NodeKind::ArrayNode
            | NodeKind::RangeNode => true,
            NodeKind::ArgumentsNode => self.last_arg_of.get(&parent.span) == Some(&call_span),
            _ => false,
        }
    }

    fn functional_block(
        &self,
        block: &BlockNode<'_>,
        facts: &OwningFacts,
        ctx: &Context<'_>,
    ) -> bool {
        let _ = block;
        self.return_value_used(ctx) || self.return_value_of_scope(ctx, facts.call_span)
    }

    fn semantic_block_style(
        &self,
        block: &BlockNode<'_>,
        facts: &OwningFacts,
        ctx: &Context<'_>,
    ) -> bool {
        let name = facts.method_name.as_slice();
        if is_braces(block, ctx) {
            self.functional_methods.contains(name)
                || self.functional_block(block, facts, ctx)
                || (self.allow_braces_on_procedural_oneliners && !is_multiline(block, ctx))
        } else {
            self.procedural_methods.contains(name) || !self.return_value_used(ctx)
        }
    }

    fn braces_for_chaining_style(
        &self,
        block: &BlockNode<'_>,
        facts: &OwningFacts,
        ctx: &Context<'_>,
    ) -> bool {
        if is_multiline(block, ctx) {
            if self.is_chained(ctx, facts.call_span) {
                is_braces(block, ctx)
            } else {
                !is_braces(block, ctx)
            }
        } else {
            is_braces(block, ctx)
        }
    }

    /// RuboCop's `modifier_rescue?`, applied to a `BeginNode`'s
    /// `rescue_clause`: a single, plain resbody (no exception classes, no
    /// reference variable, no chained `subsequent`, no `else`) -- upstream
    /// cannot distinguish this shape from a true `expr rescue expr2`
    /// modifier, and treats both alike.
    fn rescue_is_modifier_shaped(begin: &ruby_ast::node::BeginNode<'_>) -> bool {
        let Some(rescue) = begin.rescue_clause() else { return false };
        begin.statements().is_some()
            && begin.else_clause().is_none()
            && rescue.subsequent().is_none()
            && rescue.exceptions().is_empty()
            && rescue.reference().is_none()
    }

    /// RuboCop's `require_do_end?`.
    fn require_do_end(block: &BlockNode<'_>, ctx: &Context<'_>) -> bool {
        if is_braces(block, ctx) || is_multiline(block, ctx) {
            return false;
        }
        let Some(body) = block.body() else { return false };
        let Some(begin) = body.as_begin_node() else { return false };
        if begin.ensure_clause().is_some() {
            return true;
        }
        if begin.rescue_clause().is_none() {
            return false;
        }
        !Self::rescue_is_modifier_shaped(&begin)
    }

    fn braces_required_method(&self, name: &[u8]) -> bool {
        self.braces_required_methods.iter().any(|m| m.as_bytes() == name)
    }

    fn proper_block_style(
        &self,
        block: &BlockNode<'_>,
        facts: &OwningFacts,
        ctx: &Context<'_>,
    ) -> bool {
        if Self::require_do_end(block, ctx) {
            return true;
        }
        let name = facts.method_name.as_slice();
        if self.allowed_methods.iter().any(|m| m.as_bytes() == name) {
            return true;
        }
        let name_str = String::from_utf8_lossy(name);
        if self.allowed_patterns.iter().any(|pattern| pattern.is_match(&name_str)) {
            return true;
        }
        if self.braces_required_method(name) {
            return is_braces(block, ctx);
        }
        match self.style {
            Style::LineCountBased => is_multiline(block, ctx) ^ is_braces(block, ctx),
            Style::Semantic => self.semantic_block_style(block, facts, ctx),
            Style::BracesForChaining => self.braces_for_chaining_style(block, facts, ctx),
            Style::AlwaysBraces => is_braces(block, ctx),
        }
    }

    fn message(&self, block: &BlockNode<'_>, facts: &OwningFacts, ctx: &Context<'_>) -> String {
        let name = facts.method_name.as_slice();
        if self.braces_required_method(name) {
            let name_str = String::from_utf8_lossy(name);
            return format!("Brace delimiters `{{...}}` required for '{name_str}' method.");
        }
        match self.style {
            Style::LineCountBased => {
                if is_multiline(block, ctx) {
                    "Avoid using `{...}` for multi-line blocks.".to_string()
                } else {
                    "Prefer `{...}` over `do...end` for single-line blocks.".to_string()
                }
            }
            Style::Semantic => {
                if is_braces(block, ctx) {
                    "Prefer `do...end` over `{...}` for procedural blocks.".to_string()
                } else {
                    "Prefer `{...}` over `do...end` for functional blocks.".to_string()
                }
            }
            Style::BracesForChaining => {
                if is_multiline(block, ctx) {
                    if self.is_chained(ctx, facts.call_span) {
                        "Prefer `{...}` over `do...end` for multi-line chained blocks.".to_string()
                    } else {
                        "Prefer `do...end` for multi-line blocks without chaining.".to_string()
                    }
                } else {
                    "Prefer `{...}` over `do...end` for single-line blocks.".to_string()
                }
            }
            Style::AlwaysBraces => ALWAYS_BRACES_MESSAGE.to_string(),
        }
    }

    /// RuboCop's `correction_would_break_code?`.
    fn correction_would_break_code(
        block: &BlockNode<'_>,
        facts: &OwningFacts,
        ctx: &Context<'_>,
    ) -> bool {
        if is_braces(block, ctx) {
            return false;
        }
        facts.has_args && !facts.parenthesized
    }

    /// RuboCop's `begin_required?`.
    fn begin_required(block: &BlockNode<'_>, ctx: &Context<'_>) -> bool {
        if !is_multiline(block, ctx) {
            return false;
        }
        let Some(body) = block.body() else { return false };
        if let Some(begin) = body.as_begin_node() {
            begin.rescue_clause().is_some() || begin.ensure_clause().is_some()
        } else {
            body.as_rescue_modifier_node().is_some()
        }
    }

    /// RuboCop's `move_comment_before_block`.
    fn move_comment_before_block(
        &self,
        ctx: &Context<'_>,
        call_span: Span,
        closing_span: Span,
        comment_span: Span,
        edits: &mut Vec<Edit>,
    ) {
        let range_end = if self.is_chained(ctx, call_span) {
            self.end_of_chain_span(ctx, call_span).end
        } else {
            closing_span.end
        };
        let bytes = ctx.source().bytes();
        let mut trimmed_end = comment_span.start;
        while trimmed_end > range_end && bytes[(trimmed_end - 1) as usize].is_ascii_whitespace() {
            trimmed_end -= 1;
        }
        let remove_span = ctx.with_surrounding_space(comment_span, Side::Right, true, false);
        edits.push(Edit::delete(remove_span));
        if trimmed_end < comment_span.start {
            edits.push(Edit::delete(Span::new(trimmed_end, comment_span.start)));
        }
        edits.push(Edit::insert(trimmed_end, b"\n".to_vec()));
        let mut text = ctx.text(comment_span).to_vec();
        text.push(b'\n');
        edits.push(Edit::insert(call_span.start, text));
    }

    /// RuboCop's `replace_braces_with_do_end`.
    fn replace_braces_with_do_end(
        &self,
        block: &BlockNode<'_>,
        facts: &OwningFacts,
        ctx: &Context<'_>,
        edits: &mut Vec<Edit>,
    ) {
        let b = block.opening_loc().span();
        let e = block.closing_loc().span();
        if !whitespace_before(ctx, b.start) {
            edits.push(Edit::insert(b.start, b" ".to_vec()));
        }
        if !whitespace_before(ctx, e.start) {
            edits.push(Edit::insert(e.start, b" ".to_vec()));
        }
        if !whitespace_after(ctx, b.start, 1) {
            edits.push(Edit::insert(b.end, b" ".to_vec()));
        }
        edits.push(Edit::replace(b, b"do".to_vec()));
        let comment_line = ctx.line_col(e.start).line;
        if let Some(comment_span) = find_comment_on_line(ctx, comment_line) {
            self.move_comment_before_block(ctx, facts.call_span, e, comment_span, edits);
        }
        edits.push(Edit::replace(e, b"end".to_vec()));
    }

    /// RuboCop's `replace_do_end_with_braces`.
    ///
    /// A `BeginNode`'s own location is unreliable for the "begin\n"/"\nend"
    /// wrap: Prism gives it back the *whole enclosing block's* span (start
    /// at the block's own `do`, end at the block's own `end`) when there is
    /// no explicit `begin` keyword, rather than whitequark's tight range
    /// around just the protected statements. Its leading `statements` node
    /// is reliable (a `StatementsNode`'s span is always tight), giving the
    /// wrap's start ([`wrap_start`]); the end is instead found by trimming
    /// trailing whitespace back from the block's own closing delimiter,
    /// landing right after the last real token before it, exactly like
    /// whitequark's range would.
    fn replace_do_end_with_braces(block: &BlockNode<'_>, ctx: &Context<'_>, edits: &mut Vec<Edit>) {
        let b = block.opening_loc().span();
        let e = block.closing_loc().span();
        if !whitespace_after(ctx, b.start, 2) {
            edits.push(Edit::insert(b.end, b" ".to_vec()));
        }
        edits.push(Edit::replace(b, b"{".to_vec()));
        edits.push(Edit::replace(e, b"}".to_vec()));
        if Self::begin_required(block, ctx) {
            if let Some(wrap_start) = wrap_start(block) {
                let wrap_end = skip_whitespace_backward(ctx, e.start);
                edits.push(Edit::insert(wrap_start, b"begin\n".to_vec()));
                edits.push(Edit::insert(wrap_end, b"\nend".to_vec()));
            }
        }
    }

    fn autocorrect(
        &self,
        block: &BlockNode<'_>,
        facts: &OwningFacts,
        ctx: &Context<'_>,
    ) -> Option<Fix> {
        if Self::correction_would_break_code(block, facts, ctx) {
            return None;
        }
        let mut edits = Vec::new();
        if is_braces(block, ctx) {
            self.replace_braces_with_do_end(block, facts, ctx, &mut edits);
        } else {
            Self::replace_do_end_with_braces(block, ctx, &mut edits);
        }
        Some(Fix { applicability: Applicability::Safe, edits })
    }

    /// RuboCop's `on_block` plus `part_of_ignored_node?`: once a block is
    /// reported it is added to `ignored` unconditionally (matching
    /// upstream's `ignore_node(node)`, called right after `autocorrect`
    /// regardless of whether a fix was actually produced), and any
    /// descendant block of an ignored ancestor is skipped entirely without
    /// being checked -- ignoring the outer delimiter choice also silences
    /// every nested block it encloses for this same pass.
    fn check_block(&mut self, block: &BlockNode<'_>, ctx: &mut Context<'_>) {
        let block_span = block.as_node().span();
        if self.ignored.contains(&block_span)
            || ctx.ancestors().iter().any(|ancestor| self.ignored.contains(&ancestor.span))
        {
            return;
        }
        let Some(facts) = self.owning.get(&block_span).cloned() else { return };
        if self.proper_block_style(block, &facts, ctx) {
            return;
        }
        let message = self.message(block, &facts, ctx);
        let span = block.opening_loc().span();
        self.ignored.insert(block_span);
        match self.autocorrect(block, &facts, ctx) {
            Some(fix) => ctx.report_with_fix(&Self::META, span, message, fix),
            None => ctx.report(&Self::META, span, message),
        }
    }

    fn check_call(&mut self, call: &CallNode<'_>, node: &Node<'_>, ctx: &Context<'_>) {
        let call_span = node.span();
        if let Some(receiver) = call.receiver() {
            self.receiver_of.insert(call_span, receiver.span());
        }
        if let Some(block) = call.block().and_then(|b| b.as_block_node()) {
            self.owning.insert(
                block.as_node().span(),
                OwningFacts {
                    call_span,
                    method_name: call.name().as_slice().to_vec(),
                    has_args: call.arguments().is_some(),
                    parenthesized: is_parenthesized(call, ctx),
                },
            );
        }
        if call.arguments().is_some()
            && !is_parenthesized(call, ctx)
            && !is_assignment_method(call.name().as_slice())
            && !single_argument_operator_method(call)
        {
            if let Some(args) = call.arguments() {
                for arg in &args.arguments() {
                    get_blocks(&arg, &mut self.ignored);
                }
            }
        }
    }
}

impl Rule for BlockDelimiters {
    const META: RuleMeta = RuleMeta {
        name: "Style/BlockDelimiters",
        department: Department::Style,
        summary: "Avoid using {...} for multi-line blocks (multiline chaining is always ugly). \
                   Prefer {...} over do...end for single-line blocks.",
        explanation: "\
Checks for uses of braces or do/end around single line or multi-line blocks.

Methods that can be either procedural or functional and cannot be
categorised from their usage alone is ignored. `lambda`, `proc`, and `it`
are their defaults. Additional methods can be added to `AllowedMethods`.

With `EnforcedStyle: line_count_based` (default), braces are preferred for
single-line blocks and `do...end` for multi-line ones:

```ruby
# bad - single line block
items.each do |item| item / 5 end

# good - single line block
items.each { |item| item / 5 }

# bad - multi-line block
things.map { |thing|
  something = thing.some_method
  process(something)
}

# good - multi-line block
things.map do |thing|
  something = thing.some_method
  process(something)
end
```

With `EnforcedStyle: semantic`, `do...end` is preferred for procedural
blocks (return value discarded) and `{...}` for functional ones (return
value used).

With `EnforcedStyle: braces_for_chaining`, braces are required around a
multi-line block whose return value is chained with another method call.

With `EnforcedStyle: always_braces`, braces are always required.

`BracesRequiredMethods` overrides every other configuration except
`AllowedMethods` and forces `{...}` for the listed method names.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::CallNode,
            NodeKind::BlockNode,
            NodeKind::StatementsNode,
            NodeKind::ArgumentsNode,
        ],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("line_count_based"),
                allowed: &["line_count_based", "semantic", "braces_for_chaining", "always_braces"],
                doc: "The style of block delimiter to enforce.",
            },
            ConfigOption {
                name: "ProceduralMethods",
                default: ConfigDefault::StrList(DEFAULT_PROCEDURAL_METHODS),
                allowed: &[],
                doc: "Methods that are known to be procedural in nature but look functional \
                      from their usage, only used by the `semantic` style.",
            },
            ConfigOption {
                name: "FunctionalMethods",
                default: ConfigDefault::StrList(DEFAULT_FUNCTIONAL_METHODS),
                allowed: &[],
                doc: "Methods that are known to be functional in nature but look procedural \
                      from their usage, only used by the `semantic` style.",
            },
            ConfigOption {
                name: "AllowedMethods",
                default: ConfigDefault::StrList(DEFAULT_ALLOWED_METHODS),
                allowed: &[],
                doc: "Methods that can be either procedural or functional and cannot be \
                      categorised from their usage alone.",
            },
            ConfigOption {
                name: "AllowedPatterns",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Method name regex patterns always allowed to take either delimiter.",
            },
            ConfigOption {
                name: "AllowBracesOnProceduralOneLiners",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Whether a single-line procedural block may use braces, only used by the \
                      `semantic` style.",
            },
            ConfigOption {
                name: "BracesRequiredMethods",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Method names that always require brace delimiters, overriding every other \
                      configuration except `AllowedMethods`.",
            },
        ],
        blind_spots: "\
`modifier_rescue?`/`require_do_end?` cannot distinguish a true `expr rescue expr2` modifier from
a keyword `rescue`/`end` clause with no exception class, no reference variable, a single resbody
and no `else` -- both shapes are treated as modifier-like, exactly mirroring the upstream
(possibly imprecise) heuristic rather than fixing it.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = Style::parse(options.style("EnforcedStyle")?);
        let allowed_methods = options.str_list("AllowedMethods");
        let allowed_patterns =
            options.str_list("AllowedPatterns").iter().filter_map(|p| Regex::new(p).ok()).collect();
        let procedural_methods =
            options.str_list("ProceduralMethods").into_iter().map(String::into_bytes).collect();
        let functional_methods =
            options.str_list("FunctionalMethods").into_iter().map(String::into_bytes).collect();
        let allow_braces_on_procedural_oneliners = options.bool("AllowBracesOnProceduralOneLiners");
        let braces_required_methods = options.str_list("BracesRequiredMethods");
        Ok(Self {
            style,
            allowed_methods,
            allowed_patterns,
            procedural_methods,
            functional_methods,
            allow_braces_on_procedural_oneliners,
            braces_required_methods,
            ignored: HashSet::new(),
            owning: HashMap::new(),
            receiver_of: HashMap::new(),
            last_stmt_of: HashMap::new(),
            singleton_stmt: HashSet::new(),
            last_arg_of: HashMap::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => {
                if let Some(call) = node.as_call_node() {
                    self.check_call(&call, node, ctx);
                }
            }
            NodeKind::BlockNode => {
                if let Some(block) = node.as_block_node() {
                    self.check_block(&block, ctx);
                }
            }
            NodeKind::StatementsNode => {
                if let Some(stmts) = node.as_statements_node() {
                    let body = stmts.body();
                    if let Some(last) = body.last() {
                        self.last_stmt_of.insert(node.span(), last.span());
                    }
                    if body.len() == 1 {
                        self.singleton_stmt.insert(node.span());
                    }
                }
            }
            NodeKind::ArgumentsNode => {
                if let Some(args) = node.as_arguments_node() {
                    if let Some(last) = args.arguments().last() {
                        self.last_arg_of.insert(node.span(), last.span());
                    }
                }
            }
            _ => {}
        }
    }
}
