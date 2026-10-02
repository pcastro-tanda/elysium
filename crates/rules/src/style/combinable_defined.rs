//! `Style/CombinableDefined`, ported from RuboCop's
//! `lib/rubocop/cop/style/combinable_defined.rb`.
//!
//! # Duplicate-offense merging
//!
//! Upstream's `on_and` fires once per `and` node in the tree (nested `and`s
//! included), and for a 3+ term chain both the inner and the outer `and`
//! node can each independently find a redundant term to remove. When two
//! `add_offense` calls land on the *exact same* `node` (true only for the
//! outermost `and` of a chain, when it finds more than one redundant term),
//! RuboCop merges their correctors into a single offense rather than
//! reporting twice (`Base#add_offense`'s `duplicate_location?` handling).
//! This port replicates that by collecting every redundant term for one
//! `and` node first and reporting them together in one [`Context::report_with_fix`]
//! call, rather than reporting once per term.
//!
//! # `terms`
//!
//! Upstream's `node.each_descendant.select { |d| d.parent.and_type? &&
//! !d.and_type? }` walks the *whole* subtree looking for any node whose
//! immediate parent is an `and`; for a pure `&&`/`and` chain (the only
//! shape `defined?` terms ever appear in) this is equivalent to recursively
//! unfolding left/right operands that are themselves `and` nodes, which
//! [`collect_terms`] does directly, pairing each leaf with whether it was a
//! right operand (needed later to pick [`removal_edit`]'s lhs/rhs removal,
//! exactly as `remove_term`'s `term == term.parent.children.last` check
//! does).
//!
//! # Structural equality
//!
//! `namespaces.any?(call)` is rubocop-ast's location-independent AST `==`;
//! this port compares the two nodes' own source text instead (as several
//! already-ported cops do for the same purpose), which agrees whenever
//! neither snippet contains a comment or non-significant whitespace -- true
//! for the identifier-only shapes `defined?`'s argument can take.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

/// RuboCop's `MSG`.
const MSG: &str = "Combine nested `defined?` calls.";

/// Checks successive `defined?` calls that can be combined into a single call.
#[derive(Debug, Clone)]
pub struct CombinableDefined;

impl Rule for CombinableDefined {
    const META: RuleMeta = RuleMeta {
        name: "Style/CombinableDefined",
        department: Department::Style,
        summary: "Checks successive `defined?` calls that can be combined into a single call.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::AndNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(and) = node.as_and_node() else { return };

        let mut terms = Vec::new();
        collect_terms(&and.left(), false, &mut terms);
        collect_terms(&and.right(), true, &mut terms);
        if terms.is_empty() || !terms.iter().all(|(t, _)| t.kind() == NodeKind::DefinedNode) {
            return;
        }

        // `defined_calls`: each term's subject, when it is a constant or a
        // method call (upstream's `subject.type?(:const, :call)`).
        let pairs: Vec<(Node<'_>, bool, Node<'_>)> = terms
            .into_iter()
            .filter_map(|(term, is_rhs)| {
                let subject = term.as_defined_node()?.value();
                matches!(
                    subject.kind(),
                    NodeKind::ConstantReadNode | NodeKind::ConstantPathNode | NodeKind::CallNode
                )
                .then_some((term, is_rhs, subject))
            })
            .collect();

        let namespaces: Vec<&[u8]> = pairs
            .iter()
            .filter_map(|(_, _, subject)| namespace_of(subject))
            .map(|ns| ctx.text(ns.span()))
            .collect();

        let edits: Vec<Edit> = pairs
            .iter()
            .filter(|(_, _, subject)| namespaces.contains(&ctx.text(subject.span())))
            .map(|(term, is_rhs, _)| removal_edit(term, *is_rhs, ctx))
            .collect();

        if edits.is_empty() {
            return;
        }
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            MSG,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}

/// Unfolds `node` into its leaf `and`-chain terms, pairing each with
/// whether it is the right operand of its immediate `and` parent.
fn collect_terms<'pr>(node: &Node<'pr>, is_rhs: bool, out: &mut Vec<(Node<'pr>, bool)>) {
    if let Some(and) = node.as_and_node() {
        collect_terms(&and.left(), false, out);
        collect_terms(&and.right(), true, out);
    } else {
        out.push((*node, is_rhs));
    }
}

/// `node.namespace`/`node.receiver`: the parent path of a constant
/// reference, or the receiver of a method call -- `None` for a bare
/// constant or a receiver-less call.
fn namespace_of<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    if let Some(path) = node.as_constant_path_node() {
        return path.parent();
    }
    if let Some(call) = node.as_call_node() {
        return call.receiver();
    }
    None
}

/// `source` starting at `pos` begins with `&&` or `and`.
fn starts_with_operator(source: &[u8], pos: usize) -> bool {
    source[pos..].starts_with(b"&&") || source[pos..].starts_with(b"and")
}

/// `source` up to and including `pos` ends with `&&` or `and`.
fn ends_with_operator(source: &[u8], pos: usize) -> bool {
    let prefix = &source[..=pos];
    prefix.ends_with(b"&&") || prefix.ends_with(b"and")
}

/// `lhs_range_to_remove`/`rhs_range_to_remove`, dispatching on whether
/// `term` is the right operand of its immediate `and` parent.
fn removal_edit(term: &Node<'_>, is_rhs: bool, ctx: &Context<'_>) -> Edit {
    let source = ctx.source().bytes();
    let span = if is_rhs {
        let mut pos = term.span().start as usize;
        while !starts_with_operator(source, pos) {
            pos -= 1;
        }
        Span::new(u32::try_from(pos - 1).expect("pos > 0"), term.span().end)
    } else {
        let mut pos = term.span().end as usize;
        while !ends_with_operator(source, pos) {
            pos += 1;
        }
        Span::new(term.span().start, u32::try_from(pos + 1).expect("fits u32"))
    };
    // Upstream's `range_with_surrounding_space(..., side: :right, newlines:
    // false)` leaves `whitespace` at its default, `false`.
    let span = ctx.with_surrounding_space(span, Side::Right, false, false);
    Edit::delete(span)
}
