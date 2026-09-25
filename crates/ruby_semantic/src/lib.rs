//! Lexical scopes and local-variable flow for Ruby, ported from RuboCop's
//! `VariableForce` (`lib/rubocop/cop/variable_force.rb` and
//! `variable_force/*.rb`, 1.82.1) onto Prism nodes.
//!
//! The model is arena-style and id-indexed: [`Semantics`] owns every
//! [`Scope`], [`Variable`], [`Assignment`] and [`Branch`], and hands out
//! `…Id` handles. Records borrow the tree ([`Node<'pr>`] is `Copy`), so a
//! `Semantics<'pr>` lives exactly as long as the [`ruby_ast::Parsed`] it was
//! built from.
//!
//! Node identity is `(NodeKind, Span)` ([`NodeId`]): Prism hands out fresh
//! `Node` values for the same underlying node on every accessor call, so
//! pointer identity is not available and two distinct nodes never share both
//! a kind and a byte range.
//!
//! See `docs/adr/0007-lazy-semantic-traversal.md` for why this is built by a
//! second traversal rather than folded into the single rule walk.

use ruby_ast::{Node, NodeKind};
use ruby_source::Span;

mod branch;
mod builder;
mod nodes;

pub use branch::{Branch, BranchKind};
pub use nodes::{node_id, same, NodeId};

macro_rules! ids {
    ($($(#[$m:meta])* $name:ident),* $(,)?) => {$(
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(u32);

        impl $name {
            /// The handle's index into its arena.
            #[must_use]
            pub const fn index(self) -> usize {
                self.0 as usize
            }
        }
    )*};
}

ids! {
    /// Handle to a [`Scope`].
    ScopeId,
    /// Handle to a [`Variable`].
    VariableId,
    /// Handle to an [`Assignment`].
    AssignmentId,
    /// Handle to a [`Branch`].
    BranchId,
}

/// A contiguous slice of [`Semantics::ancestor_pool`]: the ancestors of one
/// record's node, outermost first. Kept as a range so the common case (many
/// short chains) costs one shared `Vec` instead of a `Box<[Node]>` each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AncestorRange {
    start: u32,
    len: u32,
}

/// What kind of Ruby construct a [`Scope`] stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    /// The whole file (`ProgramNode`); RuboCop's `naked_top_level`.
    TopLevel,
    /// `def` or `def self.`.
    Def,
    /// A block or a lambda literal (parser's `block`/`numblock`/`itblock`).
    Block,
    /// `class Foo`.
    Class,
    /// `module Foo`.
    Module,
    /// `class << foo`.
    SingletonClass,
}

impl ScopeKind {
    /// RuboCop's `node.any_block_type?`: only block scopes let inner code
    /// reach an outer scope's locals.
    #[must_use]
    pub const fn is_block(self) -> bool {
        matches!(self, Self::Block)
    }
}

/// How a local variable came into existence, mirroring the parser node types
/// RuboCop's `Variable::VARIABLE_DECLARATION_TYPES` accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclKind {
    /// `arg`
    RequiredArg,
    /// `optarg`
    OptionalArg,
    /// `restarg`
    RestArg,
    /// `kwarg`
    KeywordArg,
    /// `kwoptarg`
    OptionalKeywordArg,
    /// `kwrestarg`
    KeywordRestArg,
    /// `blockarg` (a block-pass parameter, `&block`)
    BlockArg,
    /// `shadowarg` (a block-local variable, `{ |a; this| }`)
    BlockLocal,
    /// `lvasgn`
    Assignment,
    /// `match_with_lvasgn`
    RegexpNamedCapture,
    /// `match_var`
    PatternMatch,
}

impl DeclKind {
    /// RuboCop's `Variable#argument?`.
    #[must_use]
    pub const fn is_argument(self) -> bool {
        matches!(
            self,
            Self::RequiredArg
                | Self::OptionalArg
                | Self::RestArg
                | Self::KeywordArg
                | Self::OptionalKeywordArg
                | Self::KeywordRestArg
                | Self::BlockArg
                | Self::BlockLocal
        )
    }
}

