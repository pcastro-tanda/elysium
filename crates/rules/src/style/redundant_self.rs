//! `Style/RedundantSelf`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_self.rb`.
//!
//! RuboCop does not use `VariableForce` for this cop (see
//! `docs/planning/phase4-semantic.md`'s "Style/RedundantSelf model"). It
//! keeps its own ad hoc `@local_variables_scopes: Hash[node -> names]`,
//! built with `Hash.new { |h, k| h[k] = [] }.compare_by_identity` -- a
//! default-on-read hash where several keys can alias the very same array
//! object. `self.foo` is redundant unless `foo` is in the list attached to
//! the send node itself or to any of its ancestors, is a `Kernel` method,
//! or the checks in `regular_method_call?`/`it_method_in_block?` exempt it.
//!
//! This port reproduces the aliasing with a `(NodeKind, Span)`-keyed map
//! (RuboCop's node identity) into a list-of-lists (`scopes`), plus an
//! explicit stack of the nearest enclosing `def`/block scope's list id
//! (mirroring `add_scope`'s eager sweep of every descendant, done lazily:
//! the first time a not-yet-keyed node is queried while a `def`/block
//! scope is active, it is given that scope's list id and the assignment is
//! memoized, exactly like a hash's default-on-read).
//!
//! Two upstream mechanisms are structurally impossible on Prism and are
//! therefore omitted rather than approximated: `@allowed_send_nodes`
//! (`allow_self`, used because whitequark represents `self.foo += 1` as an
//! `op_asgn` wrapping an ordinary `send` node that the generic traversal
//! *also* visits as a plain send) and `node.parent&.mlhs_type?` (used
//! because whitequark represents a multiple-assignment attribute target,
//! e.g. `self.a, foo.b = x`, as an ordinary `send` node too). Prism instead
//! gives compound-assignment and multi-assignment attribute targets their
//! own dedicated node kinds (`CallOrWriteNode`/`CallAndWriteNode`/
//! `CallOperatorWriteNode`/`IndexOrWriteNode`/... and `CallTargetNode`/
//! `IndexTargetNode`), none of which is a `CallNode`, so they never reach
//! [`RedundantSelf::enter`]'s `CallNode` handling in the first place.

use std::collections::HashMap;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind, Visitor};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Redundant `self` detected.";

/// RuboCop's `KEYWORDS`.
const KEYWORDS: &[&[u8]] = &[
    b"alias",
    b"and",
    b"begin",
    b"break",
    b"case",
    b"class",
    b"def",
    b"defined?",
    b"do",
    b"else",
    b"elsif",
    b"end",
    b"ensure",
    b"false",
    b"for",
    b"if",
    b"in",
    b"module",
    b"next",
    b"nil",
    b"not",
    b"or",
    b"redo",
    b"rescue",
    b"retry",
    b"return",
    b"self",
    b"super",
    b"then",
    b"true",
    b"undef",
    b"unless",
    b"until",
    b"when",
    b"while",
    b"yield",
    b"__FILE__",
    b"__LINE__",
    b"__ENCODING__",
];

/// RuboCop's `KERNEL_METHODS = Kernel.methods(false)`, dumped from Ruby
/// 3.4.2 (`ruby -e 'puts Kernel.methods(false).sort'`). RuboCop evaluates
/// this once, at cop-load time, using whatever Ruby runs RuboCop itself --
/// independent of the linted file's `TargetRubyVersion` -- so a build-time
/// constant is the faithful port, not a per-`TargetRubyVersion` table.
const KERNEL_METHODS: &[&[u8]] = &[
    b"Array",
    b"Complex",
    b"Float",
    b"Hash",
    b"Integer",
    b"Rational",
    b"String",
    b"__callee__",
    b"__dir__",
    b"__method__",
    b"`",
    b"abort",
    b"at_exit",
    b"autoload",
    b"autoload?",
    b"binding",
    b"block_given?",
    b"caller",
    b"caller_locations",
    b"catch",
    b"eval",
    b"exec",
    b"exit",
    b"exit!",
    b"fail",
    b"fork",
    b"format",
    b"gets",
    b"global_variables",
    b"iterator?",
    b"lambda",
    b"load",
    b"local_variables",
    b"loop",
    b"open",
    b"p",
    b"print",
    b"printf",
    b"proc",
    b"putc",
    b"puts",
    b"raise",
    b"rand",
    b"readline",
    b"readlines",
    b"require",
    b"require_relative",
    b"select",
    b"set_trace_func",
    b"sleep",
    b"spawn",
    b"sprintf",
    b"srand",
    b"syscall",
    b"system",
    b"test",
    b"throw",
    b"trace_var",
    b"trap",
    b"untrace_var",
    b"warn",
];

