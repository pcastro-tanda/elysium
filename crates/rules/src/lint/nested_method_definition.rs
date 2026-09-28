//! `Lint/NestedMethodDefinition`, ported from RuboCop's
//! `lib/rubocop/cop/lint/nested_method_definition.rb` plus the
//! `AllowedMethods`/`AllowedPattern` mixins it includes.
//!
//! # Ancestor shape
//!
//! Upstream's `on_def`/`on_defs` (aliased to the same handler) looks for
//! *any* enclosing `any_def` ancestor (`node.each_ancestor(:any_def).first`)
//! and, separately, whether *any* enclosing `any_block`/`sclass` ancestor --
//! anywhere up the tree, not merely between the flagged node and its
//! nearest enclosing def -- is a "scoping" construct
//! (`scoping_method_call?`). This port keeps that exactly: a running counter
//! of active `DefNode` ancestors, and a stack of booleans (one per active
//! `BlockNode`/`LambdaNode`/`SingletonClassNode` ancestor, recording whether
//! *that* ancestor is scoping) that is scanned with `.any()` on every
//! `DefNode` visited, matching upstream's own `.any?` scan.
//!
//! whitequark's arrow lambda (`->(x) { }`) parses as a `:block` node
//! wrapping a `(send nil :lambda)` call, so it is itself `any_block_type?`
//! and subject to the same `scoping_method_call?` test as a `do...end`/`{}`
//! block (never scoping by default, since `lambda` matches none of
//! `eval_call?`/`exec_call?`/`class_constructor?`, only a configured
//! `AllowedMethods`/`AllowedPatterns` entry naming it explicitly). Prism
//! instead gives the arrow form its own [`ruby_ast::node::LambdaNode`] kind,
//! disjoint from [`ruby_ast::node::BlockNode`], so both are subscribed and
//! treated uniformly: a `LambdaNode`'s implicit owning method name is always
//! `lambda`, with no receiver.
//!
//! A `BlockNode` itself carries no reference to the `CallNode` that owns it;
//! [`NestedMethodDefinition::pending_block_call`] instead records, on
//! entering a `CallNode` that owns a literal block, whether *that* call is a
//! scoping one, then transfers that verdict onto the scoping stack the
//! moment its `BlockNode` child is entered (`enter` always visits a
//! `CallNode`'s receiver and arguments -- any of which may themselves push
//! and fully pop their own pending entry -- before its `block`, so the top of
//! the pending stack is always exactly the block's own owning call when the
//! block is entered).
//!
//! # `class_constructor?`
//!
//! rubocop-ast's `class_constructor?` matches `Class.new`/`Module.new`/
//! `Struct.new`/`Data.define` (bare or `::`-qualified) regardless of
//! arguments, whether or not a block is attached; ported here as
//! [`is_class_constructor_call`], reusing [`ext::is_bare_or_toplevel_const`]
//! and [`ext::const_name`] for the receiver-shape and name checks the
//! upstream `global_const?` matcher performs together.
//!
//! # `allowed_subject_type?`
//!
//! Upstream destructures `subject, = *node`, which for a `:defs` node
//! (`def recv.name` -- a receiver present) is the receiver, but for a plain
//! `:def` node is actually its *method name symbol* (whitequark's `:def`
//! node has no receiver child at all); this is harmless since the
//! `node.defs_type? && allowed_subject_type?(subject)` check only ever
//! evaluates `subject` when `node.defs_type?` is true. This port therefore
//! only inspects [`ruby_ast::node::DefNode::receiver`] when it is `Some`.
//! `variable?` (`ivar`/`gvar`/`cvar`/`lvar`) becomes Prism's four dedicated
//! read-node kinds; `const_type?` becomes `ConstantReadNode`/
//! `ConstantPathNode`; `call_type?` (`send`/`csend` alike) becomes
//! `CallNode` regardless of [`ruby_ast::node::CallNode::is_safe_navigation`],
//! matching why `def (do_something&.y).z` is exempt.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, OptionError, Rule, RuleMeta, RuleOptions,
    Severity, Stability,
};
use regex::Regex;
use ruby_ast::node::CallNode;
use ruby_ast::{ext, Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Method definitions must not be nested. Use `lambda` instead.";

/// RuboCop-ast's `class_constructor?`: `Class.new`/`Module.new`/
/// `Struct.new`/`Data.define`, receiver bare or `::`-qualified.
fn is_class_constructor_call(call: &CallNode<'_>) -> bool {
    let Some(receiver) = call.receiver() else { return false };
    if !ext::is_bare_or_toplevel_const(&receiver) {
        return false;
    }
    let Some(name) = ext::const_name(&receiver) else { return false };
    match call.name().as_slice() {
        b"new" => matches!(name.as_str(), "Class" | "Module" | "Struct"),
        b"define" => name == "Data",
        _ => false,
    }
}

/// RuboCop's `allowed_method_name?`.
fn allowed_method_name(
    name: &[u8],
    allowed_methods: &[String],
    allowed_patterns: &[Regex],
) -> bool {
    let name = String::from_utf8_lossy(name);
    allowed_methods.iter().any(|m| m == name.as_ref())
        || allowed_patterns.iter().any(|pattern| pattern.is_match(&name))
}

/// RuboCop's `allowed_subject_type?`: `subject.variable? || subject.const_type?
/// || subject.call_type?`. Whitequark's `:defs` receiver child never
/// includes grouping parens (`def (do_something&.y).z` parses its receiver
/// directly as the inner call); Prism instead wraps an explicitly
/// parenthesized `defs` receiver in a `ParenthesesNode` whose `body` is,
/// empirically, the bare inner node directly here (unlike an ordinary
/// parenthesized expression elsewhere, which Prism always wraps in an
/// intervening `StatementsNode`) -- both shapes are unwrapped, down through
/// any nesting, to the innermost non-parenthesized node.
fn allowed_subject_type(receiver: &Node<'_>) -> bool {
    let mut receiver = *receiver;
    while receiver.kind() == NodeKind::ParenthesesNode {
        let Some(body) = receiver.as_parentheses_node().and_then(|p| p.body()) else {
            return false;
        };
        receiver = match body.as_statements_node() {
            Some(stmts) if stmts.body().len() == 1 => {
                let Some(single) = stmts.body().first() else { return false };
                single
            }
            Some(_) => return false,
            None => body,
        };
    }
    matches!(
        receiver.kind(),
        NodeKind::LocalVariableReadNode
            | NodeKind::InstanceVariableReadNode
            | NodeKind::ClassVariableReadNode
            | NodeKind::GlobalVariableReadNode
            | NodeKind::ConstantReadNode
            | NodeKind::ConstantPathNode
            | NodeKind::CallNode
    )
}

/// Checks for nested method definitions.
///
/// # Examples
///
/// ```ruby
/// # bad
///
/// # `bar` definition actually produces methods in the same scope
/// # as the outer `foo` method. Furthermore, the `bar` method
/// # will be redefined every time `foo` is invoked.
/// def foo
///   def bar
///   end
/// end
///
/// # good
///
/// def foo
///   bar = -> { puts 'hello' }
///   bar.call
/// end
///
/// # good
///
/// # `class_eval`, `instance_eval`, `module_eval`, `class_exec`, `instance_exec`, and
/// # `module_exec` blocks are allowed by default.
///
/// def foo
///   self.class.class_eval do
///     def bar
///     end
///   end
/// end
///
/// def foo
///   self.class.module_exec do
///     def bar
///     end
///   end
/// end
///
/// # good
///
/// def foo
///   class << self
///     def bar
///     end
///   end
/// end
/// ```
///
/// ## `AllowedMethods`: [] (default)
///
/// ```ruby
/// # bad
/// def do_something
///   has_many :articles do
///     def find_or_create_by_name(name)
///     end
///   end
/// end
/// ```
///
/// `AllowedMethods` (containing `has_many`)
///
/// ```ruby
/// # bad
/// def do_something
///   has_many :articles do
///     def find_or_create_by_name(name)
///     end
///   end
/// end
/// ```
///
/// ## `AllowedPatterns`: [] (default)
///
/// ```ruby
/// # bad
/// def foo(obj)
///   obj.do_baz do
///     def bar
///     end
///   end
/// end
/// ```
///
/// `AllowedPatterns` (containing `baz`)
///
/// ```ruby
/// # good
/// def foo(obj)
///   obj.do_baz do
///     def bar
///     end
///   end
/// end
/// ```
#[derive(Debug, Clone)]
pub struct NestedMethodDefinition {
    /// `AllowedMethods`.
    allowed_methods: Vec<String>,
    /// `AllowedPatterns`, precompiled.
    allowed_patterns: Vec<Regex>,
    /// The number of currently active `DefNode` ancestors.
    def_depth: usize,
    /// One entry per currently active `BlockNode`/`LambdaNode`/
    /// `SingletonClassNode` ancestor: whether *that* ancestor is a scoping
    /// construct. See the module doc.
    scoping_stack: Vec<bool>,
    /// One entry per currently active `CallNode` that owns a literal block:
    /// whether that call is a scoping one, transferred onto
    /// [`Self::scoping_stack`] the moment its `BlockNode` child is entered.
    pending_block_call: Vec<bool>,
}

impl NestedMethodDefinition {
    /// RuboCop's `scoping_method_call?`, for a `CallNode` known to own a
    /// literal block (the `eval_call?`/`exec_call?`/`class_constructor?`
    /// alternatives all require this).
    fn call_is_scoping(&self, call: &CallNode<'_>) -> bool {
        let name = call.name();
        if matches!(
            name.as_slice(),
            b"instance_eval"
                | b"class_eval"
                | b"module_eval"
                | b"instance_exec"
                | b"class_exec"
                | b"module_exec"
        ) {
            return true;
        }
        is_class_constructor_call(call)
            || allowed_method_name(name.as_slice(), &self.allowed_methods, &self.allowed_patterns)
    }

    /// RuboCop's `scoping_method_call?`, for a `LambdaNode` ancestor: its
    /// implicit owning method name is always `lambda`, with no receiver, so
    /// only a configured `AllowedMethods`/`AllowedPatterns` entry naming it
    /// can make it scoping.
    fn lambda_is_scoping(&self) -> bool {
        allowed_method_name(b"lambda", &self.allowed_methods, &self.allowed_patterns)
    }

    fn check_def(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let def = node.as_def_node().expect("kind matched");
        if let Some(receiver) = def.receiver() {
            if allowed_subject_type(&receiver) {
                return;
            }
        }
        if self.def_depth == 0 {
            return;
        }
        if self.scoping_stack.iter().any(|&scoping| scoping) {
            return;
        }
        ctx.report(&Self::META, node.span(), MSG);
    }
}

impl Rule for NestedMethodDefinition {
    const META: RuleMeta = RuleMeta {
        name: "Lint/NestedMethodDefinition",
        department: Department::Lint,
        summary: "Checks for nested method definitions.",
        explanation: "\
`bar` definition actually produces methods in the same scope as the outer
`foo` method. Furthermore, the `bar` method will be redefined every time
`foo` is invoked.

```ruby
# bad
def foo
  def bar
  end
end

# good
def foo
  bar = -> { puts 'hello' }
  bar.call
end

# good

# `class_eval`, `instance_eval`, `module_eval`, `class_exec`, `instance_exec`, and
# `module_exec` blocks are allowed by default.

def foo
  self.class.class_eval do
    def bar
    end
  end
end

def foo
  self.class.module_exec do
    def bar
    end
  end
end

# good

def foo
  class << self
    def bar
    end
  end
end
```

With `AllowedMethods: []` (the default):

```ruby
# bad
def do_something
  has_many :articles do
    def find_or_create_by_name(name)
    end
  end
end
```

With `AllowedMethods: ['has_many']`:

```ruby
# bad
def do_something
  has_many :articles do
    def find_or_create_by_name(name)
    end
  end
end
```

With `AllowedPatterns: []` (the default):

```ruby
# bad
def foo(obj)
  obj.do_baz do
    def bar
    end
  end
end
```

With `AllowedPatterns: ['baz']`:

```ruby
# good
def foo(obj)
  obj.do_baz do
    def bar
    end
  end
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: linter::FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[
            NodeKind::DefNode,
            NodeKind::CallNode,
            NodeKind::BlockNode,
            NodeKind::LambdaNode,
            NodeKind::SingletonClassNode,
        ],
        config: &[
            ConfigOption {
                name: "AllowedMethods",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Method names owning a block whose nested method definitions are never \
                      flagged.",
            },
            ConfigOption {
                name: "AllowedPatterns",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Method name regex patterns owning a block whose nested method \
                      definitions are never flagged.",
            },
        ],
        blind_spots: "\
Upstream's `each_ancestor(:any_block, :sclass).any?` scan considers every enclosing block/sclass
ancestor anywhere up the tree, not merely those between the flagged def and its nearest enclosing
def; this port reproduces that exactly, so a scoping block *outside* (above) the nearest enclosing
def also suppresses the offense, matching upstream. A `def` written as an argument expression
(`foo(def bar; end)`) is not specially handled, matching upstream's own lack of a special case.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let allowed_methods = options.str_list("AllowedMethods");
        let allowed_patterns =
            options.str_list("AllowedPatterns").iter().filter_map(|p| Regex::new(p).ok()).collect();
        Ok(Self {
            allowed_methods,
            allowed_patterns,
            def_depth: 0,
            scoping_stack: Vec::new(),
            pending_block_call: Vec::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.def_depth = 0;
        self.scoping_stack.clear();
        self.pending_block_call.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::DefNode => {
                self.check_def(node, ctx);
                self.def_depth += 1;
            }
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                if matches!(call.block(), Some(Node::BlockNode { .. })) {
                    self.pending_block_call.push(self.call_is_scoping(&call));
                }
            }
            NodeKind::BlockNode => {
                let scoping = self.pending_block_call.last().copied().unwrap_or(false);
                self.scoping_stack.push(scoping);
            }
            NodeKind::LambdaNode => {
                self.scoping_stack.push(self.lambda_is_scoping());
            }
            NodeKind::SingletonClassNode => {
                self.scoping_stack.push(true);
            }
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::DefNode => self.def_depth -= 1,
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                if matches!(call.block(), Some(Node::BlockNode { .. })) {
                    self.pending_block_call.pop();
                }
            }
            NodeKind::BlockNode | NodeKind::LambdaNode | NodeKind::SingletonClassNode => {
                self.scoping_stack.pop();
            }
            _ => {}
        }
    }
}
