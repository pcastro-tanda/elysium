//! `Lint/MissingSuper`, ported from RuboCop's `lib/rubocop/cop/lint/missing_super.rb`.
//!
//! Upstream's `inside_class_with_stateful_parent?` walks two independent
//! ancestor searches (`each_ancestor(:any_block).first`, then, only if that
//! found nothing, `each_ancestor(:class).first`) that can each reach past
//! the other kind of node at any depth. Since [`Context::ancestors`] carries
//! only kind and span (not enough to recover a `ClassNode`'s superclass or
//! a block's owning `Class.new` call), this port instead tracks its own
//! parallel stacks of already-computed "would this scope make `initialize`
//! need `super`?" flags -- one pushed on every [`NodeKind::ClassNode`], one
//! pushed on every call with an attached block (only ever meaningful when
//! that call is `Class.new(ARG)`) and every [`NodeKind::LambdaNode`] (a
//! stabby `-> { }` is, like a `do...end`/`{ }` block, whitequark's `:block`
//! node type, so it too counts as an "`any_block`" ancestor that blocks the
//! class search, even though it can never itself be a `Class.new` call) --
//! and reads the nearest of whichever stack is non-empty, exactly
//! replicating the "any block, anywhere, wins over any class" precedence.
//! `callback_method_def?`'s own ancestor search (any `class`/`sclass`/
//! `module` at any depth, unrelated to the block/class precedence above)
//! needs no such bookkeeping: [`Context::ancestors`] already carries every
//! node's kind regardless of which rule is subscribed to it.

use ruby_ast::node::CallNode;
use ruby_ast::{ext, Node, NodeExt as _, NodeKind};

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};

/// RuboCop's `CONSTRUCTOR_MSG`.
const CONSTRUCTOR_MSG: &str = "Call `super` to initialize state of the parent class.";
/// RuboCop's `CALLBACK_MSG`.
const CALLBACK_MSG: &str = "Call `super` to invoke callback defined in the parent class.";

/// RuboCop's `STATELESS_CLASSES`.
const STATELESS_CLASSES: &[&str] = &["BasicObject", "Object"];

/// RuboCop's `CALLBACKS` (`CLASS_LIFECYCLE_CALLBACKS + METHOD_LIFECYCLE_CALLBACKS`).
const CALLBACKS: &[&[u8]] = &[
    b"inherited",
    b"method_added",
    b"method_removed",
    b"method_undefined",
    b"singleton_method_added",
    b"singleton_method_removed",
    b"singleton_method_undefined",
];

/// `Lint::MissingSuper`.
#[derive(Debug, Clone)]
pub struct MissingSuper {
    /// RuboCop's `allowed_classes` (`STATELESS_CLASSES + AllowedParentClasses`).
    allowed_classes: Vec<String>,
    /// Stack of `ClassNode` superclass "stateful" flags, outermost first.
    class_frames: Vec<bool>,
    /// Stack of block/lambda frames, outermost first: `None` for a
    /// non-`Class.new` block or any lambda, `Some(stateful)` for a
    /// `Class.new(ARG) do ... end`.
    block_frames: Vec<Option<bool>>,
}

impl MissingSuper {
    fn is_allowed_class(&self, name: &str) -> bool {
        self.allowed_classes.iter().any(|c| c == name)
    }

    /// RuboCop's `allowed_class?` applied to a (possibly absent, possibly
    /// unresolvable) superclass/`Class.new` argument expression.
    fn is_stateful(&self, parent: Option<&Node<'_>>) -> bool {
        match parent {
            None => false,
            Some(n) => match const_name(n) {
                Some(name) => !self.is_allowed_class(&name),
                None => true,
            },
        }
    }

    /// RuboCop's `inside_class_with_stateful_parent?`.
    fn stateful_context(&self) -> bool {
        if let Some(&block_state) = self.block_frames.last() {
            return block_state.unwrap_or(false);
        }
        self.class_frames.last().copied().unwrap_or(false)
    }

    fn on_def(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if contains_super(node) {
            return;
        }
        let def = node.as_def_node().expect("kind matched");
        let name = def.name();
        if def.receiver().is_none() && name.as_slice() == b"initialize" {
            if self.stateful_context() {
                ctx.report(&Self::META, node.span(), CONSTRUCTOR_MSG);
            }
        } else if CALLBACKS.contains(&name.as_slice())
            && ctx.ancestors().iter().any(|a| {
                matches!(
                    a.kind,
                    NodeKind::ClassNode | NodeKind::ModuleNode | NodeKind::SingletonClassNode
                )
            })
        {
            ctx.report(&Self::META, node.span(), CALLBACK_MSG);
        }
    }
}