/// Identity key for one node, mirroring RuboCop's `compare_by_identity`
/// hash (two nodes with the same `(kind, span)` never coexist in one
/// file).
type NodeKey = (NodeKind, Span);

fn key(node: &Node<'_>) -> NodeKey {
    (node.kind(), node.span())
}

/// Checks for redundant uses of `self`.
#[derive(Debug, Clone, Default)]
pub struct RedundantSelf {
    /// RuboCop's `@local_variables_scopes`: every node's list id, keyed by
    /// identity. A node absent here has never been queried or pushed to.
    scope_of: HashMap<NodeKey, usize>,
    /// The list objects themselves; several keys in `scope_of` may share
    /// one id, exactly like several hash keys sharing one Ruby array.
    scopes: Vec<Vec<Vec<u8>>>,
    /// The nearest enclosing `def`/block/lambda scope's list id, for the
    /// lazy default-on-read behaviour of [`RedundantSelf::list_id`].
    stack: Vec<usize>,
    /// Parallel to the block/lambda frames on `stack`: whether that block
    /// has no written parameter delimiters at all (RuboCop's
    /// `arguments.empty_and_without_delimiters?`), for
    /// `it_method_in_block?`'s `each_ancestor(:block)` search.
    bare_block_stack: Vec<bool>,
}

