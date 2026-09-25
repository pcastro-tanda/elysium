//! The traversal that fills a [`Semantics`], handler for handler with
//! RuboCop's `VariableForce` (`lib/rubocop/cop/variable_force.rb`).
//!
//! RuboCop drives its walk off `Parser::AST::Node#each_child_node` plus a
//! `scanned_nodes` set to keep "twisted" children (a `class`'s superclass, a
//! block's own send) from being visited twice. Prism's tree makes that set
//! unnecessary: child order is explicit here, and a block's send is its
//! *parent* rather than its first child, so the twisting falls out of the
//! natural order. Everything else -- post-condition loops running their body
//! first, `for` evaluating its collection first, multiple assignment walking
//! its value before its targets, operator assignment referencing before its
//! right-hand side, pattern variables declaring without assigning -- is
//! reproduced explicitly.

use std::collections::{HashMap, HashSet};

use ruby_ast::{each_descendant, for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use crate::branch::{Branch, BranchKind};
use crate::nodes::{node_id, same, same_opt, NodeId};
use crate::{
    AncestorRange, Assignment, AssignmentId, BranchId, DeclKind, Meta, Reference, Scope, ScopeId,
    ScopeKind, Semantics, Variable, VariableId,
};

/// Builds the model for a whole file. `root` must be the `ProgramNode`.
pub(crate) fn build(root: Node<'_>) -> Semantics<'_> {
    let mut builder = Builder::new();
    builder.push_scope(root, ScopeKind::TopLevel);
    builder.with_node(root, |b| {
        for child in scope_children(&root).into_iter().flatten() {
            b.process(child);
        }
    });
    builder.pop_scope();
    builder.collect_bare_call_names(root);
    builder.sem
}

/// A branch before it is interned, so the chain can be walked
/// innermost-first and linked outermost-first.
#[derive(Debug, Clone, Copy)]
struct RawBranch<'pr> {
    control: Node<'pr>,
    child: Node<'pr>,
    kind: BranchKind,
    may_jump: bool,
    may_run_incompletely: bool,
}

struct Builder<'pr> {
    sem: Semantics<'pr>,
    scope_stack: Vec<ScopeId>,
    ancestors: Vec<Node<'pr>>,
    branch_index: HashMap<(NodeId, NodeId), BranchId>,
    /// Targets that look like `lvasgn` but are not: pattern-match variables
    /// (`match_var`) and the capture targets of a `MatchWriteNode`.
    non_assignment_targets: HashSet<NodeId>,
    in_pattern: bool,
    scratch: Vec<RawBranch<'pr>>,
}

impl<'pr> Builder<'pr> {
    fn new() -> Self {
        Self {
            sem: Semantics {
                scopes: Vec::new(),
                variables: Vec::new(),
                assignments: Vec::new(),
                branches: Vec::new(),
                ancestor_pool: Vec::new(),
                leave_order: Vec::new(),
            },
            scope_stack: Vec::new(),
            ancestors: Vec::with_capacity(32),
            branch_index: HashMap::new(),
            non_assignment_targets: HashSet::new(),
            in_pattern: false,
            scratch: Vec::new(),
        }
    }

    // ---------------------------------------------------------------- utils

    fn with_node(&mut self, node: Node<'pr>, f: impl FnOnce(&mut Self)) {
        self.ancestors.push(node);
        f(self);
        self.ancestors.pop();
    }

    fn walk_children(&mut self, node: Node<'pr>) {
        self.ancestors.push(node);
        for_each_child(&node, |child| self.process(*child));
        self.ancestors.pop();
    }

    fn record_ancestors(&mut self) -> AncestorRange {
        let start = u32::try_from(self.sem.ancestor_pool.len()).unwrap_or(u32::MAX);
        self.sem.ancestor_pool.extend_from_slice(&self.ancestors);
        AncestorRange { start, len: u32::try_from(self.ancestors.len()).unwrap_or(u32::MAX) }
    }

    fn current_scope(&self) -> ScopeId {
        *self.scope_stack.last().expect("a scope is always open")
    }

    // --------------------------------------------------------------- scopes

    fn push_scope(&mut self, node: Node<'pr>, kind: ScopeKind) {
        let ancestors = self.record_ancestors();
        let parent = self.scope_stack.last().copied();
        let id = ScopeId(u32::try_from(self.sem.scopes.len()).unwrap_or(u32::MAX));
        self.sem.scopes.push(Scope {
            node,
            parent,
            kind,
            variables: Vec::new(),
            bare_call_names: Vec::new(),
            ancestors,
        });
        self.scope_stack.push(id);
    }

    fn pop_scope(&mut self) {
        let id = self.scope_stack.pop().expect("balanced scopes");
        self.sem.leave_order.push(id);
    }

    // ------------------------------------------------------- variable table

    /// RuboCop's `VariableTable#find_variable`: outward through the scope
    /// stack, stopping after the first scope that is not a block.
    fn find_variable(&self, name: &[u8]) -> Option<VariableId> {
        for &sid in self.scope_stack.iter().rev() {
            let scope = &self.sem.scopes[sid.index()];
            let found =
                scope.variables.iter().rev().find(|v| self.sem.variables[v.index()].name == name);
            if let Some(&vid) = found {
                return Some(vid);
            }
            if !scope.kind.is_block() {
                return None;
            }
        }
        None
    }

