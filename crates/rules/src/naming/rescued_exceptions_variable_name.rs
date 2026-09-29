//! `Naming/RescuedExceptionsVariableName`, ported from RuboCop's
//! `lib/rubocop/cop/naming/rescued_exceptions_variable_name.rb`.
//!
//! # `RescueNode::subsequent` vs whitequark's flat `resbody` siblings
//!
//! Whitequark wraps every `rescue` clause of one `begin`/`rescue` as a
//! sibling `resbody` under the same `:rescue` node, so `on_resbody`'s
//! `node.each_ancestor(:resbody).any?` nesting guard never sees another
//! clause of the *same* chain as an ancestor -- only a clause that
//! genuinely wraps this one via an inner `begin`/`rescue` in its own body.
//! Prism instead nests every later clause inside the previous one's
//! `subsequent` field, so a generic per-`RescueNode` engine dispatch would
//! make `Context::ancestors` see clause *N-1* as an ancestor of clause *N*
//! even for plain multi-branch `rescue`s -- wrongly treating every branch
//! after the first as "nested". This rule instead subscribes only to
//! `NodeKind::ProgramNode` and walks the tree itself
//! ([`RescuedExceptionsVariableName::walk`]), tracking a plain `nested: bool`
//! that only flips to `true` when descending into a clause's own
//! `statements` (its executed body) -- exactly whitequark's ancestor
//! semantics -- while looping over `subsequent` at the *same* nesting
//! level, matching every sibling branch uniformly.
//!
//! # `right_siblings`-based autocorrect continuation
//!
//! Upstream's `autocorrect` renames the exception variable, then walks the
//! resbody's own `body` for further reads/reassignment; if no reassignment
//! is found there, it looks up `node.parent.each_ancestor(:kwbegin).first`
//! and continues renaming through `kwbegin_node.right_siblings` --
//! statements textually following the whole `begin...end` in whatever list
//! contains it -- until a reassignment is hit. `node.parent.each_ancestor
//! (:kwbegin)` only ever succeeds when the enclosing `begin`/`rescue`
//! construct has a literal `begin` keyword (whitequark's `:kwbegin`,
//! Prism's `BeginNode::begin_keyword_loc`); a `def`'s implicit rescue body
//! is a keyword-less `BeginNode` per the `whitequark -> Prism` trap doc, so
//! its own nearest kwbegin ancestor is never itself and, since these
//! fixtures never nest such a `def` inside a further real `begin...end`,
//! the search always comes up empty -- exactly reproduced here by only
//! reporting `right_siblings` at all when [`RescuedExceptionsVariableName::walk`]
//! reaches a `BeginNode` as a direct element of a `StatementsNode`'s own
//! `body` list (the only place upstream's `right_siblings` is ever
//! non-empty in practice): the remaining slice of that same list, threaded
//! through [`RescuedExceptionsVariableName::handle_begin`] and
//! [`RescuedExceptionsVariableName::process_clause`] alongside the clause's
//! own body.
//!
//! # Body scan: `lvar`/`lvasgn`/`masgn` only
//!
//! `correct_node`'s `node.each_node(:lvar, :lvasgn, :masgn)` only ever
//! matches whitequark's plain-local-variable read/write/multi-assign
//! types -- never `ivar`/`ivasgn` or other variable kinds, even though the
//! *exception* variable itself may be an instance/global/class variable
//! (`variable_name`'s `respond_to?(:name)` accepts any `AsgnNode`). A body
//! rename is therefore only ever applied to local-variable reads/writes:
//! [`scan_and_correct`] mirrors this literally by only special-casing
//! [`NodeKind::LocalVariableReadNode`], [`NodeKind::LocalVariableWriteNode`]
//! and [`NodeKind::MultiWriteNode`], falling back to plain structural
//! recursion (which also reaches other variable kinds' sub-expressions,
//! just never renames or stops at them, exactly like upstream's `each_node`
//! silently skipping non-matching types while still descending into them).
//!
//! # Omitted hash value (`do_something(error:)`)
//!
//! Whitequark's `pair` node holds the omitted value as a plain `lvar`
//! whose own `parent` is the `pair` node directly, letting `correct_node`
//! detect `value_omission?` one level up. Prism instead wraps an omitted
//! value in its own [`NodeKind::ImplicitNode`], so [`scan_and_correct`]
//! carries the enclosing [`NodeKind::AssocNode`]'s `operator_loc` down
//! through exactly one level (the `assoc_operator` parameter) to recover
//! the same "insert after the `:`" correction once it reaches the
//! `ImplicitNode`'s own wrapped read.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{BeginNode, MultiWriteNode, RescueNode};
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Use consistent rescued exceptions variables naming.
#[derive(Debug, Clone)]
pub struct RescuedExceptionsVariableName {
    preferred_name: String,
}

