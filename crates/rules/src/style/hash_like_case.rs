//! `Style/HashLikeCase`, ported from RuboCop's
//! `lib/rubocop/cop/style/hash_like_case.rb` plus the `MinBranchesCount`
//! mixin it includes and rubocop-ast's `Node#recursive_basic_literal?`/
//! `Node#basic_literal?` (`lib/rubocop/ast/node.rb`), which the cop's
//! `hash_like_case?` node-pattern relies on to decide which `when` bodies
//! even count as literals.
//!
//! # Node shapes
//!
//! Upstream's pattern `(case _ (when ${str_type? sym_type?}
//! $[!nil? recursive_basic_literal?])+ nil?)` requires: no `else` (the
//! trailing `nil?`), every branch a plain `when` with *exactly one*
//! condition that is a bare `str`/`sym` literal (not `dstr`/`dsym`), and a
//! non-nil body recognized as a "recursive basic literal". `nodes_of_same_type?`
//! then additionally requires every captured condition to share the same
//! `.type` (all `str`, or all `sym` -- never mixed) and every captured body
//! to share the same `.type` too.
//!
//! Prism's `WhenNode::statements` is always a `StatementsNode` (or absent),
//! whereas whitequark's parser never wraps a single-statement `when` body in
//! a `begin` node. [`effective_body`] unwraps a one-statement
//! `StatementsNode` down to that statement (so its own `.type`, e.g. `str`,
//! is what participates in the same-type check), and otherwise treats the
//! `StatementsNode` itself as the `begin`-equivalent container that
//! [`is_recursive_basic_literal`] recurses into.
//!
//! [`is_recursive_basic_literal`] mirrors rubocop-ast's
//! `recursive_basic_literal?`: leaf basic literals (`nil`/`true`/`false`,
//! integers, floats, strings, symbols), recursing through composite
//! literals (arrays, hashes, plain and interpolated strings/symbols/regexps,
//! `and`/`or`, ranges, parenthesized/interpolated single-statement wrappers,
//! and the small set of comparison/`!`/`*`/`<=>` sends rubocop-ast treats as
//! recursively literal). Rational and complex literals, and backtick/`%x`
//! command strings, are never recognized, matching this codebase's existing
//! `Lint/DuplicateHashKey` port of the same rubocop-ast method (a documented
//! false-negative-only gap there, adopted here for consistency).

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::StatementsNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Consider replacing `case-when` with a hash lookup.";

/// Checks for places where `case-when` represents a simple 1:1
/// mapping and can be replaced with a hash lookup.
#[derive(Debug, Clone)]
pub struct HashLikeCase {
    /// RuboCop's `MinBranchesCount` (`MinBranchesCount` mixin), default `3`.
    min_branches_count: i64,
}