    /// RuboCop's `VariableTable#accessible_variables`.
    fn accessible_variables(&self) -> Vec<VariableId> {
        let mut out = Vec::new();
        for &sid in self.scope_stack.iter().rev() {
            let scope = &self.sem.scopes[sid.index()];
            out.extend_from_slice(&scope.variables);
            if !scope.kind.is_block() {
                break;
            }
        }
        out
    }

    fn declare(&mut self, name: &'pr [u8], node: Node<'pr>, decl_kind: DeclKind) {
        let shadows = self.find_variable(name);
        let scope = self.current_scope();
        let ancestors = self.record_ancestors();
        let id = VariableId(u32::try_from(self.sem.variables.len()).unwrap_or(u32::MAX));
        self.sem.variables.push(Variable {
            name,
            declaration: node,
            decl_kind,
            scope,
            assignments: Vec::new(),
            references: Vec::new(),
            captured_by_block: false,
            shadows,
            ancestors,
        });
        // `current_scope.variables[name] = variable`: a repeated name keeps
        // its original position but resolves to the newer variable.
        let slot = self.sem.scopes[scope.index()]
            .variables
            .iter()
            .position(|v| self.sem.variables[v.index()].name == name);
        match slot {
            Some(index) => self.sem.scopes[scope.index()].variables[index] = id,
            None => self.sem.scopes[scope.index()].variables.push(id),
        }
    }

    fn declare_if_absent(&mut self, name: &'pr [u8], node: Node<'pr>, decl_kind: DeclKind) {
        if self.find_variable(name).is_none() {
            self.declare(name, node, decl_kind);
        }
    }

    /// RuboCop's `mark_variable_as_captured_by_block_if_so`.
    fn mark_captured(&mut self, vid: VariableId) {
        let current = self.current_scope();
        if !self.sem.scopes[current.index()].kind.is_block() {
            return;
        }
        if self.sem.variables[vid.index()].scope == current {
            return;
        }
        self.sem.variables[vid.index()].captured_by_block = true;
    }

    /// RuboCop's `VariableTable#assign_to_variable` plus `Variable#assign`.
    fn assign(
        &mut self,
        name: &[u8],
        node: Node<'pr>,
        name_span: Span,
        meta: Option<Meta<'pr>>,
        exception: bool,
    ) {
        let Some(vid) = self.find_variable(name) else { return };
        self.mark_captured(vid);
        let operator = operator_span(&node, meta.as_ref());
        let branch = self.branch_of(&node);
        let ancestors = self.record_ancestors();

        // `mark_last_as_reassigned!`
        if !self.sem.variables[vid.index()].captured_by_block {
            if let Some(&last) = self.sem.variables[vid.index()].assignments.last() {
                let previous = &mut self.sem.assignments[last.index()];
                if previous.branch == branch && !previous.referenced {
                    previous.reassigned = true;
                }
            }
        }

        let id = AssignmentId(u32::try_from(self.sem.assignments.len()).unwrap_or(u32::MAX));
        self.sem.assignments.push(Assignment {
            node,
            variable: vid,
            name_span,
            referenced: false,
            references: Vec::new(),
            reassigned: false,
            branch,
            meta,
            operator,
            exception,
            ancestors,
        });
        self.sem.variables[vid.index()].assignments.push(id);
    }

    fn reference_name(&mut self, name: &[u8], node: Node<'pr>, explicit: bool) {
        let Some(vid) = self.find_variable(name) else { return };
        self.mark_captured(vid);
        self.reference_variable(vid, node, explicit);
    }

    /// RuboCop's `Variable#reference!`, branch consumption included.
    fn reference_variable(&mut self, vid: VariableId, node: Node<'pr>, explicit: bool) {
        let reference_branch = self.branch_of(&node);
        self.sem.variables[vid.index()].references.push(Reference {
            node,
            branch: reference_branch,
            explicit,
        });

        let assignments = std::mem::take(&mut self.sem.variables[vid.index()].assignments);
        let mut consumed: Vec<BranchId> = Vec::new();
        for &aid in assignments.iter().rev() {
            let branch = self.sem.assignments[aid.index()].branch;
            if branch.is_some_and(|b| consumed.contains(&b)) {
                continue;
            }
            if !self.sem.run_exclusively(branch, reference_branch) {
                let assignment = &mut self.sem.assignments[aid.index()];
                assignment.referenced = true;
                assignment.references.push(node);
            }
            // Assignments made in a modifier condition do not put the
            // variable in scope to the left of the keyword, so a preceding
            // assignment still has to be reached.
            if self.in_modifier_conditional(aid) {
                continue;
            }
            let Some(branch) = branch else { break };
            if Some(branch) == reference_branch {
                break;
            }
            if !self.sem.branches[branch.index()].may_run_incompletely() {
                consumed.push(branch);
            }
        }
        self.sem.variables[vid.index()].assignments = assignments;
    }

    /// RuboCop's `Variable#in_modifier_conditional?`. Prism spells parser's
    /// single `begin` wrapper as a `StatementsNode` optionally inside a
    /// `ParenthesesNode`, so up to two levels are skipped.
    fn in_modifier_conditional(&self, aid: AssignmentId) -> bool {
        let ancestors = self.sem.slice(self.sem.assignments[aid.index()].ancestors);
        let mut index = ancestors.len();
        if index == 0 {
            return false;
        }
        index -= 1;
        if ancestors[index].kind() == NodeKind::StatementsNode {
            if index == 0 {
                return false;
            }
            index -= 1;
        }
        if ancestors[index].kind() == NodeKind::ParenthesesNode {
            if index == 0 {
                return false;
            }
            index -= 1;
        }
        is_modifier_conditional(&ancestors[index])
    }