/// A place local variables live in: a file, a method body, a block body, or
/// a class/module/singleton-class body.
#[derive(Debug, Clone)]
pub struct Scope<'pr> {
    node: Node<'pr>,
    parent: Option<ScopeId>,
    kind: ScopeKind,
    variables: Vec<VariableId>,
    bare_call_names: Vec<&'pr [u8]>,
    ancestors: AncestorRange,
}

impl<'pr> Scope<'pr> {
    /// The node that opens the scope.
    #[must_use]
    pub const fn node(&self) -> Node<'pr> {
        self.node
    }

    /// The enclosing scope, if any.
    #[must_use]
    pub const fn parent(&self) -> Option<ScopeId> {
        self.parent
    }

    /// What construct this scope stands for.
    #[must_use]
    pub const fn kind(&self) -> ScopeKind {
        self.kind
    }

    /// Every variable declared here, in declaration order. A name declared
    /// twice keeps its first position but resolves to the later variable,
    /// matching `VariableTable`'s `Hash#[]=`.
    #[must_use]
    pub fn variables(&self) -> &[VariableId] {
        &self.variables
    }

    /// Names of every receiverless, argumentless call written directly in
    /// this scope, in source order and deduplicated: the "variable-like
    /// method invocations" half of `UselessAssignment`'s
    /// `collect_variable_like_names`.
    #[must_use]
    pub fn bare_call_names(&self) -> &[&'pr [u8]] {
        &self.bare_call_names
    }

    /// RuboCop's `Scope#body_node`.
    #[must_use]
    pub fn body(&self) -> Option<Node<'pr>> {
        match self.node {
            Node::ProgramNode { .. } => Some(self.node.as_program_node()?.statements().as_node()),
            Node::DefNode { .. } => self.node.as_def_node()?.body(),
            Node::BlockNode { .. } => self.node.as_block_node()?.body(),
            Node::LambdaNode { .. } => self.node.as_lambda_node()?.body(),
            Node::ClassNode { .. } => self.node.as_class_node()?.body(),
            Node::ModuleNode { .. } => self.node.as_module_node()?.body(),
            Node::SingletonClassNode { .. } => self.node.as_singleton_class_node()?.body(),
            _ => None,
        }
    }

    /// RuboCop's `UselessAssignment#return_value_node_of_scope`: the last
    /// statement of the scope's body, or the body itself when it is not a
    /// statement list.
    #[must_use]
    pub fn return_value_node(&self) -> Option<Node<'pr>> {
        match self.body()? {
            body @ Node::StatementsNode { .. } => body.as_statements_node()?.body().iter().last(),
            body => Some(body),
        }
    }
}

/// One local variable: a declaration plus everything that happened to it.
#[derive(Debug, Clone)]
pub struct Variable<'pr> {
    name: &'pr [u8],
    declaration: Node<'pr>,
    decl_kind: DeclKind,
    scope: ScopeId,
    assignments: Vec<AssignmentId>,
    references: Vec<Reference<'pr>>,
    captured_by_block: bool,
    shadows: Option<VariableId>,
    ancestors: AncestorRange,
}

