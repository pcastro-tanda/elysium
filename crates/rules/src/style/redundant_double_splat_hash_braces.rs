//! `Style/RedundantDoubleSplatHashBraces`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_double_splat_hash_braces.rb`.
//!
//! Prism has no parent pointers, so the whole-tree walk here tracks an
//! explicit ancestor stack (mirroring `lint/useless_method_definition.rs`),
//! skipping the transparent `ArgumentsNode` wrapper -- absent from
//! whitequark, where a call's arguments are its own direct children -- so a
//! hash literal's logical parent is always the call or `**` (`AssocSplatNode`)
//! it is actually nested in. RuboCop's own `add_offense` deduplicates
//! same-range offenses within one cop run (`current_offense_locations.add?`
//! in `Base#add_offense`); a braced hash passed as a *positional* argument to
//! a `merge`/`merge!` call in the chain (e.g. `{foo: bar}.merge({baz: qux})`)
//! resolves to the exact same `**`-span offense as the chain's own receiver
//! hash, so the span-keyed `seen` set here reproduces that dedup -- only the
//! first (pre-order, so the receiver before any argument) hash to reach a
//! given `**` span is corrected.

use std::collections::HashSet;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{AssocSplatNode, CallNode, HashNode};
use ruby_ast::{walk, LocationExt as _, Node, NodeExt as _, Visitor};
use ruby_source::Span;

const MSG: &str = "Remove the redundant double splat and braces, use keyword arguments directly.";

/// Checks for redundant uses of double splat hash braces.
#[derive(Debug, Clone)]
pub struct RedundantDoubleSplatHashBraces;

impl Rule for RedundantDoubleSplatHashBraces {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantDoubleSplatHashBraces",
        department: Department::Style,
        summary: "Checks for redundant uses of double splat hash braces.",
        explanation: "Checks for redundant uses of double splat hash braces.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let root = ctx.parsed().root();
        let mut finder = Finder { stack: Vec::new(), seen: HashSet::new(), offenses: Vec::new() };
        walk(&root, &mut finder);

        for offense in finder.offenses {
            let mut edits = vec![
                Edit::delete(offense.operator),
                Edit::delete(offense.opening),
                Edit::delete(offense.closing),
            ];
            if let Some(merge) = offense.merge {
                edits.push(Edit::replace(merge.range, build_merge_replacement(ctx, &merge.args)));
            }
            ctx.report_with_fix(
                &Self::META,
                offense.span,
                MSG,
                Fix { applicability: Applicability::Safe, edits },
            );
        }
    }
}

/// How one `merge`/`merge!` argument renders once the braces that wrapped it
/// are gone: RuboCop's `convert_to_new_arguments`/`hash_argument_source`.
enum ArgKind {
    /// A non-hash argument: rendered as `**<source>`.
    Splat(Span),
    /// A braceless keyword-hash argument: rendered as its own raw source.
    RawHash(Span),
    /// A braced hash-literal argument: rendered as its pairs' own sources,
    /// comma-joined (the braces themselves are dropped).
    BracedHash(Vec<Span>),
}

/// The `kwsplat.each_descendant(:call)`-driven replacement for a mergeable
/// chain's dot-through-end range.
struct MergeReplacement {
    range: Span,
    /// One entry per argument, in written (outer-to-inner reversed, i.e.
    /// first-written call first) order.
    args: Vec<ArgKind>,
}

/// One `on_hash` offense: the reported `**` span, the three deletions that
/// strip the operator and braces, and the optional merge-chain rewrite.
struct Offense {
    span: Span,
    operator: Span,
    opening: Span,
    closing: Span,
    merge: Option<MergeReplacement>,
}

/// Walks the whole tree tracking a live ancestor stack (Prism has no parent
/// pointers), collecting one [`Offense`] per `on_hash` match.
struct Finder<'pr> {
    stack: Vec<Node<'pr>>,
    seen: HashSet<Span>,
    offenses: Vec<Offense>,
}

impl<'pr> Visitor<'pr> for Finder<'pr> {
    fn enter(&mut self, node: &Node<'pr>) {
        if let Some(hash) = node.as_hash_node() {
            if let Some(offense) = check_hash(&self.stack, &hash) {
                if self.seen.insert(offense.span) {
                    self.offenses.push(offense);
                }
            }
        }
        if node.kind() != ruby_ast::NodeKind::ArgumentsNode {
            self.stack.push(*node);
        }
    }

    fn leave(&mut self, node: &Node<'pr>) {
        if node.kind() != ruby_ast::NodeKind::ArgumentsNode {
            self.stack.pop();
        }
    }
}

/// RuboCop's `MERGE_METHODS`.
fn is_merge_method(name: &[u8]) -> bool {
    name == b"merge" || name == b"merge!"
}

/// RuboCop's `on_hash` guards up through `return unless (kwsplat = ...)`.
fn check_hash<'pr>(stack: &[Node<'pr>], hash: &HashNode<'pr>) -> Option<Offense> {
    let elements = hash.elements();
    if elements.is_empty() {
        return None;
    }
    if elements.iter().any(|e| e.as_assoc_node().is_some_and(|a| a.operator_loc().is_some())) {
        return None;
    }

    let parent = *stack.last()?;
    if parent.as_call_node().is_none() && parent.as_assoc_splat_node().is_none() {
        return None;
    }
    if !mergeable_from(stack, isize::try_from(stack.len()).expect("stack fits isize") - 1) {
        return None;
    }

    let kwsplat = stack.iter().rev().find_map(ruby_ast::Node::as_assoc_splat_node)?;

    if allowed_double_splat_receiver(&kwsplat) {
        return None;
    }

    Some(build_offense(hash, &kwsplat))
}