    // -------------------------------------------------------------- branches

    fn branch_of(&mut self, node: &Node<'pr>) -> Option<BranchId> {
        let mut scratch = std::mem::take(&mut self.scratch);
        scratch.clear();
        self.collect_branch_specs(node, &mut scratch);
        let mut parent: Option<BranchId> = None;
        for raw in scratch.iter().rev() {
            parent = Some(self.intern_branch(*raw, parent));
        }
        self.scratch = scratch;
        parent
    }

    fn intern_branch(&mut self, raw: RawBranch<'pr>, parent: Option<BranchId>) -> BranchId {
        let key = (node_id(&raw.control), node_id(&raw.child));
        if let Some(&id) = self.branch_index.get(&key) {
            return id;
        }
        let id = BranchId(u32::try_from(self.sem.branches.len()).unwrap_or(u32::MAX));
        self.sem.branches.push(Branch::new(
            raw.control,
            raw.child,
            raw.kind,
            parent,
            raw.may_jump,
            raw.may_run_incompletely,
        ));
        self.branch_index.insert(key, id);
        id
    }

    /// RuboCop's `Branch.of`, unrolled: the whole chain from `node` up to the
    /// enclosing scope, innermost first, skipping the alternatives that
    /// always run.
    // One arm per control-structure node kind; splitting would only hide
    // the dispatch.
    #[allow(clippy::too_many_lines)]
    fn collect_branch_specs(&self, node: &Node<'pr>, out: &mut Vec<RawBranch<'pr>>) {
        let scope = node_id(&self.sem.scopes[self.current_scope().index()].node);
        let last = self.ancestors.len();
        let mut index = last;
        loop {
            if index == 0 {
                return;
            }
            let child = if index == last { *node } else { self.ancestors[index] };
            if node_id(&child) == scope {
                return;
            }
            let parent = self.ancestors[index - 1];
            match parent.kind() {
                NodeKind::IfNode => {
                    let control = parent.as_if_node().expect("kind matched");
                    if !same(&control.predicate(), &child) {
                        out.push(simple(parent, child, BranchKind::If));
                    }
                    index -= 1;
                }
                NodeKind::UnlessNode => {
                    let control = parent.as_unless_node().expect("kind matched");
                    if !same(&control.predicate(), &child) {
                        out.push(simple(parent, child, BranchKind::If));
                    }
                    index -= 1;
                }
                NodeKind::WhileNode => {
                    let control = parent.as_while_node().expect("kind matched");
                    if !same(&control.predicate(), &child) {
                        out.push(simple(parent, child, BranchKind::While));
                    }
                    index -= 1;
                }
                NodeKind::UntilNode => {
                    let control = parent.as_until_node().expect("kind matched");
                    if !same(&control.predicate(), &child) {
                        out.push(simple(parent, child, BranchKind::While));
                    }
                    index -= 1;
                }
                NodeKind::CaseNode => {
                    let control = parent.as_case_node().expect("kind matched");
                    if !same_opt(control.predicate().as_ref(), &child) {
                        out.push(simple(parent, child, BranchKind::Case));
                    }
                    index -= 1;
                }
                NodeKind::CaseMatchNode => {
                    let control = parent.as_case_match_node().expect("kind matched");
                    if !same_opt(control.predicate().as_ref(), &child) {
                        out.push(simple(parent, child, BranchKind::CaseMatch));
                    }
                    index -= 1;
                }
                NodeKind::ForNode => {
                    let control = parent.as_for_node().expect("kind matched");
                    if !same(&control.index(), &child) && !same(&control.collection(), &child) {
                        out.push(simple(parent, child, BranchKind::For));
                    }
                    index -= 1;
                }
                NodeKind::AndNode => {
                    let control = parent.as_and_node().expect("kind matched");
                    if !same(&control.left(), &child) {
                        out.push(simple(parent, child, BranchKind::LogicalOperator));
                    }
                    index -= 1;
                }
                NodeKind::OrNode => {
                    let control = parent.as_or_node().expect("kind matched");
                    if !same(&control.left(), &child) {
                        out.push(simple(parent, child, BranchKind::LogicalOperator));
                    }
                    index -= 1;
                }
                NodeKind::LocalVariableOrWriteNode
                | NodeKind::LocalVariableAndWriteNode
                | NodeKind::LocalVariableOperatorWriteNode
                | NodeKind::InstanceVariableOrWriteNode
                | NodeKind::InstanceVariableAndWriteNode
                | NodeKind::InstanceVariableOperatorWriteNode
                | NodeKind::ClassVariableOrWriteNode
                | NodeKind::ClassVariableAndWriteNode
                | NodeKind::ClassVariableOperatorWriteNode
                | NodeKind::GlobalVariableOrWriteNode
                | NodeKind::GlobalVariableAndWriteNode
                | NodeKind::GlobalVariableOperatorWriteNode
                | NodeKind::ConstantOrWriteNode
                | NodeKind::ConstantAndWriteNode
                | NodeKind::ConstantOperatorWriteNode
                | NodeKind::ConstantPathOrWriteNode
                | NodeKind::ConstantPathAndWriteNode
                | NodeKind::ConstantPathOperatorWriteNode
                | NodeKind::IndexOrWriteNode
                | NodeKind::IndexAndWriteNode
                | NodeKind::IndexOperatorWriteNode
                | NodeKind::CallOrWriteNode
                | NodeKind::CallAndWriteNode
                | NodeKind::CallOperatorWriteNode => {
                    // RuboCop ≥ 1.84 (#14796): `lhs op= rhs` is a branch
                    // whose left body always runs and whose right body is
                    // conditional. Parser's child 0 is the whole assignable
                    // (receiver, index arguments, constant path); every
                    // Prism child other than `value` belongs to it.
                    if same(&operator_write_value(&parent), &child) {
                        out.push(simple(parent, child, BranchKind::OperatorAssignment));
                    }
                    index -= 1;
                }
                NodeKind::BeginNode => {
                    push_begin_branches(&parent, &child, out);
                    index -= 1;
                }
                NodeKind::RescueNode if child.kind() == NodeKind::RescueNode => {
                    // Prism nests `rescue` clauses; parser hangs them all off
                    // the one `rescue` node, so flatten the chain back.
                    let mut root = index - 1;
                    while root > 0 && self.ancestors[root].kind() == NodeKind::RescueNode {
                        root -= 1;
                    }
                    push_begin_branches(&self.ancestors[root], &child, out);
                    index = root;
                }
                _ => index -= 1,
            }
        }
    }