impl<'pr> Variable<'pr> {
    /// The variable's name.
    #[must_use]
    pub const fn name(&self) -> &'pr [u8] {
        self.name
    }

    /// The node that declared it: a parameter node, or the first assignment
    /// that introduced the name.
    #[must_use]
    pub const fn declaration(&self) -> Node<'pr> {
        self.declaration
    }

    /// How it was declared.
    #[must_use]
    pub const fn decl_kind(&self) -> DeclKind {
        self.decl_kind
    }

    /// The scope it belongs to.
    #[must_use]
    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    /// Every assignment to it, in source order.
    #[must_use]
    pub fn assignments(&self) -> &[AssignmentId] {
        &self.assignments
    }

    /// Every reference to it, in source order.
    #[must_use]
    pub fn references(&self) -> &[Reference<'pr>] {
        &self.references
    }

    /// RuboCop's `Variable#captured_by_block?`.
    #[must_use]
    pub const fn captured_by_block(&self) -> bool {
        self.captured_by_block
    }

    /// The variable this one hid at declaration time: `find_variable(name)`
    /// as `ShadowingOuterLocalVariable#before_declaring_variable` sees it.
    #[must_use]
    pub const fn shadows(&self) -> Option<VariableId> {
        self.shadows
    }

    /// RuboCop's `Variable#referenced?`.
    #[must_use]
    pub fn referenced(&self) -> bool {
        !self.references.is_empty()
    }

    /// RuboCop's `Variable#used?`.
    #[must_use]
    pub fn used(&self) -> bool {
        self.captured_by_block || self.referenced()
    }

    /// RuboCop's `Variable#should_be_unused?`.
    #[must_use]
    pub fn should_be_unused(&self) -> bool {
        self.name.first() == Some(&b'_')
    }

    /// RuboCop's `Variable#argument?`.
    #[must_use]
    pub const fn is_argument(&self) -> bool {
        self.decl_kind.is_argument()
    }

    /// RuboCop's `Variable#keyword_argument?`.
    #[must_use]
    pub const fn is_keyword_argument(&self) -> bool {
        matches!(self.decl_kind, DeclKind::KeywordArg | DeclKind::OptionalKeywordArg)
    }

    /// RuboCop's `Variable#explicit_block_local_variable?`.
    #[must_use]
    pub const fn is_explicit_block_local(&self) -> bool {
        matches!(self.decl_kind, DeclKind::BlockLocal)
    }
}

/// One assignment to a local variable.
#[derive(Debug, Clone)]
pub struct Assignment<'pr> {
    node: Node<'pr>,
    variable: VariableId,
    name_span: Span,
    referenced: bool,
    references: Vec<Node<'pr>>,
    reassigned: bool,
    branch: Option<BranchId>,
    meta: Option<Meta<'pr>>,
    operator: Option<Span>,
    exception: bool,
    ancestors: AncestorRange,
}

impl<'pr> Assignment<'pr> {
    /// The assigning node.
    #[must_use]
    pub const fn node(&self) -> Node<'pr> {
        self.node
    }

    /// The variable assigned to.
    #[must_use]
    pub const fn variable(&self) -> VariableId {
        self.variable
    }

    /// The range RuboCop reports: `node.loc.name`, or the regexp literal for
    /// a named capture.
    #[must_use]
    pub const fn name_span(&self) -> Span {
        self.name_span
    }

    /// RuboCop's `Assignment#referenced?`.
    #[must_use]
    pub const fn referenced(&self) -> bool {
        self.referenced
    }

    /// Nodes that referenced this assignment.
    #[must_use]
    pub fn references(&self) -> &[Node<'pr>] {
        &self.references
    }

    /// RuboCop's `Assignment#reassigned?`.
    #[must_use]
    pub const fn reassigned(&self) -> bool {
        self.reassigned
    }

    /// The conditional branch this assignment sits in, if any.
    #[must_use]
    pub const fn branch(&self) -> Option<BranchId> {
        self.branch
    }

    /// The enclosing assignment construct, RuboCop's `meta_assignment_node`.
    #[must_use]
    pub const fn meta(&self) -> Option<Meta<'pr>> {
        self.meta
    }

    /// RuboCop's `Assignment#exception_assignment?`.
    #[must_use]
    pub const fn is_exception(&self) -> bool {
        self.exception
    }

    /// RuboCop's `Assignment#regexp_named_capture?`.
    #[must_use]
    pub fn is_regexp_named_capture(&self) -> bool {
        matches!(self.node, Node::MatchWriteNode { .. })
    }

    /// RuboCop's `Assignment#operator_assignment?`.
    #[must_use]
    pub const fn is_operator_assignment(&self) -> bool {
        matches!(self.meta, Some(Meta::Operator))
    }

    /// RuboCop's `Assignment#multiple_assignment?`.
    #[must_use]
    pub const fn is_multiple_assignment(&self) -> bool {
        matches!(self.meta, Some(Meta::Multiple(_)))
    }

    /// RuboCop's `Assignment#rest_assignment?`.
    #[must_use]
    pub const fn is_rest_assignment(&self) -> bool {
        matches!(self.meta, Some(Meta::Rest(_)))
    }

    /// RuboCop's `Assignment#for_assignment?`.
    #[must_use]
    pub const fn is_for_assignment(&self) -> bool {
        matches!(self.meta, Some(Meta::For(_)))
    }

    /// RuboCop's `Assignment#operator`: the source range of the assignment
    /// operator (`+=`, `||=`, `=`). `None` for the `for` and bare-splat
    /// targets, where RuboCop's own accessor would raise.
    #[must_use]
    pub const fn operator_span(&self) -> Option<Span> {
        self.operator
    }
}