impl Rule for MissingSuper {
    const META: RuleMeta = RuleMeta {
        name: "Lint/MissingSuper",
        department: Department::Lint,
        summary: "Checks for the presence of constructors and lifecycle callbacks without calls to `super`.",
        explanation: "\
This cop does not consider `method_missing` (and `respond_to_missing?`)
because in some cases it makes sense to overtake what is considered a
missing method. In other cases, the theoretical ideal handling could be
challenging or verbose for no actual gain.

Autocorrection is not supported because the position of `super` cannot be
determined automatically.

`Object` and `BasicObject` are allowed by this cop because of their
stateless nature. However, sometimes you might want to allow other parent
classes from this cop, for example in the case of an abstract class that is
not meant to be called with `super`. In those cases, you can use the
`AllowedParentClasses` option to specify which classes should be allowed
*in addition to* `Object` and `BasicObject`.

```ruby
# bad
class Employee < Person
  def initialize(name, salary)
    @salary = salary
  end
end

# good
class Employee < Person
  def initialize(name, salary)
    super(name)
    @salary = salary
  end
end

# bad
Employee = Class.new(Person) do
  def initialize(name, salary)
    @salary = salary
  end
end

# good
Employee = Class.new(Person) do
  def initialize(name, salary)
    super(name)
    @salary = salary
  end
end

# bad
class Parent
  def self.inherited(base)
    do_something
  end
end

# good
class Parent
  def self.inherited(base)
    super
    do_something
  end
end

# good
class ClassWithNoParent
  def initialize
    do_something
  end
end
```

With `AllowedParentClasses: [MyAbstractClass]`:

```ruby
# good
class MyConcreteClass < MyAbstractClass
  def initialize
    do_something
  end
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode, NodeKind::ClassNode, NodeKind::CallNode, NodeKind::LambdaNode],
        config: &[ConfigOption {
            name: "AllowedParentClasses",
            default: ConfigDefault::StrList(&[]),
            allowed: &[],
            doc: "Allow parent classes that are stateless and not meant to be called with `super`, in addition to `Object` and `BasicObject`.",
        }],
        blind_spots: "\
`contains_super?` is `node.each_descendant(:super, :zsuper).any?`, an
unconditional subtree walk: a `super`/`super(...)` found inside a *nested*
method definition (or a further nested `Class.new do ... end`) still counts
as satisfying the outer method's requirement, matching upstream exactly
rather than scoping the search to the enclosing method only.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let mut allowed_classes: Vec<String> =
            STATELESS_CLASSES.iter().map(|s| (*s).to_string()).collect();
        allowed_classes.extend(options.str_list("AllowedParentClasses"));
        Ok(Self { allowed_classes, class_frames: Vec::new(), block_frames: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::DefNode { .. } => self.on_def(node, ctx),
            Node::ClassNode { .. } => {
                let class = node.as_class_node().expect("kind matched");
                let stateful = self.is_stateful(class.superclass().as_ref());
                self.class_frames.push(stateful);
            }
            Node::LambdaNode { .. } => self.block_frames.push(None),
            Node::CallNode { .. } => {
                let call = node.as_call_node().expect("kind matched");
                if matches!(call.block(), Some(Node::BlockNode { .. })) {
                    let arg = class_new_argument(&call);
                    self.block_frames.push(arg.map(|a| self.is_stateful(Some(&a))));
                }
            }
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        match node {
            Node::ClassNode { .. } => {
                self.class_frames.pop();
            }
            Node::LambdaNode { .. } => {
                self.block_frames.pop();
            }
            Node::CallNode { .. } => {
                let call = node.as_call_node().expect("kind matched");
                if matches!(call.block(), Some(Node::BlockNode { .. })) {
                    self.block_frames.pop();
                }
            }
            _ => {}
        }
    }
}

/// RuboCop's `Node#const_name`: the whole qualified name of a constant
/// (path) node, e.g. `Foo::Bar::Baz`; a leading `::` (a `ConstantPathNode`
/// with no `parent`) contributes nothing extra, matching upstream's
/// `cbase_type?` special case. `None` for any other expression shape (a
/// dynamic superclass/`Class.new` argument), matching upstream's
/// `const_name` returning `nil` for a non-`const` node.
fn const_name(node: &Node<'_>) -> Option<String> {
    match node.kind() {
        NodeKind::ConstantReadNode => {
            let c = node.as_constant_read_node()?;
            Some(String::from_utf8_lossy(c.name().as_slice()).into_owned())
        }
        NodeKind::ConstantPathNode => {
            let path = node.as_constant_path_node()?;
            let short = String::from_utf8_lossy(path.name()?.as_slice()).into_owned();
            match path.parent() {
                Some(parent) => Some(format!("{}::{short}", const_name(&parent)?)),
                None => Some(short),
            }
        }
        _ => None,
    }
}

/// A bare or top-level `Name` constant reference with the given written
/// name. Shape check delegated to [`ext::is_bare_or_toplevel_const`]; the
/// name comparison stays local since the shared helper only checks shape.
fn is_bare_or_toplevel_const(node: &Node<'_>, expected: &[u8]) -> bool {
    if !ext::is_bare_or_toplevel_const(node) {
        return false;
    }
    match node.kind() {
        NodeKind::ConstantReadNode => {
            node.as_constant_read_node().is_some_and(|n| n.name().as_slice() == expected)
        }
        NodeKind::ConstantPathNode => node
            .as_constant_path_node()
            .and_then(|path| path.name())
            .is_some_and(|id| id.as_slice() == expected),
        _ => unreachable!("ext::is_bare_or_toplevel_const already checked the shape"),
    }
}

/// RuboCop's `class_new_block` matcher's captured argument: the sole
/// argument of a `Class.new(ARG)` call (`(send (const {nil? cbase} :Class)
/// :new $_) ...`, arity exactly one -- no arguments, or more than one,
/// never matches).
fn class_new_argument<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    let receiver = call.receiver()?;
    if !is_bare_or_toplevel_const(&receiver, b"Class") || call.name().as_slice() != b"new" {
        return None;
    }
    let args = call.arguments()?;
    let list = args.arguments();
    if list.len() != 1 {
        return None;
    }
    list.iter().next()
}

/// RuboCop's `contains_super?`: `node.each_descendant(:super, :zsuper).any?`.
fn contains_super(node: &Node<'_>) -> bool {
    struct Finder {
        found: bool,
    }
    impl<'pr> ruby_ast::Visitor<'pr> for Finder {
        fn enter(&mut self, node: &Node<'pr>) {
            if matches!(node, Node::SuperNode { .. } | Node::ForwardingSuperNode { .. }) {
                self.found = true;
            }
        }
    }
    let mut finder = Finder { found: false };
    ruby_ast::walk(node, &mut finder);
    finder.found
}