impl Rule for RedundantSelf {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantSelf",
        department: Department::Style,
        summary: "Checks for redundant uses of `self`.",
        explanation: "\
The usage of `self` is only needed when:

* Sending a message to same object with zero arguments in
  presence of a method name clash with an argument or a local
  variable.

* Calling an attribute writer to prevent a local variable assignment.

Note, with using explicit self you can only send messages with public or
protected scope, you cannot send private messages this way.

Note we allow uses of `self` with operators because it would be awkward
otherwise. Also allows the use of `self.it` without arguments in blocks,
as in `0.times { self.it }`, following `Lint/ItWithoutArgumentsInBlock` cop.

```ruby
# bad
def foo(bar)
  self.baz
end

# good
def foo(bar)
  self.bar  # Resolves name clash with the argument.
end

def foo
  bar = 1
  self.bar  # Resolves name clash with the local variable.
end

def foo
  %w[x y z].select do |bar|
    self.bar == bar  # Resolves name clash with argument of the block.
  end
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[
            NodeKind::DefNode,
            NodeKind::BlockNode,
            NodeKind::LambdaNode,
            NodeKind::ParametersNode,
            NodeKind::BlockParameterNode,
            NodeKind::BlockLocalVariableNode,
            NodeKind::InNode,
            NodeKind::IfNode,
            NodeKind::UnlessNode,
            NodeKind::WhileNode,
            NodeKind::UntilNode,
            NodeKind::LocalVariableWriteNode,
            NodeKind::LocalVariableOrWriteNode,
            NodeKind::LocalVariableAndWriteNode,
            NodeKind::MultiWriteNode,
            NodeKind::CallNode,
        ],
        config: &[],
        blind_spots: "\
`@allowed_send_nodes` (`allow_self`) and `node.parent&.mlhs_type?` are
omitted: both exist upstream only because whitequark represents
`self.foo += 1`/`self.foo ||= x` and a multi-assignment attribute target
(`self.a, foo.b = x`) as an ordinary `send` node nested inside an
`op_asgn`/`or_asgn`/`masgn` wrapper, which the generic traversal also
visits as a plain send. Prism gives every one of those shapes its own
dedicated node kind instead (`CallOrWriteNode`, `CallAndWriteNode`,
`CallOperatorWriteNode`, `CallTargetNode`, ...), none of which is a
`CallNode`, so they never reach this rule's send handling and need no
special-casing.

`KERNEL_METHODS` is a build-time dump of `Kernel.methods(false)` on Ruby
3.4.2 (see the constant's doc comment), matching RuboCop's own
load-time evaluation on whatever Ruby runs RuboCop.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.scope_of.clear();
        self.scopes.clear();
        self.stack.clear();
        self.bare_block_stack.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::DefNode { .. } => {
                let id = self.new_scope();
                self.scope_of.insert(key(node), id);
                self.stack.push(id);
            }
            Node::BlockNode { .. } => {
                let n = node.as_block_node().expect("kind matched");
                self.enter_block_like(node, n.parameters().is_none());
            }
            Node::LambdaNode { .. } => {
                let n = node.as_lambda_node().expect("kind matched");
                self.enter_block_like(node, n.parameters().is_none());
            }
            Node::ParametersNode { .. } => {
                let n = node.as_parameters_node().expect("kind matched");
                for param in &n.requireds() {
                    self.process_param_node(&param);
                }
                for param in &n.optionals() {
                    self.process_param_node(&param);
                }
                if let Some(rest) = n.rest() {
                    self.process_param_node(&rest);
                }
                for param in &n.posts() {
                    self.process_param_node(&param);
                }
                for param in &n.keywords() {
                    self.process_param_node(&param);
                }
                if let Some(kwrest) = n.keyword_rest() {
                    self.process_param_node(&kwrest);
                }
                // `.block` (`&blk`) is a real child node, visited on its
                // own via the `BlockParameterNode` arm below.
            }
            Node::BlockParameterNode { .. } => {
                let n = node.as_block_parameter_node().expect("kind matched");
                if let Some(name) = n.name() {
                    self.push_name(key(node), name.as_slice());
                }
            }
            Node::BlockLocalVariableNode { .. } => {
                let n = node.as_block_local_variable_node().expect("kind matched");
                self.push_name(key(node), n.name().as_slice());
            }
            Node::InNode { .. } => {
                let n = node.as_in_node().expect("kind matched");
                let pattern = n.pattern();
                let names = collect_local_variable_target_names(&pattern);
                for name in names {
                    self.push_name(key(node), name);
                }
            }
            Node::IfNode { .. } => {
                let n = node.as_if_node().expect("kind matched");
                self.handle_conditional(node, n.predicate());
            }
            Node::UnlessNode { .. } => {
                let n = node.as_unless_node().expect("kind matched");
                self.handle_conditional(node, n.predicate());
            }
            Node::WhileNode { .. } => {
                let n = node.as_while_node().expect("kind matched");
                self.handle_conditional(node, n.predicate());
            }
            Node::UntilNode { .. } => {
                let n = node.as_until_node().expect("kind matched");
                self.handle_conditional(node, n.predicate());
            }
            Node::LocalVariableWriteNode { .. } => {
                let n = node.as_local_variable_write_node().expect("kind matched");
                self.add_lhs_to_local_variables_scopes(Some(n.value()), n.name().as_slice());
            }
            Node::LocalVariableOrWriteNode { .. } => {
                let n = node.as_local_variable_or_write_node().expect("kind matched");
                self.add_lhs_to_local_variables_scopes(Some(n.value()), n.name().as_slice());
            }
            Node::LocalVariableAndWriteNode { .. } => {
                let n = node.as_local_variable_and_write_node().expect("kind matched");
                self.add_lhs_to_local_variables_scopes(Some(n.value()), n.name().as_slice());
            }
            Node::MultiWriteNode { .. } => {
                let n = node.as_multi_write_node().expect("kind matched");
                let value = n.value();
                for target in n.lefts().iter().chain(n.rights().iter()) {
                    if let Some(name) = masgn_target_name(&target) {
                        self.add_lhs_to_local_variables_scopes(Some(value), name);
                    }
                }
                // `n.rest()` (the `*splat` target, if any) never
                // contributes a usable name even upstream: whitequark's
                // `child.to_a.first` on a `(splat (lvasgn :x))` child
                // returns the wrapped `lvasgn` *node*, not the `:x`
                // symbol, so it can never equal a method name.
            }
            Node::CallNode { .. } => self.handle_send(node, ctx),
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        match node {
            Node::DefNode { .. } => {
                self.stack.pop();
            }
            Node::BlockNode { .. } | Node::LambdaNode { .. } => {
                self.stack.pop();
                self.bare_block_stack.pop();
            }
            _ => {}
        }
    }
}

impl RedundantSelf {
    fn new_scope(&mut self) -> usize {
        self.scopes.push(Vec::new());
        self.scopes.len() - 1
    }

    /// Lazy `Hash.new { |h, k| h[k] = [] }`: a key already present keeps
    /// its id; a fresh key adopts the nearest enclosing `def`/block scope
    /// (mirroring `add_scope`'s eager sweep, which would already have set
    /// it), or gets its own brand-new isolated list outside any such
    /// scope.
    fn list_id(&mut self, k: NodeKey) -> usize {
        if let Some(&id) = self.scope_of.get(&k) {
            return id;
        }
        let id = match self.stack.last() {
            Some(&top) => top,
            None => self.new_scope(),
        };
        self.scope_of.insert(k, id);
        id
    }

    fn push_name(&mut self, k: NodeKey, name: &[u8]) {
        let id = self.list_id(k);
        self.scopes[id].push(name.to_vec());
    }

    fn contains(&mut self, k: NodeKey, name: &[u8]) -> bool {
        let id = self.list_id(k);
        self.scopes[id].iter().any(|n| n.as_slice() == name)
    }

    fn enter_block_like(&mut self, node: &Node<'_>, is_bare: bool) {
        let id = self.list_id(key(node));
        self.stack.push(id);
        self.bare_block_stack.push(is_bare);
    }

    /// RuboCop's `add_lhs_to_local_variables_scopes`: pushes `name` onto
    /// `rhs`'s list, or onto each of `rhs`'s arguments' lists when `rhs`
    /// is itself a non-empty-argument call (so a later `self.foo` used as
    /// one of those arguments is exempted).
    fn add_lhs_to_local_variables_scopes(&mut self, rhs: Option<Node<'_>>, name: &[u8]) {
        let Some(rhs) = rhs else { return };
        if let Node::CallNode { .. } = rhs {
            let call = rhs.as_call_node().expect("kind matched");
            if let Some(args) = call.arguments() {
                let list = args.arguments();
                if !list.is_empty() {
                    for arg in &list {
                        self.push_name(key(&arg), name);
                    }
                    return;
                }
            }
        }
        self.push_name(key(&rhs), name);
    }

    /// RuboCop's `on_argument`, recursing through a destructured
    /// (`MultiTargetNode`) parameter the way `on_args` would.
    fn process_param_node(&mut self, node: &Node<'_>) {
        match node {
            Node::RequiredParameterNode { .. } => {
                let n = node.as_required_parameter_node().expect("kind matched");
                self.push_name(key(node), n.name().as_slice());
            }
            Node::OptionalParameterNode { .. } => {
                let n = node.as_optional_parameter_node().expect("kind matched");
                self.push_name(key(node), n.name().as_slice());
            }
            Node::RestParameterNode { .. } => {
                let n = node.as_rest_parameter_node().expect("kind matched");
                if let Some(name) = n.name() {
                    self.push_name(key(node), name.as_slice());
                }
            }
            Node::RequiredKeywordParameterNode { .. } => {
                let n = node.as_required_keyword_parameter_node().expect("kind matched");
                self.push_name(key(node), n.name().as_slice());
            }
            Node::OptionalKeywordParameterNode { .. } => {
                let n = node.as_optional_keyword_parameter_node().expect("kind matched");
                self.push_name(key(node), n.name().as_slice());
            }
            Node::KeywordRestParameterNode { .. } => {
                let n = node.as_keyword_rest_parameter_node().expect("kind matched");
                if let Some(name) = n.name() {
                    self.push_name(key(node), name.as_slice());
                }
            }
            Node::MultiTargetNode { .. } => {
                let n = node.as_multi_target_node().expect("kind matched");
                for child in &n.lefts() {
                    self.process_param_node(&child);
                }
                if let Some(rest) = n.rest() {
                    if let Node::SplatNode { .. } = rest {
                        let splat = rest.as_splat_node().expect("kind matched");
                        if let Some(inner) = splat.expression() {
                            self.process_param_node(&inner);
                        }
                    } else {
                        self.process_param_node(&rest);
                    }
                }
                for child in &n.rights() {
                    self.process_param_node(&child);
                }
            }
            // `**nil` (`NoKeywordsParameterNode`) and `...`
            // (`ForwardingParameterNode`) have no name to push.
            _ => {}
        }
    }

    /// RuboCop's `on_if`/`on_while`/`on_until`: every `lvasgn`/`masgn`
    /// anywhere in the whole conditional (predicate *and* body, crossing
    /// nested scopes freely, exactly like `each_descendant`) exempts its
    /// left-hand name from `self.<name>` in the *predicate* -- not in its
    /// own assignment's value.
    fn handle_conditional(&mut self, node: &Node<'_>, predicate: Node<'_>) {
        for descendant in collect_assignment_descendants(node) {
            match descendant {
                Node::LocalVariableWriteNode { .. } => {
                    let n = descendant.as_local_variable_write_node().expect("kind matched");
                    self.add_lhs_to_local_variables_scopes(Some(predicate), n.name().as_slice());
                }
                Node::MultiWriteNode { .. } => {
                    let n = descendant.as_multi_write_node().expect("kind matched");
                    for target in n.lefts().iter().chain(n.rights().iter()) {
                        if let Some(name) = masgn_target_name(&target) {
                            self.add_lhs_to_local_variables_scopes(Some(predicate), name);
                        }
                    }
                }
                _ => unreachable!("collect_assignment_descendants only yields these two kinds"),
            }
        }
    }

    /// RuboCop's `it_method_in_block?`.
    fn it_method_in_block(&self, call: &CallNode<'_>, name: &[u8]) -> bool {
        if name != b"it" {
            return false;
        }
        let Some(&is_bare) = self.bare_block_stack.last() else { return false };
        is_bare && call.arguments().is_none() && call.block().is_none()
    }

    /// Ruby's grammar forbids the bare, argument-less, block-less `it`
    /// this fix would produce whenever it sits directly inside a block
    /// that has explicit parameter delimiters (even empty `||`) --
    /// `` `it` is not allowed when an ordinary parameter is defined ``.
    /// Real RuboCop's own corrector has no such guard (its spec never
    /// exercises `expect_correction` for this shape); this rule keeps the
    /// offense but withholds the otherwise syntax-breaking fix.
    fn it_fix_would_break_syntax(&self, call: &CallNode<'_>, name: &[u8]) -> bool {
        name == b"it"
            && call.arguments().is_none()
            && call.block().is_none()
            && self.bare_block_stack.last() == Some(&false)
    }

    /// RuboCop's `on_send`.
    fn handle_send(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node().expect("kind matched");
        let Some(receiver) = call.receiver() else { return };
        if !matches!(receiver, Node::SelfNode { .. }) {
            return;
        }
        let name = call.name().as_slice();
        if !regular_method_call(&call, name) {
            return;
        }
        if KERNEL_METHODS.contains(&name) {
            return;
        }
        if self.contains(key(node), name) {
            return;
        }
        if ctx.ancestors().iter().any(|a| self.contains((a.kind, a.span), name)) {
            return;
        }
        if self.it_method_in_block(&call, name) {
            return;
        }

        let receiver_span = receiver.span();
        if self.it_fix_would_break_syntax(&call, name) {
            ctx.report(&Self::META, receiver_span, MSG);
            return;
        }
        let mut edits = vec![Edit::delete(receiver_span)];
        if let Some(dot) = call.call_operator_loc() {
            edits.push(Edit::delete(dot.span()));
        }
        let fix = Fix { applicability: Applicability::Safe, edits };
        ctx.report_with_fix(&Self::META, receiver_span, MSG, fix);
    }
}

/// RuboCop's `regular_method_call?`.
fn regular_method_call(call: &CallNode<'_>, name: &[u8]) -> bool {
    !(is_operator_method(name)
        || KEYWORDS.contains(&name)
        || is_camel_case_method(name)
        || call.equal_loc().is_some() // `setter_method?` (`loc.operator`)
        || is_implicit_call(call, name))
}

/// RuboCop's `Node#implicit_call?`: the receiverless-looking `self.(args)`
/// shorthand for `self.call(args)`, recognized by having no `message_loc`
/// at all rather than by any particular spelling.
fn is_implicit_call(call: &CallNode<'_>, name: &[u8]) -> bool {
    name == b"call" && call.message_loc().is_none()
}

/// `rubocop-ast`'s `Node#camel_case_method?`.
fn is_camel_case_method(name: &[u8]) -> bool {
    name.first().is_some_and(u8::is_ascii_uppercase)
}

/// `rubocop-ast`'s `MethodIdentifierPredicates::OPERATOR_METHODS`.
fn is_operator_method(name: &[u8]) -> bool {
    matches!(
        name,
        b"|" | b"^"
            | b"&"
            | b"<=>"
            | b"=="
            | b"==="
            | b"=~"
            | b">"
            | b">="
            | b"<"
            | b"<="
            | b"<<"
            | b">>"
            | b"+"
            | b"-"
            | b"*"
            | b"/"
            | b"%"
            | b"**"
            | b"~"
            | b"+@"
            | b"-@"
            | b"!@"
            | b"~@"
            | b"[]"
            | b"[]="
            | b"!"
            | b"!="
            | b"!~"
            | b"`"
    )
}

/// A masgn target's name, if it directly names a local variable
/// (`LocalVariableTargetNode`). Every other target kind (attribute/index
/// writers, ivars/cvars/gvars/constants, a nested destructured group) is a
/// no-op upstream too: whitequark's `child.to_a.first` only unwraps one
/// level, so anything but a bare `lvasgn` child yields a `Node`, never a
/// symbol, and can thus never match a method name.
fn masgn_target_name<'pr>(node: &Node<'pr>) -> Option<&'pr [u8]> {
    match node {
        Node::LocalVariableTargetNode { .. } => {
            Some(node.as_local_variable_target_node().expect("kind matched").name().as_slice())
        }
        _ => None,
    }
}