/// The enclosing construct an assignment target belongs to, RuboCop's
/// `Assignment#meta_assignment_node`.
#[derive(Debug, Clone, Copy)]
pub enum Meta<'pr> {
    /// `foo += 1`, `foo ||= 1`, `foo &&= 1`.
    Operator,
    /// A target of a `MultiWriteNode`.
    Multiple(Node<'pr>),
    /// A bare splat target that is not part of a multiple assignment.
    Rest(Node<'pr>),
    /// The index of a `for` loop.
    For(Node<'pr>),
}

/// One read of a local variable.
#[derive(Debug, Clone, Copy)]
pub struct Reference<'pr> {
    node: Node<'pr>,
    branch: Option<BranchId>,
    explicit: bool,
}

impl<'pr> Reference<'pr> {
    /// The referencing node.
    #[must_use]
    pub const fn node(&self) -> Node<'pr> {
        self.node
    }

    /// The conditional branch the reference sits in, if any.
    #[must_use]
    pub const fn branch(&self) -> Option<BranchId> {
        self.branch
    }

    /// RuboCop's `Reference#explicit?`: false for the implicit reads a
    /// zero-arity `super` or a `binding` call performs.
    #[must_use]
    pub const fn explicit(&self) -> bool {
        self.explicit
    }
}

/// Every scope, variable, assignment and branch of one parsed file.
#[derive(Debug)]
pub struct Semantics<'pr> {
    scopes: Vec<Scope<'pr>>,
    variables: Vec<Variable<'pr>>,
    assignments: Vec<Assignment<'pr>>,
    branches: Vec<Branch<'pr>>,
    ancestor_pool: Vec<Node<'pr>>,
    leave_order: Vec<ScopeId>,
}

impl<'pr> Semantics<'pr> {
    /// Walks `root` (a `ProgramNode`) and builds the whole model.
    #[must_use]
    pub fn build(root: &Node<'pr>) -> Self {
        builder::build(*root)
    }

    /// Every scope, in the order they were entered (pre-order).
    #[must_use]
    pub fn scopes(&self) -> &[Scope<'pr>] {
        &self.scopes
    }

    /// Every scope id in the order RuboCop's `after_leaving_scope` fires:
    /// post-order, inner scopes before the scopes that contain them.
    #[must_use]
    pub fn leave_order(&self) -> &[ScopeId] {
        &self.leave_order
    }

    /// The top-level scope, which always exists.
    #[must_use]
    pub const fn top_level(&self) -> ScopeId {
        ScopeId(0)
    }