/// RuboCop's `mergeable?`, walked up an explicit ancestor stack starting at
/// `idx` (the hash's own parent) instead of following `node.parent`.
fn mergeable_from(stack: &[Node<'_>], mut idx: isize) -> bool {
    loop {
        if idx < 0 {
            return true;
        }
        let node = stack[usize::try_from(idx).expect("checked non-negative")];
        let Some(call) = node.as_call_node() else { return true };
        if !is_merge_method(call.name().as_slice()) {
            return false;
        }
        idx -= 1;
    }
}

/// RuboCop's `allowed_double_splat_receiver?`.
fn allowed_double_splat_receiver(kwsplat: &AssocSplatNode<'_>) -> bool {
    let Some(first_child) = kwsplat.value() else { return true };
    if is_block_call(&first_child) {
        return true;
    }
    if first_child.as_call_node().is_none() {
        return false;
    }
    root_receiver(&first_child).is_none_or(|r| r.as_hash_node().is_none())
}

/// RuboCop's `first_child.any_block_type?`: a call with an attached block is
/// whitequark's `block`-wrapped call, Prism's `CallNode` with `block()` set.
fn is_block_call(node: &Node<'_>) -> bool {
    node.as_call_node().is_some_and(|c| c.block().is_some())
}

/// RuboCop's `root_receiver`.
fn root_receiver<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    let call = node.as_call_node()?;
    let receiver = call.receiver()?;
    match receiver.as_call_node().and_then(|c| c.receiver()) {
        Some(_) => root_receiver(&receiver),
        None => Some(receiver),
    }
}

/// RuboCop's `autocorrect` plus `opening_brace`/`closing_brace`, deferring
/// the merge-chain argument sources (needing source text) to
/// [`build_merge_replacement`].
fn build_offense<'pr>(hash: &HashNode<'pr>, kwsplat: &AssocSplatNode<'pr>) -> Offense {
    let elements = hash.elements();
    let first = elements.first().expect("checked nonempty");
    let last = elements.last().expect("checked nonempty");
    let opening = Span::new(hash.opening_loc().span().start, first.location().span().start);
    let closing = Span::new(last.location().span().end, hash.closing_loc().span().end);

    let chain = merge_chain(kwsplat.value());
    let merge = if chain.is_empty() {
        None
    } else {
        let begin_merge = chain.last().expect("checked nonempty");
        let end_merge = chain.first().expect("checked nonempty");
        let range = Span::new(
            begin_merge.call_operator_loc().expect("merge call has a dot").span().start,
            end_merge.location().span().end,
        );
        let mut args = Vec::new();
        for call in chain.iter().rev() {
            if let Some(call_args) = call.arguments() {
                for arg in &call_args.arguments() {
                    args.push(convert_argument_kind(&arg));
                }
            }
        }
        Some(MergeReplacement { range, args })
    };

    Offense {
        span: kwsplat.location().span(),
        operator: kwsplat.operator_loc().span(),
        opening,
        closing,
        merge,
    }
}

/// RuboCop's `select_merge_method_nodes`/`mergeable?`, specialised to the
/// linear receiver chain a `**`'s value can only be: walks `.receiver()`
/// while each link is a `merge`/`merge!` call.
fn merge_chain(value: Option<Node<'_>>) -> Vec<CallNode<'_>> {
    let mut chain = Vec::new();
    let mut current = value;
    while let Some(node) = current {
        let Some(call) = node.as_call_node() else { break };
        if !is_merge_method(call.name().as_slice()) {
            break;
        }
        current = call.receiver();
        chain.push(call);
    }
    chain
}

/// RuboCop's `convert_to_new_arguments`'s per-argument branch.
fn convert_argument_kind(arg: &Node<'_>) -> ArgKind {
    if let Some(hash) = arg.as_hash_node() {
        ArgKind::BracedHash(hash.elements().iter().map(|e| e.location().span()).collect())
    } else if let Some(kw) = arg.as_keyword_hash_node() {
        ArgKind::RawHash(kw.location().span())
    } else {
        ArgKind::Splat(arg.location().span())
    }
}

/// RuboCop's `autocorrect_merge_methods`'s `new_source` construction.
fn build_merge_replacement(ctx: &Context<'_>, args: &[ArgKind]) -> Vec<u8> {
    let parts: Vec<Vec<u8>> = args
        .iter()
        .map(|arg| match arg {
            ArgKind::Splat(span) => {
                let mut v = b"**".to_vec();
                v.extend_from_slice(ctx.text(*span));
                v
            }
            ArgKind::RawHash(span) => ctx.text(*span).to_vec(),
            ArgKind::BracedHash(spans) => {
                let pairs: Vec<Vec<u8>> = spans.iter().map(|s| ctx.text(*s).to_vec()).collect();
                pairs.join(&b", "[..])
            }
        })
        .collect();
    if parts.is_empty() {
        Vec::new()
    } else {
        let mut out = b", ".to_vec();
        out.extend(parts.join(&b", "[..]));
        out
    }
}
