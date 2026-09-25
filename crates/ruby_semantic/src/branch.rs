//! Conditional branches, ported from RuboCop's
//! `lib/rubocop/cop/variable_force/branch.rb`.
//!
//! A branch is a `(control node, child node)` pair: the child is one of the
//! control structure's alternatives, and code inside it only conditionally
//! runs. RuboCop never returns an "always run" branch (an `if` condition, a
//! `for` element, an `ensure` body): `Branch.of` skips those and keeps
//! climbing, so only conditional branches are ever recorded here.
//!
//! Prism's tree differs from whitequark's in three places that matter:
//!
//! * `rescue` clauses form a `subsequent` chain hanging off the `BeginNode`
//!   instead of being flat children of a `rescue` node, so each `RescueNode`
//!   in the chain is treated as a direct child of its `BeginNode`.
//! * `begin`/`rescue`/`else`/`ensure` are one `BeginNode` rather than
//!   parser's nested `(ensure (rescue ...))`, so a `BeginNode` carrying both
//!   produces two branches on the same control node: the `Rescue` one, whose
//!   parent is an `Ensure` branch whose child is the `BeginNode` itself
//!   (standing in for parser's inner `rescue` node).
//! * `StatementsNode`, `ElseNode` and `ParenthesesNode` have no parser
//!   counterpart, but none of them is a control node, so the climb simply
//!   passes through them.

use ruby_ast::Node;

use crate::nodes::{node_id, NodeId};
use crate::BranchId;

/// Which control structure a branch belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchKind {
    /// `if`/`unless`/ternary.
    If,
    /// `while`/`until`, in either pre- or post-condition form.
    While,
    /// `case`/`when`.
    Case,
    /// `case`/`in`.
    CaseMatch,
    /// `for`.
    For,
    /// `&&`/`||`.
    LogicalOperator,
    /// `||=`/`&&=`/`op=` (RuboCop ≥ 1.84, #14796): the right-hand side
    /// only runs conditionally.
    OperatorAssignment,
    /// The `begin`/`rescue`/`else` part of a `BeginNode`.
    Rescue,
    /// The `begin`/`ensure` part of a `BeginNode`.
    Ensure,
}

/// One conditional branch of one control structure.
// `may_jump_to_other_branch` keeps RuboCop's own predicate name.
#[allow(clippy::struct_field_names)]
#[derive(Debug, Clone)]
pub struct Branch<'pr> {
    control: Node<'pr>,
    child: Node<'pr>,
    control_id: NodeId,
    child_id: NodeId,
    kind: BranchKind,
    parent: Option<BranchId>,
    may_jump_to_other_branch: bool,
    may_run_incompletely: bool,
}

impl<'pr> Branch<'pr> {
    pub(crate) fn new(
        control: Node<'pr>,
        child: Node<'pr>,
        kind: BranchKind,
        parent: Option<BranchId>,
        may_jump_to_other_branch: bool,
        may_run_incompletely: bool,
    ) -> Self {
        Self {
            control_id: node_id(&control),
            child_id: node_id(&child),
            control,
            child,
            kind,
            parent,
            may_jump_to_other_branch,
            may_run_incompletely,
        }
    }

    /// The control structure node.
    #[must_use]
    pub const fn control(&self) -> Node<'pr> {
        self.control
    }

    /// The alternative of the control structure this branch stands for.
    #[must_use]
    pub const fn child(&self) -> Node<'pr> {
        self.child
    }

    /// Identity of [`Branch::control`].
    #[must_use]
    pub const fn control_id(&self) -> NodeId {
        self.control_id
    }

    /// Identity of [`Branch::child`].
    #[must_use]
    pub const fn child_id(&self) -> NodeId {
        self.child_id
    }

    /// Which control structure this is.
    #[must_use]
    pub const fn kind(&self) -> BranchKind {
        self.kind
    }

    /// The branch that contains this one, if any.
    #[must_use]
    pub const fn parent(&self) -> Option<BranchId> {
        self.parent
    }

    /// RuboCop's `may_jump_to_other_branch?`: true for the main body of an
    /// exception handler, which can abandon itself for a `rescue` clause.
    #[must_use]
    pub const fn may_jump_to_other_branch(&self) -> bool {
        self.may_jump_to_other_branch
    }

    /// RuboCop's `may_run_incompletely?`: same condition, used to decide
    /// whether a branch can be "consumed" while resolving a reference.
    #[must_use]
    pub const fn may_run_incompletely(&self) -> bool {
        self.may_run_incompletely
    }
}