    /// Looks a scope up by id.
    #[must_use]
    pub fn scope(&self, id: ScopeId) -> &Scope<'pr> {
        &self.scopes[id.index()]
    }

    /// Every variable, in declaration order.
    #[must_use]
    pub fn variables(&self) -> &[Variable<'pr>] {
        &self.variables
    }

    /// Every variable id, in declaration order.
    pub fn variable_ids(&self) -> impl Iterator<Item = VariableId> + use<'_, 'pr> {
        (0..self.variables.len()).map(|i| VariableId(u32::try_from(i).unwrap_or(u32::MAX)))
    }

    /// Looks a variable up by id.
    #[must_use]
    pub fn variable(&self, id: VariableId) -> &Variable<'pr> {
        &self.variables[id.index()]
    }

    /// Looks an assignment up by id.
    #[must_use]
    pub fn assignment(&self, id: AssignmentId) -> &Assignment<'pr> {
        &self.assignments[id.index()]
    }

    /// Looks a branch up by id.
    #[must_use]
    pub fn branch(&self, id: BranchId) -> &Branch<'pr> {
        &self.branches[id.index()]
    }

    /// Ancestors of a scope's node, outermost first.
    #[must_use]
    pub fn scope_ancestors(&self, id: ScopeId) -> &[Node<'pr>] {
        self.slice(self.scopes[id.index()].ancestors)
    }

    /// Ancestors of a variable's declaration node, outermost first.
    #[must_use]
    pub fn variable_ancestors(&self, id: VariableId) -> &[Node<'pr>] {
        self.slice(self.variables[id.index()].ancestors)
    }

    /// Ancestors of an assignment's node, outermost first.
    #[must_use]
    pub fn assignment_ancestors(&self, id: AssignmentId) -> &[Node<'pr>] {
        self.slice(self.assignments[id.index()].ancestors)
    }

    fn slice(&self, range: AncestorRange) -> &[Node<'pr>] {
        let start = range.start as usize;
        &self.ancestor_pool[start..start + range.len as usize]
    }

    /// RuboCop's `Variable#method_argument?`.
    #[must_use]
    pub fn is_method_argument(&self, id: VariableId) -> bool {
        let variable = self.variable(id);
        variable.is_argument() && self.scope(variable.scope).kind == ScopeKind::Def
    }

    /// RuboCop's `Variable#block_argument?`.
    #[must_use]
    pub fn is_block_argument(&self, id: VariableId) -> bool {
        let variable = self.variable(id);
        variable.is_argument() && self.scope(variable.scope).kind == ScopeKind::Block
    }

    /// RuboCop's `Assignment#used?`.
    #[must_use]
    pub fn assignment_used(&self, id: AssignmentId) -> bool {
        let assignment = self.assignment(id);
        (!assignment.reassigned && self.variable(assignment.variable).captured_by_block)
            || assignment.referenced
    }

    /// RuboCop's `Branch::Base#exclusive_with?`.
    #[must_use]
    pub fn exclusive_with(&self, this: BranchId, other: BranchId) -> bool {
        let mut current = Some(this);
        while let Some(id) = current {
            let branch = self.branch(id);
            if branch.may_jump_to_other_branch() {
                return false;
            }
            let mut probe = Some(other);
            while let Some(other_id) = probe {
                let other_branch = self.branch(other_id);
                if branch.control_id() == other_branch.control_id() {
                    return branch.child_id() != other_branch.child_id();
                }
                probe = other_branch.parent();
            }
            current = branch.parent();
        }
        false
    }

    /// RuboCop's `Branchable#run_exclusively_with?`.
    #[must_use]
    pub fn run_exclusively(&self, this: Option<BranchId>, other: Option<BranchId>) -> bool {
        match (this, other) {
            (Some(a), Some(b)) => self.exclusive_with(a, b),
            _ => false,
        }
    }
}

/// True when `kind` opens a new local-variable scope.
#[must_use]
pub const fn is_scope_kind(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::ProgramNode
            | NodeKind::DefNode
            | NodeKind::BlockNode
            | NodeKind::LambdaNode
            | NodeKind::ClassNode
            | NodeKind::ModuleNode
            | NodeKind::SingletonClassNode
    )
}

#[cfg(test)]
mod tests;