    // ------------------------------------------------------------ traversal

    fn process(&mut self, node: Node<'pr>) {
        match node.kind() {
            NodeKind::LocalVariableWriteNode => self.process_variable_assignment(node),
            NodeKind::LocalVariableTargetNode => self.process_variable_target(node),
            NodeKind::LocalVariableOrWriteNode
            | NodeKind::LocalVariableAndWriteNode
            | NodeKind::LocalVariableOperatorWriteNode => self.process_operator_assignment(node),
            NodeKind::MultiWriteNode => self.process_multiple_assignment(node),
            NodeKind::MatchWriteNode => self.process_regexp_named_captures(node),
            NodeKind::LocalVariableReadNode => {
                let name = node.as_local_variable_read_node().expect("kind matched").name();
                self.reference_name(name.as_slice(), node, true);
            }
            NodeKind::ItLocalVariableReadNode => {}
            NodeKind::WhileNode | NodeKind::UntilNode | NodeKind::ForNode => {
                self.process_loop(node);
            }
            NodeKind::BeginNode => self.process_begin(node),
            NodeKind::ForwardingSuperNode => {
                self.process_zero_arity_super(node);
                self.walk_children(node);
            }
            NodeKind::CallNode => {
                self.process_send(node);
                self.walk_children(node);
            }
            NodeKind::DefNode => self.process_scope(node, ScopeKind::Def),
            NodeKind::BlockNode | NodeKind::LambdaNode => {
                self.process_scope(node, ScopeKind::Block);
            }
            NodeKind::ClassNode => self.process_scope(node, ScopeKind::Class),
            NodeKind::ModuleNode => self.process_scope(node, ScopeKind::Module),
            NodeKind::SingletonClassNode => self.process_scope(node, ScopeKind::SingletonClass),
            NodeKind::MatchPredicateNode => {
                let n = node.as_match_predicate_node().expect("kind matched");
                self.with_node(node, |b| {
                    b.process(n.value());
                    b.process_pattern(n.pattern());
                });
            }
            NodeKind::MatchRequiredNode => {
                let n = node.as_match_required_node().expect("kind matched");
                self.with_node(node, |b| {
                    b.process(n.value());
                    b.process_pattern(n.pattern());
                });
            }
            NodeKind::InNode => {
                let n = node.as_in_node().expect("kind matched");
                self.with_node(node, |b| {
                    b.process_pattern(n.pattern());
                    if let Some(statements) = n.statements() {
                        b.process(statements.as_node());
                    }
                });
            }
            _ => self.walk_children(node),
        }
    }

    fn process_pattern(&mut self, node: Node<'pr>) {
        let saved = std::mem::replace(&mut self.in_pattern, true);
        self.process(node);
        self.in_pattern = saved;
    }

    fn process_variable_assignment(&mut self, node: Node<'pr>) {
        let n = node.as_local_variable_write_node().expect("kind matched");
        let name = n.name().as_slice();
        self.declare_if_absent(name, node, DeclKind::Assignment);
        // The right-hand side is scanned first so `foo = foo + 1` marks the
        // previous assignment as referenced.
        self.with_node(node, |b| b.process(n.value()));
        self.assign(name, node, n.name_loc().span(), None, false);
    }

    fn process_variable_target(&mut self, node: Node<'pr>) {
        let n = node.as_local_variable_target_node().expect("kind matched");
        let name = n.name().as_slice();
        if self.in_pattern {
            // `match_var`: declares the name, never assigns to it.
            self.non_assignment_targets.insert(node_id(&node));
            self.declare_if_absent(name, node, DeclKind::PatternMatch);
            return;
        }
        self.declare_if_absent(name, node, DeclKind::Assignment);
        let meta = self.meta_for_target();
        let exception = self.is_exception_reference(&node);
        self.assign(name, node, node.span(), meta, exception);
    }

    fn process_operator_assignment(&mut self, node: Node<'pr>) {
        let (name, name_span, value) = match node.kind() {
            NodeKind::LocalVariableOrWriteNode => {
                let n = node.as_local_variable_or_write_node().expect("kind matched");
                (n.name().as_slice(), n.name_loc().span(), n.value())
            }
            NodeKind::LocalVariableAndWriteNode => {
                let n = node.as_local_variable_and_write_node().expect("kind matched");
                (n.name().as_slice(), n.name_loc().span(), n.value())
            }
            _ => {
                let n = node.as_local_variable_operator_write_node().expect("kind matched");
                (n.name().as_slice(), n.name_loc().span(), n.value())
            }
        };
        self.declare_if_absent(name, node, DeclKind::Assignment);
        // `foo += bar` is `foo = foo + bar`: the read happens first.
        self.reference_name(name, node, true);
        self.with_node(node, |b| b.process(value));
        self.assign(name, node, name_span, Some(Meta::Operator), false);
    }