/// RuboCop's `add_match_var_scopes`' `in_pattern_node.each_descendant(:match_var)`:
/// every `LocalVariableTargetNode` anywhere in a pattern -- array/hash/find
/// elements, a `=>` capture's target, a `*rest`/`**rest` splat's target --
/// is where Prism represents what whitequark calls `match_var`. A pinned
/// variable (`^foo`) reads an existing local (`LocalVariableReadNode`) and
/// never matches.
fn collect_local_variable_target_names<'pr>(root: &Node<'pr>) -> Vec<&'pr [u8]> {
    struct Finder<'pr> {
        out: Vec<&'pr [u8]>,
    }
    impl<'pr> Visitor<'pr> for Finder<'pr> {
        fn enter(&mut self, node: &Node<'pr>) {
            if let Node::LocalVariableTargetNode { .. } = node {
                let n = node.as_local_variable_target_node().expect("kind matched");
                self.out.push(n.name().as_slice());
            }
        }
    }
    let mut finder = Finder { out: Vec::new() };
    ruby_ast::walk(root, &mut finder);
    finder.out
}

/// RuboCop's `node.each_descendant(:lvasgn, :masgn)`, done eagerly and
/// locally (matching upstream's own immediate, scope-crossing recursive
/// scan) rather than waiting for the main traversal to reach these nodes.
fn collect_assignment_descendants<'pr>(root: &Node<'pr>) -> Vec<Node<'pr>> {
    struct Finder<'pr> {
        out: Vec<Node<'pr>>,
    }
    impl<'pr> Visitor<'pr> for Finder<'pr> {
        fn enter(&mut self, node: &Node<'pr>) {
            if matches!(node, Node::LocalVariableWriteNode { .. } | Node::MultiWriteNode { .. }) {
                self.out.push(*node);
            }
        }
    }
    let mut finder = Finder { out: Vec::new() };
    ruby_ast::walk(root, &mut finder);
    finder.out
}
