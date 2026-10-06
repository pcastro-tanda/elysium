//! `Minitest/SkipEnsure`, ported from rubocop-minitest's
//! `lib/rubocop/cop/minitest/skip_ensure.rb`.
//!
//! whitequark's `ensure` node is a Prism `BeginNode` with an
//! `ensure_clause`, and its `rescue` node is the same `BeginNode`'s
//! `rescue_clause` (plus the `RescueModifierNode`). Upstream's `ancestors`
//! reach all the way to the root, so the tree is walked from the root to the
//! `ensure`.
//!
//! # Approximation
//!
//! `ensure_node.branch.condition == if_node.condition` is whitequark's
//! structural equality; here the two conditions' source text is compared.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::BeginNode;
use ruby_ast::{for_each_child, walk, LocationExt as _, Node, NodeExt as _, NodeKind, Visitor};

const MSG: &str = "`ensure` is called even though the test is skipped.";

/// Checks that `ensure` call even if `skip`.
#[derive(Debug, Clone)]
pub struct SkipEnsure;

impl Rule for SkipEnsure {
    const META: RuleMeta = RuleMeta {
        name: "Minitest/SkipEnsure",
        department: Department::Minitest,
        summary: "Checks that `ensure` call even if `skip`.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::BeginNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(begin) = node.as_begin_node() else { return };
        let Some(ensure) = begin.ensure_clause() else { return };
        let keyword = ensure.ensure_keyword_loc().span();

        let root = ctx.parsed().root();
        let mut finder = PathFinder { keyword_start: keyword.start, stack: Vec::new(), path: None };
        walk(&root, &mut finder);
        let Some(mut path) = finder.path else { return };

        if find_skip(&begin, &mut path).is_none() {
            return;
        }
        // `path` is now root .. ensure node .. skip.
        if use_skip_in_rescue(&path) || valid_conditional_skip(&begin, &path, ctx) {
            return;
        }
        ctx.report(&Self::META, keyword, MSG);
    }
}

/// Collects the nodes from the root down to (and including) the `BeginNode`
/// whose `ensure` keyword starts at `keyword_start`.
struct PathFinder<'pr> {
    keyword_start: u32,
    stack: Vec<Node<'pr>>,
    path: Option<Vec<Node<'pr>>>,
}

impl<'pr> Visitor<'pr> for PathFinder<'pr> {
    fn enter(&mut self, node: &Node<'pr>) {
        self.stack.push(*node);
        if self.path.is_some() {
            return;
        }
        let is_target = node
            .as_begin_node()
            .and_then(|begin| begin.ensure_clause())
            .is_some_and(|ensure| ensure.ensure_keyword_loc().span().start == self.keyword_start);
        if is_target {
            self.path = Some(self.stack.clone());
        }
    }

    fn leave(&mut self, _node: &Node<'pr>) {
        self.stack.pop();
    }
}

/// `body.descendants.detect { |n| n.send_type? && n.receiver.nil? && n.method?(:skip) }`,
/// where `body` is the protected part of the `ensure`. On success `path` is
/// extended with the descent from the `ensure` node to the `skip` call.
fn find_skip<'pr>(begin: &BeginNode<'pr>, path: &mut Vec<Node<'pr>>) -> Option<Node<'pr>> {
    if begin.rescue_clause().is_some() {
        // The body is the `rescue` node: everything but the `ensure` branch is below it.
        let mut found = None;
        for_each_child(&begin.as_node(), |child| {
            if found.is_none() && child.as_ensure_node().is_none() {
                found = descend(child, path);
            }
        });
        return found;
    }
    let statements: Vec<Node<'pr>> = begin.statements()?.body().iter().collect();
    match statements.as_slice() {
        [] => None,
        // A lone statement is the body itself; only its descendants count.
        [single] => {
            path.push(*single);
            let found = search_children(single, path);
            if found.is_none() {
                path.pop();
            }
            found
        }
        many => many.iter().find_map(|stmt| descend(stmt, path)),
    }
}

/// Checks `node` and then its descendants, pre-order.
fn descend<'pr>(node: &Node<'pr>, path: &mut Vec<Node<'pr>>) -> Option<Node<'pr>> {
    path.push(*node);
    if is_skip(node) {
        return Some(*node);
    }
    let found = search_children(node, path);
    if found.is_none() {
        path.pop();
    }
    found
}

fn search_children<'pr>(node: &Node<'pr>, path: &mut Vec<Node<'pr>>) -> Option<Node<'pr>> {
    let mut found = None;
    for_each_child(node, |child| {
        if found.is_none() {
            found = descend(child, path);
        }
    });
    found
}

/// `n.send_type? && n.receiver.nil? && n.method?(:skip)`.
fn is_skip(node: &Node<'_>) -> bool {
    node.as_call_node().is_some_and(|call| {
        !call.is_safe_navigation() && call.receiver().is_none() && call.name().as_slice() == b"skip"
    })
}

/// `skip_method.ancestors.detect(&:rescue_type?)`.
fn use_skip_in_rescue(path: &[Node<'_>]) -> bool {
    path.windows(2).any(|pair| {
        let (ancestor, child) = (&pair[0], &pair[1]);
        ancestor.as_rescue_modifier_node().is_some()
            || ancestor.as_begin_node().is_some_and(|begin| {
                begin.rescue_clause().is_some() && child.as_ensure_node().is_none()
            })
    })
}

/// `if?`: the node is an `if` (modifier or not), not an `unless`/ternary/`elsif`.
fn keyword_is(node: &Node<'_>, ctx: &Context<'_>, keyword: &[u8]) -> Option<bool> {
    if let Some(if_node) = node.as_if_node() {
        let text = if_node.if_keyword_loc().map(|loc| ctx.text(loc.span())).unwrap_or_default();
        return Some(text == keyword);
    }
    node.as_unless_node().map(|_| keyword == b"unless")
}

fn condition<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    node.as_if_node()
        .map(|if_node| if_node.predicate())
        .or_else(|| node.as_unless_node().map(|unless| unless.predicate()))
}

/// `valid_conditional_skip?`.
fn valid_conditional_skip(begin: &BeginNode<'_>, path: &[Node<'_>], ctx: &Context<'_>) -> bool {
    // `skip_method.ancestors.detect(&:if_type?)`
    let Some(if_node) = path[..path.len() - 1]
        .iter()
        .rev()
        .find(|ancestor| ancestor.as_if_node().is_some() || ancestor.as_unless_node().is_some())
    else {
        return false;
    };
    // `ensure_node.branch.if_type?`
    let Some(ensure) = begin.ensure_clause() else { return false };
    let Some(statements) = ensure.statements() else { return false };
    let list: Vec<Node<'_>> = statements.body().iter().collect();
    let [branch] = list.as_slice() else { return false };
    if branch.as_if_node().is_none() && branch.as_unless_node().is_none() {
        return false;
    }

    let branch_is_if = keyword_is(branch, ctx, b"if").unwrap_or(false);
    let match_keyword = if branch_is_if {
        keyword_is(if_node, ctx, b"if")
    } else {
        keyword_is(if_node, ctx, b"unless")
    }
    .unwrap_or(false);
    if !match_keyword {
        return false;
    }
    match (condition(branch), condition(if_node)) {
        (Some(left), Some(right)) => ctx.text(left.span()) == ctx.text(right.span()),
        _ => false,
    }
}