    fn process_multiple_assignment(&mut self, node: Node<'pr>) {
        let n = node.as_multi_write_node().expect("kind matched");
        self.with_node(node, |b| {
            b.process(n.value());
            for target in &n.lefts() {
                b.process(target);
            }
            if let Some(rest) = n.rest() {
                b.process(rest);
            }
            for target in &n.rights() {
                b.process(target);
            }
        });
    }

    fn process_regexp_named_captures(&mut self, node: Node<'pr>) {
        let n = node.as_match_write_node().expect("kind matched");
        let call = n.call();
        let mut names: Vec<&'pr [u8]> = Vec::new();
        for target in &n.targets() {
            if let Some(target) = target.as_local_variable_target_node() {
                self.non_assignment_targets.insert(node_id(&target.as_node()));
                names.push(target.name().as_slice());
            }
        }
        for name in &names {
            self.declare_if_absent(name, node, DeclKind::RegexpNamedCapture);
        }
        let regexp = call.receiver();
        self.with_node(node, |b| {
            b.with_node(call.as_node(), |b| {
                if let Some(arguments) = call.arguments() {
                    b.process(arguments.as_node());
                }
                if let Some(regexp) = call.receiver() {
                    b.process(regexp);
                }
            });
        });
        let span = regexp.map_or_else(|| node.span(), |r| r.span());
        for name in &names {
            self.assign(name, node, span, None, false);
        }
    }

    fn process_loop(&mut self, node: Node<'pr>) {
        match node.kind() {
            NodeKind::WhileNode => {
                let n = node.as_while_node().expect("kind matched");
                if n.is_begin_modifier() {
                    self.with_node(node, |b| {
                        if let Some(statements) = n.statements() {
                            b.process(statements.as_node());
                        }
                        b.process(n.predicate());
                    });
                } else {
                    self.walk_children(node);
                }
            }
            NodeKind::UntilNode => {
                let n = node.as_until_node().expect("kind matched");
                if n.is_begin_modifier() {
                    self.with_node(node, |b| {
                        if let Some(statements) = n.statements() {
                            b.process(statements.as_node());
                        }
                        b.process(n.predicate());
                    });
                } else {
                    self.walk_children(node);
                }
            }
            _ => {
                // `for item in items`: the rightmost expression runs first.
                let n = node.as_for_node().expect("kind matched");
                self.with_node(node, |b| {
                    b.process(n.collection());
                    b.process(n.index());
                    if let Some(statements) = n.statements() {
                        b.process(statements.as_node());
                    }
                });
            }
        }
        self.mark_assignments_as_referenced_in_loop(node);
    }

    /// RuboCop's `process_rescue`: `begin`/`rescue` with a `retry` behaves
    /// like a loop.
    fn process_begin(&mut self, node: Node<'pr>) {
        let n = node.as_begin_node().expect("kind matched");
        let mut contains_retry = false;
        let mut clause = n.rescue_clause();
        while let Some(rescue) = clause {
            let rescue_node = rescue.as_node();
            each_descendant(&rescue_node, &mut |descendant| {
                if descendant.kind() == NodeKind::RetryNode {
                    contains_retry = true;
                }
            });
            clause = rescue.subsequent();
        }
        self.walk_children(node);
        if contains_retry {
            self.mark_assignments_as_referenced_in_loop(node);
        }
    }

    fn process_zero_arity_super(&mut self, node: Node<'pr>) {
        for vid in self.accessible_variables() {
            if self.sem.is_method_argument(vid) {
                self.reference_variable(vid, node, false);
            }
        }
    }

    /// RuboCop's `process_send`: an argumentless `binding` can read anything
    /// in scope.
    fn process_send(&mut self, node: Node<'pr>) {
        let call = node.as_call_node().expect("kind matched");
        if call.name().as_slice() != b"binding" || call.is_safe_navigation() {
            return;
        }
        if call.arguments().is_some_and(|args| args.arguments().iter().next().is_some()) {
            return;
        }
        for vid in self.accessible_variables() {
            self.reference_variable(vid, node, false);
        }
    }

    fn process_scope(&mut self, node: Node<'pr>, kind: ScopeKind) {
        // Twisted children (a `defs` receiver, a class name and superclass, a
        // `class << expr` expression) belong to the enclosing scope.
        self.with_node(node, |b| {
            for child in twisted_children(&node).into_iter().flatten() {
                b.process(child);
            }
        });
        self.push_scope(node, kind);
        let saved = std::mem::replace(&mut self.in_pattern, false);
        self.with_node(node, |b| {
            for child in scope_children(&node).into_iter().flatten() {
                if is_parameter_container(child.kind()) {
                    b.declare_parameters(child);
                } else {
                    b.process(child);
                }
            }
        });
        self.in_pattern = saved;
        self.pop_scope();
    }