impl Rule for RescuedExceptionsVariableName {
    const META: RuleMeta = RuleMeta {
        name: "Naming/RescuedExceptionsVariableName",
        department: Department::Naming,
        summary: "Use consistent rescued exceptions variables naming.",
        explanation: "\
The `PreferredName` config option takes a `String`. It represents the \
required name of the variable. Its default is `e`.

This cop does not consider nested rescues because it cannot guarantee that \
the variable from the outer rescue is not used within the inner rescue (in \
which case, changing the inner variable would shadow the outer variable).",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ProgramNode],
        config: &[ConfigOption {
            name: "PreferredName",
            default: ConfigDefault::Str("e"),
            allowed: &[],
            doc: "The required name of the variable.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { preferred_name: options.str("PreferredName").into_owned() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        self.walk(node, false, &[], ctx);
    }
}

impl RescuedExceptionsVariableName {
    /// The whole-tree walk. See the module doc comment for why this rule
    /// drives its own traversal instead of subscribing to
    /// `NodeKind::RescueNode`/`NodeKind::BeginNode` directly. `nested`
    /// tracks whether we are currently inside some enclosing `rescue`
    /// clause's own executed body; `right_siblings` is only non-empty when
    /// `node` is a direct element of a `StatementsNode`'s own list (see the
    /// module doc comment on autocorrect continuation).
    fn walk(
        &self,
        node: &Node<'_>,
        nested: bool,
        right_siblings: &[Node<'_>],
        ctx: &mut Context<'_>,
    ) {
        match node.kind() {
            NodeKind::StatementsNode => {
                let stmts = node.as_statements_node().expect("kind matched");
                let items: Vec<Node<'_>> = stmts.body().iter().collect();
                for i in 0..items.len() {
                    self.walk(&items[i], nested, &items[i + 1..], ctx);
                }
            }
            NodeKind::BeginNode => {
                let begin = node.as_begin_node().expect("kind matched");
                self.handle_begin(&begin, nested, right_siblings, ctx);
                if let Some(stmts) = begin.statements() {
                    self.walk(&stmts.as_node(), nested, &[], ctx);
                }
                if let Some(else_clause) = begin.else_clause() {
                    if let Some(stmts) = else_clause.statements() {
                        self.walk(&stmts.as_node(), nested, &[], ctx);
                    }
                }
                if let Some(ensure_clause) = begin.ensure_clause() {
                    if let Some(stmts) = ensure_clause.statements() {
                        self.walk(&stmts.as_node(), nested, &[], ctx);
                    }
                }
            }
            _ => {
                for_each_child(node, |child| self.walk(child, nested, &[], ctx));
            }
        }
    }

    /// Walks one `begin`/`rescue`'s clause chain (`rescue_clause`, then
    /// every `subsequent`), reporting each in turn and recursing into each
    /// clause's own `statements` with `nested` forced to `true`.
    fn handle_begin(
        &self,
        begin: &BeginNode<'_>,
        nested: bool,
        right_siblings: &[Node<'_>],
        ctx: &mut Context<'_>,
    ) {
        let Some(mut clause) = begin.rescue_clause() else { return };
        loop {
            self.process_clause(&clause, nested, right_siblings, ctx);
            if let Some(stmts) = clause.statements() {
                self.walk(&stmts.as_node(), true, &[], ctx);
            }
            for exception in &clause.exceptions() {
                self.walk(&exception, nested, &[], ctx);
            }
            if let Some(reference) = clause.reference() {
                self.walk(&reference, nested, &[], ctx);
            }
            match clause.subsequent() {
                Some(next) => clause = next,
                None => break,
            }
        }
    }

    /// RuboCop's `on_resbody`, for one clause of the chain.
    fn process_clause(
        &self,
        clause: &RescueNode<'_>,
        nested: bool,
        right_siblings: &[Node<'_>],
        ctx: &mut Context<'_>,
    ) {
        let Some(reference) = clause.reference() else { return };
        let Some((offending_name, name_span)) = target_name(&reference) else { return };
        if nested {
            return;
        }
        let preferred = preferred_name_for(&offending_name, &self.preferred_name);
        if preferred.as_bytes() == offending_name.as_slice() {
            return;
        }
        if shadowed_variable_name(clause, &self.preferred_name) {
            return;
        }
        let offending_display = String::from_utf8_lossy(&offending_name);
        let message = format!("Use `{preferred}` instead of `{offending_display}`.");
        let mut edits = vec![Edit::replace(name_span, preferred.clone().into_bytes())];
        let mut stopped = false;
        if let Some(stmts) = clause.statements() {
            stopped = scan_and_correct(&stmts.as_node(), &offending_name, &preferred, &mut edits);
        }
        if !stopped {
            for sibling in right_siblings {
                if scan_and_correct(sibling, &offending_name, &preferred, &mut edits) {
                    break;
                }
            }
        }
        ctx.report_with_fix(
            &Self::META,
            name_span,
            message,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}

/// RuboCop's `variable_name`: only assignment-target kinds `AsgnNode`
/// covers respond to a bare `.name` (`lvasgn`/`ivasgn`/`gvasgn`/`cvasgn`)
/// plus `casgn`'s own `CasgnNode#name`. A method-call target
/// (`rescue => storage.exception`, `CallTargetNode`) or a destructuring
/// target (`IndexTargetNode`/`MultiTargetNode`) has no such method
/// upstream, so both return `None` here, matching "does not register an
/// offense" for a writer-method rescue variable.
fn target_name(node: &Node<'_>) -> Option<(Vec<u8>, Span)> {
    let span = node.span();
    let name: &[u8] = match node.kind() {
        NodeKind::LocalVariableTargetNode => {
            node.as_local_variable_target_node()?.name().as_slice()
        }
        NodeKind::InstanceVariableTargetNode => {
            node.as_instance_variable_target_node()?.name().as_slice()
        }
        NodeKind::GlobalVariableTargetNode => {
            node.as_global_variable_target_node()?.name().as_slice()
        }
        NodeKind::ClassVariableTargetNode => {
            node.as_class_variable_target_node()?.name().as_slice()
        }
        NodeKind::ConstantTargetNode => node.as_constant_target_node()?.name().as_slice(),
        NodeKind::ConstantPathTargetNode => node.as_constant_path_target_node()?.name()?.as_slice(),
        _ => return None,
    };
    Some((name.to_vec(), span))
}

/// RuboCop's `preferred_name`.
fn preferred_name_for(offending: &[u8], configured: &str) -> String {
    if offending.first() == Some(&b'_') {
        format!("_{configured}")
    } else {
        configured.to_string()
    }
}

/// RuboCop's `shadowed_variable_name?`. Upstream's `preferred_name(n)` is
/// called with the whole `lvar` *node* rather than a variable-name string,
/// so its own `variable_name.to_s.start_with?('_')` check (`Node#to_s`
/// returns the node's pretty-printed s-expression, which never starts with
/// `_`) always resolves to the bare configured name -- reproduced here by
/// comparing against `preferred_plain` directly, with no underscore
/// adjustment.
fn shadowed_variable_name(clause: &RescueNode<'_>, preferred_plain: &str) -> bool {
    let target = preferred_plain.as_bytes();
    for exception in &clause.exceptions() {
        if contains_local_read(&exception, target) {
            return true;
        }
    }
    if let Some(reference) = clause.reference() {
        if contains_local_read(&reference, target) {
            return true;
        }
    }
    if let Some(stmts) = clause.statements() {
        if contains_local_read(&stmts.as_node(), target) {
            return true;
        }
    }
    false
}

/// `node.each_descendant(:lvar).any? { |n| n.children.first.to_s == name }`,
/// restricted to `node`'s own subtree (never crossing into a `subsequent`
/// clause, since callers only ever pass one clause's own exceptions/
/// reference/statements).
fn contains_local_read(node: &Node<'_>, name: &[u8]) -> bool {
    if node.as_local_variable_read_node().is_some_and(|read| read.name().as_slice() == name) {
        return true;
    }
    let mut found = false;
    each_descendant_read(node, name, &mut found);
    found
}

fn each_descendant_read(node: &Node<'_>, name: &[u8], found: &mut bool) {
    if *found {
        return;
    }
    for_each_child(node, |child| {
        if *found {
            return;
        }
        if child.as_local_variable_read_node().is_some_and(|read| read.name().as_slice() == name) {
            *found = true;
            return;
        }
        each_descendant_read(child, name, found);
    });
}

/// RuboCop's `correct_node`/`correct_reassignment`/`variable_name_matches?`
/// combined: walks `node`'s subtree pre-order, renaming every matching
/// local-variable read, and stops (returning `true`) at the first matching
/// local-variable or multi-assignment reassignment -- after still
/// correcting that reassignment's own right-hand side. See the module doc
/// comment for the omitted-hash-value (`assoc_operator`) handling and why
/// only these three node kinds are special-cased.
fn scan_and_correct(node: &Node<'_>, name: &[u8], preferred: &str, edits: &mut Vec<Edit>) -> bool {
    scan_in(node, name, preferred, edits, None)
}

fn scan_in(
    node: &Node<'_>,
    name: &[u8],
    preferred: &str,
    edits: &mut Vec<Edit>,
    assoc_operator: Option<Span>,
) -> bool {
    match node.kind() {
        NodeKind::LocalVariableReadNode => {
            let read = node.as_local_variable_read_node().expect("kind matched");
            if read.name().as_slice() == name {
                edits.push(Edit::replace(node.span(), preferred.as_bytes().to_vec()));
            }
            false
        }
        NodeKind::ImplicitNode => {
            let implicit = node.as_implicit_node().expect("kind matched");
            let inner = implicit.value();
            if let Some(read) = inner.as_local_variable_read_node() {
                if read.name().as_slice() == name {
                    if let Some(operator) = assoc_operator {
                        edits
                            .push(Edit::insert(operator.end, format!(" {preferred}").into_bytes()));
                    } else {
                        edits.push(Edit::replace(inner.span(), preferred.as_bytes().to_vec()));
                    }
                    return false;
                }
            }
            scan_in(&inner, name, preferred, edits, None)
        }
        NodeKind::AssocNode => {
            let assoc = node.as_assoc_node().expect("kind matched");
            if scan_in(&assoc.key(), name, preferred, edits, None) {
                return true;
            }
            let operator = assoc.operator_loc().map(|loc| loc.span()).or_else(|| {
                assoc
                    .key()
                    .as_symbol_node()
                    .and_then(|symbol| symbol.closing_loc())
                    .map(|loc| loc.span())
            });
            scan_in(&assoc.value(), name, preferred, edits, operator)
        }
        NodeKind::LocalVariableWriteNode => {
            let write = node.as_local_variable_write_node().expect("kind matched");
            let matches = write.name().as_slice() == name;
            let inner_stopped = scan_in(&write.value(), name, preferred, edits, None);
            if matches {
                true
            } else {
                inner_stopped
            }
        }
        NodeKind::MultiWriteNode => {
            let multi = node.as_multi_write_node().expect("kind matched");
            if multi_target_matches(&multi, name) {
                scan_in(&multi.value(), name, preferred, edits, None);
                true
            } else {
                for left in &multi.lefts() {
                    if scan_in(&left, name, preferred, edits, None) {
                        return true;
                    }
                }
                if let Some(rest) = multi.rest() {
                    if scan_in(&rest, name, preferred, edits, None) {
                        return true;
                    }
                }
                for right in &multi.rights() {
                    if scan_in(&right, name, preferred, edits, None) {
                        return true;
                    }
                }
                scan_in(&multi.value(), name, preferred, edits, None)
            }
        }
        _ => {
            let mut stop = false;
            for_each_child(node, |child| {
                if !stop && scan_in(child, name, preferred, edits, None) {
                    stop = true;
                }
            });
            stop
        }
    }
}

/// RuboCop's `variable_name_matches?` for a `masgn` node: any of its
/// `lvasgn` descendants (here, `LocalVariableTargetNode`s reachable
/// through nested `MultiTargetNode`s/`SplatNode`s) matches `name`.
fn multi_target_matches(multi: &MultiWriteNode<'_>, name: &[u8]) -> bool {
    multi.lefts().iter().any(|n| target_flat_matches(&n, name))
        || multi.rest().is_some_and(|n| target_flat_matches(&n, name))
        || multi.rights().iter().any(|n| target_flat_matches(&n, name))
}

fn target_flat_matches(node: &Node<'_>, name: &[u8]) -> bool {
    match node.kind() {
        NodeKind::LocalVariableTargetNode => {
            node.as_local_variable_target_node().is_some_and(|t| t.name().as_slice() == name)
        }
        NodeKind::MultiTargetNode => {
            let multi = node.as_multi_target_node().expect("kind matched");
            multi.lefts().iter().any(|n| target_flat_matches(&n, name))
                || multi.rest().is_some_and(|n| target_flat_matches(&n, name))
                || multi.rights().iter().any(|n| target_flat_matches(&n, name))
        }
        NodeKind::SplatNode => {
            let splat = node.as_splat_node().expect("kind matched");
            splat.expression().is_some_and(|expr| target_flat_matches(&expr, name))
        }
        _ => false,
    }
}
