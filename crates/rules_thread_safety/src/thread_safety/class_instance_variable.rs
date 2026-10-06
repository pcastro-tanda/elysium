//! `ThreadSafety/ClassInstanceVariable`, ported from rubocop-thread_safety's
//! `lib/rubocop/cop/thread_safety/class_instance_variable.rb`.
//!
//! The upstream cop walks `node.ancestors` of a whitequark tree. Prism's tree
//! differs (a call owns its block; single-statement bodies keep a
//! `StatementsNode`), so the rule keeps its own stack of Prism ancestors and
//! converts it, per candidate node, into the whitequark ancestor chain:
//! a call with a block literal becomes a `block` wrapper (and a `send` when
//! the node sits in the receiver or arguments), single-statement
//! `StatementsNode`s disappear.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::{call_span_excluding_block, is_bare_or_toplevel_const};
use ruby_ast::node::CallNode;
use ruby_ast::{walk, LocationExt as _, Node, NodeExt as _, NodeKind, Visitor};
use ruby_source::Span;

const MSG: &str = "Avoid class instance variables.";

/// Avoid class instance variables.
#[derive(Debug, Clone)]
pub struct ClassInstanceVariable;

/// One whitequark ancestor.
#[derive(Clone, Copy)]
enum Anc<'pr> {
    /// A `send`/`csend` node.
    Send(CallNode<'pr>),
    /// A `block`/`numblock`/`itblock` node wrapping a call.
    Block { call: CallNode<'pr>, numblock: bool },
    /// A `->` literal block.
    Lambda { numblock: bool },
    /// Any other node.
    Other(Node<'pr>),
}

/// An ancestor plus its index in the Prism stack.
#[derive(Clone, Copy)]
struct Entry<'pr> {
    anc: Anc<'pr>,
    index: usize,
}

/// The first child of a whitequark node, when that is a `SendNode`.
enum FirstChild<'pr> {
    None,
    /// A lambda literal (never matches any of the method names used here).
    Lambda,
    Send(CallNode<'pr>),
}

fn send_without_block<'pr>(node: &Node<'pr>) -> FirstChild<'pr> {
    match node.as_call_node() {
        Some(call) if call.block().is_none_or(|b| b.as_block_node().is_none()) => {
            FirstChild::Send(call)
        }
        _ => FirstChild::None,
    }
}

fn first_stmt(body: Option<Node<'_>>) -> FirstChild<'_> {
    let Some(body) = body else { return FirstChild::None };
    match body.as_statements_node() {
        Some(stmts) => {
            stmts.body().iter().next().map_or(FirstChild::None, |first| send_without_block(&first))
        }
        None => send_without_block(&body),
    }
}

fn first_child<'pr>(anc: &Anc<'pr>) -> FirstChild<'pr> {
    match anc {
        Anc::Send(call) => {
            call.receiver().map_or(FirstChild::None, |recv| send_without_block(&recv))
        }
        Anc::Block { call, .. } => FirstChild::Send(*call),
        Anc::Lambda { .. } => FirstChild::Lambda,
        Anc::Other(node) => {
            if let Some(n) = node.as_if_node() {
                send_without_block(&n.predicate())
            } else if let Some(n) = node.as_unless_node() {
                send_without_block(&n.predicate())
            } else if let Some(n) = node.as_while_node() {
                send_without_block(&n.predicate())
            } else if let Some(n) = node.as_until_node() {
                send_without_block(&n.predicate())
            } else if let Some(n) = node.as_case_node() {
                n.predicate().map_or(FirstChild::None, |p| send_without_block(&p))
            } else if let Some(n) = node.as_and_node() {
                send_without_block(&n.left())
            } else if let Some(n) = node.as_or_node() {
                send_without_block(&n.left())
            } else if let Some(n) = node.as_statements_node() {
                n.body().iter().next().map_or(FirstChild::None, |f| send_without_block(&f))
            } else if let Some(n) = node.as_parentheses_node() {
                first_stmt(n.body())
            } else if let Some(n) = node.as_begin_node() {
                first_stmt(n.statements().map(|s| s.as_node()))
            } else if let Some(n) = node.as_array_node() {
                n.elements().iter().next().map_or(FirstChild::None, |f| send_without_block(&f))
            } else if let Some(n) = node.as_return_node() {
                n.arguments()
                    .and_then(|a| a.arguments().iter().next())
                    .map_or(FirstChild::None, |f| send_without_block(&f))
            } else {
                FirstChild::None
            }
        }
    }
}