    /// Walks the containers (`args`, block `args`, destructuring `mlhs`)
    /// that only hold other parameter nodes.
    fn declare_parameter_group(&mut self, node: Node<'pr>) {
        match node.kind() {
            NodeKind::BlockParametersNode => {
                let n = node.as_block_parameters_node().expect("kind matched");
                self.with_node(node, |b| {
                    if let Some(parameters) = n.parameters() {
                        b.declare_parameters(parameters.as_node());
                    }
                    for local in &n.locals() {
                        b.declare_parameters(local);
                    }
                });
            }
            NodeKind::ParametersNode => {
                let n = node.as_parameters_node().expect("kind matched");
                self.with_node(node, |b| {
                    for parameter in &n.requireds() {
                        b.declare_parameters(parameter);
                    }
                    for parameter in &n.optionals() {
                        b.declare_parameters(parameter);
                    }
                    if let Some(parameter) = n.rest() {
                        b.declare_parameters(parameter);
                    }
                    for parameter in &n.posts() {
                        b.declare_parameters(parameter);
                    }
                    for parameter in &n.keywords() {
                        b.declare_parameters(parameter);
                    }
                    if let Some(parameter) = n.keyword_rest() {
                        b.declare_parameters(parameter);
                    }
                    if let Some(parameter) = n.block() {
                        b.declare_parameters(parameter.as_node());
                    }
                });
            }
            _ => {
                let n = node.as_multi_target_node().expect("kind matched");
                self.with_node(node, |b| {
                    for target in &n.lefts() {
                        b.declare_parameters(target);
                    }
                    if let Some(rest) = n.rest() {
                        b.declare_parameters(rest);
                    }
                    for target in &n.rights() {
                        b.declare_parameters(target);
                    }
                });
            }
        }
    }

    fn declare_parameters(&mut self, node: Node<'pr>) {
        match node.kind() {
            NodeKind::BlockParametersNode
            | NodeKind::ParametersNode
            | NodeKind::MultiTargetNode => self.declare_parameter_group(node),
            NodeKind::SplatNode => {
                let n = node.as_splat_node().expect("kind matched");
                if let Some(expression) = n.expression() {
                    self.with_node(node, |b| b.declare_parameters(expression));
                }
            }
            NodeKind::RequiredParameterNode => {
                let n = node.as_required_parameter_node().expect("kind matched");
                self.declare(n.name().as_slice(), node, DeclKind::RequiredArg);
            }
            NodeKind::OptionalParameterNode => {
                let n = node.as_optional_parameter_node().expect("kind matched");
                self.declare(n.name().as_slice(), node, DeclKind::OptionalArg);
                self.with_node(node, |b| b.process(n.value()));
            }
            NodeKind::RestParameterNode => {
                let n = node.as_rest_parameter_node().expect("kind matched");
                if let Some(name) = n.name() {
                    self.declare(name.as_slice(), node, DeclKind::RestArg);
                }
            }
            NodeKind::RequiredKeywordParameterNode => {
                let n = node.as_required_keyword_parameter_node().expect("kind matched");
                self.declare(n.name().as_slice(), node, DeclKind::KeywordArg);
            }
            NodeKind::OptionalKeywordParameterNode => {
                let n = node.as_optional_keyword_parameter_node().expect("kind matched");
                self.declare(n.name().as_slice(), node, DeclKind::OptionalKeywordArg);
                self.with_node(node, |b| b.process(n.value()));
            }
            NodeKind::KeywordRestParameterNode => {
                let n = node.as_keyword_rest_parameter_node().expect("kind matched");
                if let Some(name) = n.name() {
                    self.declare(name.as_slice(), node, DeclKind::KeywordRestArg);
                }
            }
            NodeKind::BlockParameterNode => {
                let n = node.as_block_parameter_node().expect("kind matched");
                if let Some(name) = n.name() {
                    self.declare(name.as_slice(), node, DeclKind::BlockArg);
                }
            }
            NodeKind::BlockLocalVariableNode => {
                let n = node.as_block_local_variable_node().expect("kind matched");
                self.declare(n.name().as_slice(), node, DeclKind::BlockLocal);
            }
            NodeKind::NumberedParametersNode
            | NodeKind::ItParametersNode
            | NodeKind::ForwardingParameterNode => {}
            _ => self.process(node),
        }
    }

    // ----------------------------------------------------------------- loops

    /// RuboCop's `mark_assignments_as_referenced_in_loop`: an assignment in a
    /// loop body is read by the next iteration, whatever the AST order says.
    fn mark_assignments_as_referenced_in_loop(&mut self, node: Node<'pr>) {
        let mut names: Vec<&'pr [u8]> = Vec::new();
        let mut assignment_nodes: Vec<NodeId> = Vec::new();
        {
            let skip = &self.non_assignment_targets;
            each_descendant(&node, &mut |descendant| match descendant.kind() {
                NodeKind::LocalVariableReadNode => {
                    let n = descendant.as_local_variable_read_node().expect("kind matched");
                    names.push(n.name().as_slice());
                }
                NodeKind::LocalVariableWriteNode => assignment_nodes.push(node_id(descendant)),
                NodeKind::LocalVariableTargetNode => {
                    let id = node_id(descendant);
                    if !skip.contains(&id) {
                        assignment_nodes.push(id);
                    }
                }
                NodeKind::LocalVariableOrWriteNode => {
                    let n = descendant.as_local_variable_or_write_node().expect("kind matched");
                    names.push(n.name().as_slice());
                    assignment_nodes.push(node_id(descendant));
                }
                NodeKind::LocalVariableAndWriteNode => {
                    let n = descendant.as_local_variable_and_write_node().expect("kind matched");
                    names.push(n.name().as_slice());
                    assignment_nodes.push(node_id(descendant));
                }
                NodeKind::LocalVariableOperatorWriteNode => {
                    let n =
                        descendant.as_local_variable_operator_write_node().expect("kind matched");
                    names.push(n.name().as_slice());
                    assignment_nodes.push(node_id(descendant));
                }
                _ => {}
            });
        }

        for name in names {
            let Some(vid) = self.find_variable(name) else { continue };
            let in_loop: Vec<AssignmentId> = self.sem.variables[vid.index()]
                .assignments
                .iter()
                .copied()
                .filter(|aid| {
                    assignment_nodes.contains(&node_id(&self.sem.assignments[aid.index()].node))
                })
                .collect();
            let Some(&last) = in_loop.last() else { continue };
            // In a branching statement every assignment survives to the next
            // iteration; otherwise only the last one does. `rescue` counts as
            // branching because of `retry`.
            for &aid in &in_loop {
                if self.has_branch_ancestor(aid) {
                    self.mark_referenced(aid, node);
                }
            }
            self.mark_referenced(last, node);
        }
    }