impl Rule for HashLikeCase {
    const META: RuleMeta = RuleMeta {
        name: "Style/HashLikeCase",
        department: Department::Style,
        summary: "Checks for places where `case-when` represents a simple 1:1 mapping and can be replaced with a hash lookup.",
        explanation: "\
Checks for places where `case-when` represents a simple 1:1
mapping and can be replaced with a hash lookup.

```ruby
# bad
case country
when 'europe'
  'http://eu.example.com'
when 'america'
  'http://us.example.com'
when 'australia'
  'http://au.example.com'
end

# good
SITES = {
  'europe'    => 'http://eu.example.com',
  'america'   => 'http://us.example.com',
  'australia' => 'http://au.example.com'
}
SITES[country]
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CaseNode],
        config: &[linter::ConfigOption {
            name: "MinBranchesCount",
            default: linter::ConfigDefault::Int(3),
            allowed: &[],
            doc: "Minimum number of branches a `case-when` needs to be flagged.",
        }],
        blind_spots: "\
Bodies are recognized as literals by rubocop-ast's `recursive_basic_literal?`
(mirroring `Lint/DuplicateHashKey`'s existing port of the same method): plain
and interpolated strings/symbols/regexps, arrays, hashes, `and`/`or`,
ranges, parenthesized/interpolated single-statement wrappers, and the
comparison-operator/`!`/`*`/`<=>` sends rubocop-ast treats as recursively
literal. Rational and complex literals, and backtick/`%x` command strings,
are never recognized (a false-negative-only gap). A `MinBranchesCount` that
is not a positive integer falls back to the default of `3`, matching this
codebase's established fallback for the same shape of option in
`Style/GuardClause`'s `MinBodyLength`, rather than upstream's `raise`.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let min_branches_count = options.int("MinBranchesCount");
        let min_branches_count = if min_branches_count > 0 { min_branches_count } else { 3 };
        Ok(Self { min_branches_count })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(case_node) = node.as_case_node() else { return };

        // RuboCop's `min_branches_count?`: `node.when_branches.size >= min_branches_count`.
        let branches: Vec<Node<'_>> = case_node.conditions().iter().collect();
        let min_branches_count = usize::try_from(self.min_branches_count).unwrap_or(3);
        if branches.len() < min_branches_count {
            return;
        }

        // RuboCop's pattern's trailing `nil?`: no `else` branch.
        if case_node.else_clause().is_some() {
            return;
        }

        let mut condition_kind: Option<NodeKind> = None;
        let mut body_kind: Option<NodeKind> = None;

        for branch in &branches {
            let Some(when_node) = branch.as_when_node() else { return };

            // The pattern's `${str_type? sym_type?}`: exactly one condition, a bare
            // `str`/`sym` literal.
            let conditions: Vec<Node<'_>> = when_node.conditions().iter().collect();
            let [condition] = conditions.as_slice() else { return };
            let condition_kind_here = condition.kind();
            if condition_kind_here != NodeKind::StringNode
                && condition_kind_here != NodeKind::SymbolNode
            {
                return;
            }
            match condition_kind {
                None => condition_kind = Some(condition_kind_here),
                Some(previous) if previous != condition_kind_here => return,
                Some(_) => {}
            }

            // The pattern's `$[!nil? recursive_basic_literal?]`.
            let Some(statements) = when_node.statements() else { return };
            let body = effective_body(&statements);
            if !is_recursive_basic_literal(&body) {
                return;
            }
            let body_kind_here = body.kind();
            match body_kind {
                None => body_kind = Some(body_kind_here),
                Some(previous) if previous != body_kind_here => return,
                Some(_) => {}
            }
        }

        ctx.report(&Self::META, node.span(), MSG);
    }
}

/// A `WhenNode`'s body: a one-statement `StatementsNode` unwraps down to
/// that statement (mirroring whitequark's parser never wrapping a
/// single-statement body in a `begin` node), otherwise the `StatementsNode`
/// itself stands in for whitequark's `begin` node.
fn effective_body<'pr>(statements: &StatementsNode<'pr>) -> Node<'pr> {
    let body = statements.body();
    if body.len() == 1 {
        body.first().expect("checked len == 1")
    } else {
        statements.as_node()
    }
}

/// `Lint::LITERAL_RECURSIVE_METHODS` (rubocop-ast's `Node`): the only
/// `send`-node method names an expression may use and still count as a
/// recursive basic literal -- `COMPARISON_OPERATORS + [*, !, <=>]`.
fn is_literal_recursive_method(name: &[u8]) -> bool {
    matches!(name, b"==" | b"===" | b"!=" | b"<=" | b">=" | b">" | b"<" | b"*" | b"!" | b"<=>")
}

/// rubocop-ast's `Node#recursive_basic_literal?`, extended to also recurse
/// through a `StatementsNode` (this cop's stand-in for whitequark's `begin`
/// node, per [`effective_body`]).
fn is_recursive_basic_literal(node: &Node<'_>) -> bool {
    match node {
        Node::NilNode { .. }
        | Node::TrueNode { .. }
        | Node::FalseNode { .. }
        | Node::IntegerNode { .. }
        | Node::FloatNode { .. }
        | Node::StringNode { .. }
        | Node::SymbolNode { .. }
        | Node::RegularExpressionNode { .. } => true,
        Node::InterpolatedRegularExpressionNode { .. } => node
            .as_interpolated_regular_expression_node()
            .is_some_and(|n| n.parts().iter().all(|p| is_recursive_basic_literal(&p))),
        Node::InterpolatedStringNode { .. } => node
            .as_interpolated_string_node()
            .is_some_and(|n| n.parts().iter().all(|p| is_recursive_basic_literal(&p))),
        Node::InterpolatedSymbolNode { .. } => node
            .as_interpolated_symbol_node()
            .is_some_and(|n| n.parts().iter().all(|p| is_recursive_basic_literal(&p))),
        Node::ArrayNode { .. } => node
            .as_array_node()
            .is_some_and(|n| n.elements().iter().all(|el| is_recursive_basic_literal(&el))),
        Node::HashNode { .. } => {
            node.as_hash_node().is_some_and(|n| pairs_are_literal(&n.elements()))
        }
        Node::KeywordHashNode { .. } => {
            node.as_keyword_hash_node().is_some_and(|n| pairs_are_literal(&n.elements()))
        }
        Node::AndNode { .. } => node.as_and_node().is_some_and(|n| {
            is_recursive_basic_literal(&n.left()) && is_recursive_basic_literal(&n.right())
        }),
        Node::OrNode { .. } => node.as_or_node().is_some_and(|n| {
            is_recursive_basic_literal(&n.left()) && is_recursive_basic_literal(&n.right())
        }),
        Node::RangeNode { .. } => node.as_range_node().is_some_and(|n| {
            n.left().is_none_or(|side| is_recursive_basic_literal(&side))
                && n.right().is_none_or(|side| is_recursive_basic_literal(&side))
        }),
        Node::CallNode { .. } => node.as_call_node().is_some_and(|n| {
            is_literal_recursive_method(n.name().as_slice())
                && n.receiver().is_some_and(|r| is_recursive_basic_literal(&r))
                && match n.arguments() {
                    Some(args) => args.arguments().iter().all(|a| is_recursive_basic_literal(&a)),
                    None => true,
                }
        }),
        Node::ParenthesesNode { .. } => node.as_parentheses_node().is_some_and(|n| {
            n.body().and_then(|b| b.as_statements_node()).is_some_and(|stmts| {
                single_statement(&stmts).is_some_and(|s| is_recursive_basic_literal(&s))
            })
        }),
        Node::EmbeddedStatementsNode { .. } => {
            node.as_embedded_statements_node().is_some_and(|n| {
                n.statements().is_some_and(|stmts| {
                    single_statement(&stmts).is_some_and(|s| is_recursive_basic_literal(&s))
                })
            })
        }
        Node::StatementsNode { .. } => node
            .as_statements_node()
            .is_some_and(|n| n.body().iter().all(|s| is_recursive_basic_literal(&s))),
        _ => false,
    }
}

/// A `StatementsNode` with exactly one statement, standing in for that
/// statement itself.
fn single_statement<'pr>(stmts: &StatementsNode<'pr>) -> Option<Node<'pr>> {
    let body = stmts.body();
    if body.len() == 1 {
        body.first()
    } else {
        None
    }
}

/// `HashNode#pairs`/`#keys`: only `AssocNode` (`pair`) elements count; a
/// `**splat` (`AssocSplatNode`) makes the enclosing literal unrecognized.
fn pairs_are_literal(list: &ruby_ast::node::NodeList<'_>) -> bool {
    list.iter().all(|element| {
        element.as_assoc_node().is_some_and(|assoc| {
            is_recursive_basic_literal(&assoc.key()) && is_recursive_basic_literal(&assoc.value())
        })
    })
}