/// `ancestor.children.first.command?(name)` after the `is_a? SendNode` guard.
fn first_child_is_command(anc: &Anc<'_>, name: &[u8]) -> bool {
    matches!(first_child(anc), FirstChild::Send(call)
        if call.receiver().is_none() && call.name().as_slice() == name)
}

fn is_numblock_params(params: Option<Node<'_>>) -> bool {
    params.is_some_and(|p| {
        matches!(p.kind(), NodeKind::NumberedParametersNode | NodeKind::ItParametersNode)
    })
}

fn const_named(node: &Node<'_>, name: &[u8]) -> bool {
    node.as_constant_read_node().is_some_and(|c| c.name().as_slice() == name)
}

/// `(block (send (const nil? :recv) :meth ...) _ (any_def ...))`.
fn is_new_scope_block(call: &CallNode<'_>, block: &Node<'_>) -> bool {
    let Some(receiver) = call.receiver() else { return false };
    let name = call.name();
    let matches_send = (const_named(&receiver, b"Struct") && name.as_slice() == b"new")
        || (const_named(&receiver, b"Class") && name.as_slice() == b"new")
        || (const_named(&receiver, b"Data") && name.as_slice() == b"define");
    if !matches_send || call.is_safe_navigation() {
        return false;
    }
    let Some(block) = block.as_block_node() else { return false };
    let Some(body) = block.body() else { return false };
    let Some(stmts) = body.as_statements_node() else { return false };
    let mut it = stmts.body().iter();
    matches!((it.next(), it.next()), (Some(only), None) if only.as_def_node().is_some())
}

fn is_callback_block(call: &CallNode<'_>) -> bool {
    call.receiver().is_none()
        && call.arguments().is_none()
        && call.block().is_some_and(|b| b.as_block_node().is_some())
        && matches!(
            call.name().as_slice(),
            b"prepend_around_action"
                | b"prepend_before_action"
                | b"before_action"
                | b"append_before_action"
                | b"around_action"
                | b"append_around_action"
                | b"append_after_action"
                | b"after_action"
                | b"prepend_after_action"
        )
}

fn is_def(anc: &Anc<'_>) -> bool {
    matches!(anc, Anc::Other(n) if n.as_def_node().is_some_and(|d| d.receiver().is_none()))
}

fn is_defs(anc: &Anc<'_>) -> bool {
    matches!(anc, Anc::Other(n) if n.as_def_node().is_some_and(|d| d.receiver().is_some()))
}

/// A plain `block` node (not numblock).
fn is_block(anc: &Anc<'_>) -> bool {
    matches!(anc, Anc::Block { numblock: false, .. } | Anc::Lambda { numblock: false })
}

struct Collector<'pr> {
    stack: Vec<Node<'pr>>,
    offenses: Vec<Span>,
}