    fn mark_referenced(&mut self, aid: AssignmentId, node: Node<'pr>) {
        let assignment = &mut self.sem.assignments[aid.index()];
        assignment.referenced = true;
        assignment.references.push(node);
    }

    /// RuboCop's `assignment.node.each_ancestor(:if, :case, :case_match,
    /// :rescue).any?`.
    fn has_branch_ancestor(&self, aid: AssignmentId) -> bool {
        self.sem.slice(self.sem.assignments[aid.index()].ancestors).iter().any(|ancestor| {
            match ancestor.kind() {
                NodeKind::IfNode
                | NodeKind::UnlessNode
                | NodeKind::CaseNode
                | NodeKind::CaseMatchNode => true,
                NodeKind::BeginNode => {
                    ancestor.as_begin_node().expect("kind matched").rescue_clause().is_some()
                }
                _ => false,
            }
        })
    }

    // ------------------------------------------------------- meta / helpers

    /// RuboCop's `Assignment#meta_assignment_node` for a bare target: the
    /// `masgn`, bare `splat` or `for` it belongs to.
    fn meta_for_target(&self) -> Option<Meta<'pr>> {
        let innermost = *self.ancestors.last()?;
        let mut index = self.ancestors.len() - 1;
        loop {
            match self.ancestors[index].kind() {
                NodeKind::MultiWriteNode => return Some(Meta::Multiple(self.ancestors[index])),
                NodeKind::ForNode => return Some(Meta::For(self.ancestors[index])),
                NodeKind::SplatNode | NodeKind::MultiTargetNode => {
                    if index == 0 {
                        break;
                    }
                    index -= 1;
                }
                _ => break,
            }
        }
        (innermost.kind() == NodeKind::SplatNode).then_some(Meta::Rest(innermost))
    }

    fn is_exception_reference(&self, node: &Node<'pr>) -> bool {
        let Some(parent) = self.ancestors.last() else { return false };
        parent.as_rescue_node().is_some_and(|rescue| same_opt(rescue.reference().as_ref(), node))
    }

    // ------------------------------------------------- variable-like names

    /// RuboCop's `collect_variable_like_names`, run as its own pass so the
    /// names land in source order (`Scope#each_node`) rather than in the
    /// twisted order the main walk uses.
    fn collect_bare_call_names(&mut self, root: Node<'pr>) {
        let index: HashMap<NodeId, ScopeId> = self
            .sem
            .scopes
            .iter()
            .enumerate()
            .map(|(i, scope)| (node_id(&scope.node), ScopeId(u32::try_from(i).unwrap_or(u32::MAX))))
            .collect();
        let top = self.sem.top_level();
        self.visit_names(&index, top, root);
    }

    fn visit_names(&mut self, index: &HashMap<NodeId, ScopeId>, scope: ScopeId, node: Node<'pr>) {
        if let Some(call) = node.as_call_node() {
            if call.receiver().is_none() && call.arguments().is_none() {
                let name = call.name().as_slice();
                let names = &mut self.sem.scopes[scope.index()].bare_call_names;
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        match index.get(&node_id(&node)) {
            Some(&inner) if crate::is_scope_kind(node.kind()) => {
                for child in twisted_children(&node).into_iter().flatten() {
                    self.visit_names(index, scope, child);
                }
                for child in scope_children(&node).into_iter().flatten() {
                    self.visit_names(index, inner, child);
                }
            }
            _ => for_each_child(&node, |child| self.visit_names(index, scope, *child)),
        }
    }
}

/// RuboCop's `Assignment#operator`: `(meta_assignment_node || node)
/// .loc.operator`. `for` and bare-splat metas have no operator location --
/// RuboCop would raise there, so `None` stands in.
fn operator_span(node: &Node<'_>, meta: Option<&Meta<'_>>) -> Option<Span> {
    match meta {
        Some(Meta::Operator) => match node.kind() {
            NodeKind::LocalVariableOrWriteNode => {
                Some(node.as_local_variable_or_write_node()?.operator_loc().span())
            }
            NodeKind::LocalVariableAndWriteNode => {
                Some(node.as_local_variable_and_write_node()?.operator_loc().span())
            }
            _ => Some(node.as_local_variable_operator_write_node()?.binary_operator_loc().span()),
        },
        Some(Meta::Multiple(write)) => Some(write.as_multi_write_node()?.operator_loc().span()),
        Some(Meta::Rest(_) | Meta::For(_)) => None,
        None => Some(node.as_local_variable_write_node()?.operator_loc().span()),
    }
}

fn simple<'pr>(control: Node<'pr>, child: Node<'pr>, kind: BranchKind) -> RawBranch<'pr> {
    RawBranch { control, child, kind, may_jump: false, may_run_incompletely: false }
}

/// The `value` child of any of Prism's `*OrWriteNode`/`*AndWriteNode`/
/// `*OperatorWriteNode` variants (parser's `or_asgn`/`and_asgn`/`op_asgn`
/// right-hand side).
fn operator_write_value<'pr>(node: &Node<'pr>) -> Node<'pr> {
    macro_rules! value {
        ($($as:ident),*) => {
            $(if let Some(n) = node.$as() { return n.value(); })*
        };
    }
    value!(
        as_local_variable_or_write_node,
        as_local_variable_and_write_node,
        as_local_variable_operator_write_node,
        as_instance_variable_or_write_node,
        as_instance_variable_and_write_node,
        as_instance_variable_operator_write_node,
        as_class_variable_or_write_node,
        as_class_variable_and_write_node,
        as_class_variable_operator_write_node,
        as_global_variable_or_write_node,
        as_global_variable_and_write_node,
        as_global_variable_operator_write_node,
        as_constant_or_write_node,
        as_constant_and_write_node,
        as_constant_operator_write_node,
        as_constant_path_or_write_node,
        as_constant_path_and_write_node,
        as_constant_path_operator_write_node,
        as_index_or_write_node,
        as_index_and_write_node,
        as_index_operator_write_node,
        as_call_or_write_node,
        as_call_and_write_node,
        as_call_operator_write_node
    );
    unreachable!("caller matched an operator-write kind")
}