impl<'pr> Collector<'pr> {
    /// Whitequark `node.ancestors` (parent first). `own_block` is the call's
    /// block when the node itself is a call that owns one.
    fn ancestors(&self, own_block: Option<CallNode<'pr>>) -> Vec<Entry<'pr>> {
        let mut out = Vec::new();
        let top = self.stack.len();
        if let Some(call) = own_block {
            let numblock = call
                .block()
                .and_then(|b| b.as_block_node())
                .is_some_and(|b| is_numblock_params(b.parameters()));
            out.push(Entry { anc: Anc::Block { call, numblock }, index: top });
        }
        let mut i = top;
        while i > 0 {
            i -= 1;
            let node = self.stack[i];
            let parent_kind = i.checked_sub(1).map(|p| self.stack[p].kind());
            if let Some(call) = node.as_call_node() {
                let in_block =
                    self.stack.get(i + 1).is_some_and(|n| n.kind() == NodeKind::BlockNode);
                if in_block {
                    let numblock = call
                        .block()
                        .and_then(|b| b.as_block_node())
                        .is_some_and(|b| is_numblock_params(b.parameters()));
                    out.push(Entry { anc: Anc::Block { call, numblock }, index: i });
                } else {
                    out.push(Entry { anc: Anc::Send(call), index: i });
                    if let Some(block) = call.block().and_then(|b| b.as_block_node()) {
                        let numblock = is_numblock_params(block.parameters());
                        out.push(Entry { anc: Anc::Block { call, numblock }, index: i });
                    }
                }
            } else if node.kind() == NodeKind::BlockNode {
                // Represented by the owning call's block wrapper.
            } else if let Some(lambda) = node.as_lambda_node() {
                out.push(Entry {
                    anc: Anc::Lambda { numblock: is_numblock_params(lambda.parameters()) },
                    index: i,
                });
            } else if let Some(stmts) = node.as_statements_node() {
                let elided = stmts.body().iter().nth(1).is_none()
                    || matches!(parent_kind, Some(NodeKind::ParenthesesNode | NodeKind::BeginNode));
                if !elided {
                    out.push(Entry { anc: Anc::Other(node), index: i });
                }
            } else {
                out.push(Entry { anc: Anc::Other(node), index: i });
            }
        }
        out
    }

    fn is_scope(anc: &Anc<'pr>) -> bool {
        let Anc::Block { call, numblock: false } = anc else { return false };
        let Some(block) = call.block() else { return false };
        is_new_scope_block(call, &block) || is_callback_block(call)
    }

    fn in_defs(anc: &[Entry<'pr>]) -> bool {
        for e in anc {
            if Self::is_scope(&e.anc) {
                return false;
            }
            if is_defs(&e.anc) {
                return true;
            }
        }
        false
    }

    /// `node.ancestors.find { break if new_lexical_scope?; def_type? }`.
    fn enclosing_def(anc: &[Entry<'pr>]) -> Option<usize> {
        for (idx, e) in anc.iter().enumerate() {
            if Self::is_scope(&e.anc) {
                return None;
            }
            if is_def(&e.anc) {
                return Some(idx);
            }
        }
        None
    }

    fn in_def_sclass(anc: &[Entry<'pr>]) -> bool {
        let Some(def) = Self::enclosing_def(anc) else { return false };
        anc[def + 1..]
            .iter()
            .any(|e| matches!(&e.anc, Anc::Other(n) if n.kind() == NodeKind::SingletonClassNode))
    }

    fn in_def_class_methods_dsl(anc: &[Entry<'pr>]) -> bool {
        for e in anc {
            if Self::is_scope(&e.anc) {
                return false;
            }
            if !is_block(&e.anc) {
                continue;
            }
            if first_child_is_command(&e.anc, b"class_methods") {
                return true;
            }
        }
        false
    }

    fn in_def_class_methods_module(anc: &[Entry<'pr>]) -> bool {
        let Some(def) = Self::enclosing_def(anc) else { return false };
        for e in &anc[def + 1..] {
            if let Anc::Other(n) = &e.anc {
                if n.kind() == NodeKind::ClassNode {
                    return false;
                }
                if let Some(module) = n.as_module_node() {
                    let path = module.constant_path();
                    return const_named(&path, b"ClassMethods")
                        || path.as_constant_path_node().is_some_and(|p| {
                            p.name().is_some_and(|name| name.as_slice() == b"ClassMethods")
                        });
                }
            }
        }
        false
    }

    fn in_def_module_function(&self, anc: &[Entry<'pr>]) -> bool {
        let Some(def_entry) = anc.iter().find(|e| is_def(&e.anc)) else { return false };
        let Anc::Other(def_node) = def_entry.anc else { return false };
        let Some(def) = def_node.as_def_node() else { return false };
        let Some(parent) = def_entry.index.checked_sub(1).map(|p| self.stack[p]) else {
            return false;
        };
        let Some(stmts) = parent.as_statements_node() else { return false };
        let def_start = def_node.span().start;
        let def_name = def.name();
        let mut left = true;
        for sibling in &stmts.body() {
            if sibling.span().start == def_start && sibling.kind() == NodeKind::DefNode {
                left = false;
                continue;
            }
            let Some(call) = sibling.as_call_node() else { continue };
            if call.block().is_some_and(|b| b.as_block_node().is_some())
                || call.is_safe_navigation()
                || call.receiver().is_some()
                || call.name().as_slice() != b"module_function"
            {
                continue;
            }
            match call.arguments() {
                None if left => return true,
                Some(args) if !left => {
                    let mut it = args.arguments().iter();
                    if let (Some(arg), None) = (it.next(), it.next()) {
                        let value = arg
                            .as_symbol_node()
                            .map(|s| s.unescaped().to_vec())
                            .or_else(|| arg.as_string_node().map(|s| s.unescaped().to_vec()));
                        if value.is_some_and(|v| v == def_name.as_slice()) {
                            return true;
                        }
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn in_class_eval(anc: &[Entry<'pr>]) -> bool {
        for e in anc {
            if is_def(&e.anc) || Self::is_scope(&e.anc) {
                return false;
            }
            let Anc::Block { call, numblock: false } = &e.anc else { continue };
            return !call.is_safe_navigation()
                && call.arguments().is_none()
                && call.receiver().is_some_and(|r| is_bare_or_toplevel_const(&r))
                && matches!(call.name().as_slice(), b"class_eval" | b"class_exec");
        }
        false
    }

    fn singleton_method_definition(anc: &[Entry<'pr>]) -> bool {
        for e in anc {
            if Self::is_scope(&e.anc) {
                return false;
            }
            if first_child_is_command(&e.anc, b"define_singleton_method") {
                return true;
            }
        }
        false
    }

    fn method_definition(anc: &[Entry<'pr>]) -> bool {
        for e in anc {
            if Self::is_scope(&e.anc) {
                return false;
            }
            if first_child_is_command(&e.anc, b"define_method") {
                return true;
            }
        }
        false
    }

    fn synchronized(anc: &[Entry<'pr>]) -> bool {
        anc.iter().any(|e| {
            is_block(&e.anc)
                && matches!(first_child(&e.anc), FirstChild::Send(s)
                    if !s.is_safe_navigation()
                        && s.arguments().is_none()
                        && s.name().as_slice() == b"synchronize"
                        && s.block().is_none_or(|b| b.as_block_node().is_some()))
        })
    }

    fn class_method_definition(&self, anc: &[Entry<'pr>]) -> bool {
        Self::in_defs(anc)
            || Self::in_def_sclass(anc)
            || Self::in_def_class_methods_dsl(anc)
            || Self::in_def_class_methods_module(anc)
            || self.in_def_module_function(anc)
            || Self::in_class_eval(anc)
            || Self::singleton_method_definition(anc)
    }

    fn offends(&self, own_block: Option<CallNode<'pr>>) -> bool {
        let anc = self.ancestors(own_block);
        self.class_method_definition(&anc)
            && !Self::method_definition(&anc)
            && !Self::synchronized(&anc)
    }
}

/// `instance_variable_set_call?` / `instance_variable_get_call?`: a
/// receiver-less call with exactly two / one node arguments.
fn is_instance_variable_call(call: &CallNode<'_>) -> bool {
    if call.is_safe_navigation() || call.receiver().is_some() {
        return false;
    }
    let want = match call.name().as_slice() {
        b"instance_variable_set" => 2,
        b"instance_variable_get" => 1,
        _ => return false,
    };
    let given = call.arguments().map_or(0, |a| a.arguments().iter().count())
        + usize::from(call.block().is_some_and(|b| b.as_block_argument_node().is_some()));
    given == want
}

impl<'pr> Visitor<'pr> for Collector<'pr> {
    fn enter(&mut self, node: &Node<'pr>) {
        let ivar_span = match node.kind() {
            NodeKind::InstanceVariableReadNode | NodeKind::InstanceVariableTargetNode => {
                Some(node.span())
            }
            NodeKind::InstanceVariableWriteNode => {
                node.as_instance_variable_write_node().map(|n| n.name_loc().span())
            }
            NodeKind::InstanceVariableOrWriteNode => {
                node.as_instance_variable_or_write_node().map(|n| n.name_loc().span())
            }
            NodeKind::InstanceVariableAndWriteNode => {
                node.as_instance_variable_and_write_node().map(|n| n.name_loc().span())
            }
            NodeKind::InstanceVariableOperatorWriteNode => {
                node.as_instance_variable_operator_write_node().map(|n| n.name_loc().span())
            }
            _ => None,
        };
        if let Some(span) = ivar_span {
            if self.offends(None) {
                self.offenses.push(span);
            }
        } else if let Some(call) = node.as_call_node() {
            if is_instance_variable_call(&call) {
                let own_block = call.block().and_then(|b| b.as_block_node()).map(|_| call);
                if self.offends(own_block) {
                    self.offenses.push(call_span_excluding_block(&call));
                }
            }
        }
        self.stack.push(*node);
    }

    fn leave(&mut self, _node: &Node<'pr>) {
        self.stack.pop();
    }
}

impl Rule for ClassInstanceVariable {
    const META: RuleMeta = RuleMeta {
        name: "ThreadSafety/ClassInstanceVariable",
        department: Department::ThreadSafety,
        summary: "Avoid class instance variables.",
        explanation: "Avoid class instance variables: instance variables set or read in class \
                      methods are shared between threads.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let mut collector = Collector { stack: Vec::new(), offenses: Vec::new() };
        walk(&ctx.parsed().root(), &mut collector);
        for span in collector.offenses {
            ctx.report(&Self::META, span, MSG);
        }
    }
}