/// The `Rescue`/`Ensure` branches of a `BeginNode`, mirroring parser's
/// `(ensure (rescue body resbody... else) ensure_body)` nesting.
fn push_begin_branches<'pr>(control: &Node<'pr>, child: &Node<'pr>, out: &mut Vec<RawBranch<'pr>>) {
    let n = control.as_begin_node().expect("kind matched");
    let ensure = n.ensure_clause().map(|clause| clause.as_node());
    if let Some(ensure) = ensure {
        if same(&ensure, child) {
            // The ensure body always runs: not a branch.
            return;
        }
    }
    if n.rescue_clause().is_some() {
        let main = n.statements().map(|statements| statements.as_node());
        let is_main = same_opt(main.as_ref(), child);
        out.push(RawBranch {
            control: *control,
            child: *child,
            kind: BranchKind::Rescue,
            may_jump: is_main,
            may_run_incompletely: is_main,
        });
        if ensure.is_some() {
            // Stands in for parser's inner `rescue` node as the `ensure`
            // branch's main-body child.
            out.push(RawBranch {
                control: *control,
                child: *control,
                kind: BranchKind::Ensure,
                may_jump: true,
                may_run_incompletely: true,
            });
        }
    } else if ensure.is_some() {
        out.push(RawBranch {
            control: *control,
            child: *child,
            kind: BranchKind::Ensure,
            may_jump: true,
            may_run_incompletely: true,
        });
    }
}

/// Children of a scope node that belong to the enclosing scope.
fn twisted_children<'pr>(node: &Node<'pr>) -> [Option<Node<'pr>>; 2] {
    match node.kind() {
        NodeKind::DefNode => [node.as_def_node().expect("kind matched").receiver(), None],
        NodeKind::ClassNode => {
            let n = node.as_class_node().expect("kind matched");
            [Some(n.constant_path()), n.superclass()]
        }
        NodeKind::ModuleNode => {
            [Some(node.as_module_node().expect("kind matched").constant_path()), None]
        }
        NodeKind::SingletonClassNode => {
            [Some(node.as_singleton_class_node().expect("kind matched").expression()), None]
        }
        _ => [None, None],
    }
}

/// Children of a scope node that belong to the scope itself.
fn scope_children<'pr>(node: &Node<'pr>) -> [Option<Node<'pr>>; 2] {
    match node.kind() {
        NodeKind::ProgramNode => {
            [Some(node.as_program_node().expect("kind matched").statements().as_node()), None]
        }
        NodeKind::DefNode => {
            let n = node.as_def_node().expect("kind matched");
            [n.parameters().map(|p| p.as_node()), n.body()]
        }
        NodeKind::BlockNode => {
            let n = node.as_block_node().expect("kind matched");
            [n.parameters(), n.body()]
        }
        NodeKind::LambdaNode => {
            let n = node.as_lambda_node().expect("kind matched");
            [n.parameters(), n.body()]
        }
        NodeKind::ClassNode => [node.as_class_node().expect("kind matched").body(), None],
        NodeKind::ModuleNode => [node.as_module_node().expect("kind matched").body(), None],
        NodeKind::SingletonClassNode => {
            [node.as_singleton_class_node().expect("kind matched").body(), None]
        }
        _ => [None, None],
    }
}

fn is_parameter_container(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::BlockParametersNode
            | NodeKind::ParametersNode
            | NodeKind::NumberedParametersNode
            | NodeKind::ItParametersNode
    )
}

/// rubocop-ast's `basic_conditional? && modifier_form?`.
fn is_modifier_conditional(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::IfNode => node.as_if_node().expect("kind matched").end_keyword_loc().is_none(),
        NodeKind::UnlessNode => {
            node.as_unless_node().expect("kind matched").end_keyword_loc().is_none()
        }
        NodeKind::WhileNode => {
            let n = node.as_while_node().expect("kind matched");
            !n.is_begin_modifier() && n.closing_loc().is_none()
        }
        NodeKind::UntilNode => {
            let n = node.as_until_node().expect("kind matched");
            !n.is_begin_modifier() && n.closing_loc().is_none()
        }
        _ => false,
    }
}
