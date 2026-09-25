//! `Lint/UselessAccessModifier`, ported from RuboCop's
//! `lib/rubocop/cop/lint/useless_access_modifier.rb`.
//!
//! RuboCop implements this cop with a hand-rolled recursive walk
//! (`check_scope`/`check_child_nodes`) layered on top of its own visitor
//! callbacks (`on_class`/`on_module`/`on_sclass`/`on_block`/`on_begin`),
//! which independently re-visit the same nodes and rely on
//! `Base#add_offense` deduplicating identical offense ranges. This port
//! instead rides the single generic tree traversal directly: a stack of
//! [`Scope`] frames (one per class/module/singleton-class body or
//! qualifying block) tracks the current visibility and the last
//! unconfirmed modifier, and a per-frame `suppressed` counter brackets
//! method-definition subtrees (regular `def`s, singleton `def`s, and
//! method-creating calls) so nothing inside them leaks into the
//! enclosing scope -- mirroring RuboCop's `check_child_nodes` never
//! recursing into a `defs` node or a matched `method_definition?` child.
//! Because every node is visited exactly once, no deduplication step is
//! needed.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    NodeInfo, OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CallNode, DefNode};
use ruby_ast::{ext, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// One of the four bare visibility modifiers. `private_class_method` is
/// handled separately (it never sets/compares `cur_vis`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Vis {
    Public,
    Protected,
    Private,
    ModuleFunction,
}

impl Vis {
    fn from_bytes(name: &[u8]) -> Option<Self> {
        Some(match name {
            b"public" => Self::Public,
            b"protected" => Self::Protected,
            b"private" => Self::Private,
            b"module_function" => Self::ModuleFunction,
            _ => return None,
        })
    }

    fn as_bytes(self) -> &'static [u8] {
        match self {
            Self::Public => b"public",
            Self::Protected => b"protected",
            Self::Private => b"private",
            Self::ModuleFunction => b"module_function",
        }
    }
}

/// Whether a frame is the file's implicit top-level scope (access
/// modifiers there are always useless, unconditionally, per RuboCop's
/// `on_begin`) or a real tracked scope (class/module/sclass body, or a
/// qualifying block) with RuboCop's `check_scope` visibility bookkeeping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScopeKind {
    TopLevel,
    Tracked,
}

/// One entry on the scope stack. `cur_vis: None` for a `Tracked` frame
/// models RuboCop's `cur_vis, unused = nil` quirk after a
/// `private_class_method` call with arguments (see
/// [`UselessAccessModifier::apply_access_modifier`]): no subsequent bare
/// modifier in that scope can ever compare equal again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Scope {
    kind: ScopeKind,
    cur_vis: Option<Vis>,
    unused: Option<(Span, Vis)>,
    suppressed: u32,
}

impl Scope {
    fn top_level() -> Self {
        Self { kind: ScopeKind::TopLevel, cur_vis: None, unused: None, suppressed: 0 }
    }

    fn tracked() -> Self {
        Self { kind: ScopeKind::Tracked, cur_vis: Some(Vis::Public), unused: None, suppressed: 0 }
    }
}

/// What a `CallNode`'s `enter` pushed, so `leave` knows what to undo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CallAction {
    None,
    /// Pushed a new [`Scope`] frame (an `included` block with
    /// `ActiveSupportExtensionsEnabled`, or an eval-like block).
    PushedScope,
    /// Incremented the top frame's `suppressed` counter (an access
    /// modifier call or a method-creating call).
    PushedSuppress,
}

/// Checks for redundant access modifiers.
#[derive(Debug, Clone)]
pub struct UselessAccessModifier {
    context_creating_methods: Vec<Box<[u8]>>,
    method_creating_methods: Vec<Box<[u8]>>,
    active_support_extensions_enabled: bool,
    scopes: Vec<Scope>,
    call_actions: Vec<CallAction>,
}

impl UselessAccessModifier {
    /// RuboCop's `add_offense(node, message: ...) { |corrector| autocorrect
    /// (corrector, node) }`: reports at `span` and removes its whole
    /// line(s), including the trailing newline (`range_by_whole_lines`,
    /// `include_final_newline: true`).
    fn report(ctx: &mut Context<'_>, span: Span, name: &[u8]) {
        let message = format!("Useless `{}` access modifier.", String::from_utf8_lossy(name));
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::delete(ctx.whole_lines(span))],
        };
        ctx.report_with_fix(&Self::META, span, message, fix);
    }

    /// RuboCop's `check_scope`: pops the frame and, if a modifier was left
    /// pending at the end of its body, reports it.
    fn close_scope(&mut self, ctx: &mut Context<'_>) {
        let frame = self.scopes.pop().expect("push/pop of scopes is balanced");
        if let Some((span, vis)) = frame.unused {
            Self::report(ctx, span, vis.as_bytes());
        }
    }

    /// RuboCop's `access_modifier?` branch of `check_send_node`/
    /// `check_child_nodes`, generalized over [`ScopeKind`]: at the top
    /// level every access modifier is unconditionally useless
    /// (`on_begin`); in a tracked scope, a bare modifier goes through
    /// `check_new_visibility`, `private_class_method` with no arguments
    /// always fires, and `private_class_method` with arguments hits
    /// RuboCop's `cur_vis, unused = nil` quirk (the implicit method
    /// return of an unmatched `check_send_node` is not an array, so
    /// Ruby's parallel assignment sets both to `nil`).
    fn apply_access_modifier(
        &mut self,
        ctx: &mut Context<'_>,
        call: &CallNode<'_>,
        span: Span,
        bare: bool,
    ) {
        let name = call.name();
        let name = name.as_slice();
        let frame = self.scopes.last_mut().expect("top-level frame always present");
        match frame.kind {
            ScopeKind::TopLevel => {
                // RuboCop's `on_begin` returns unless the `begin` is the
                // root and inspects only its direct children, so a
                // modifier nested in any non-tracked construct at top level
                // (`RSpec.describe do private end`, `if x then private end`)
                // is never reported; nor is a file whose whole body is the
                // one modifier (whitequark emits no `begin` for a single
                // statement, so `on_begin` never fires).
                let direct_child_of_root = matches!(
                    ctx.ancestors(),
                    [NodeInfo { kind: NodeKind::ProgramNode, .. }, NodeInfo { kind: NodeKind::StatementsNode, span: body }]
                        if *body != call.as_node().span()
                );
                if direct_child_of_root && (bare || call.arguments().is_none()) {
                    Self::report(ctx, span, name);
                }
            }
            ScopeKind::Tracked => {
                if bare {
                    let vis =
                        Vis::from_bytes(name).expect("bare_access_modifier name is one of four");
                    let flush = if frame.cur_vis == Some(vis) {
                        Some((span, vis))
                    } else {
                        let flush = frame.unused.take();
                        frame.cur_vis = Some(vis);
                        frame.unused = Some((span, vis));
                        flush
                    };
                    if let Some((flush_span, flush_vis)) = flush {
                        Self::report(ctx, flush_span, flush_vis.as_bytes());
                    }
                } else if call.arguments().is_none() {
                    Self::report(ctx, span, name);
                } else {
                    frame.cur_vis = None;
                    frame.unused = None;
                }
            }
        }
    }

    /// RuboCop's `method_definition?`: a bare (receiver-less) `def`
    /// counts unconditionally; `attr`/`attr_reader`/`attr_writer`/
    /// `attr_accessor`/`define_method` and any configured
    /// `MethodCreatingMethods` name count as a bare (receiver-less) call,
    /// regardless of arguments or an attached block.
    fn is_method_creating_call(&self, call: &CallNode<'_>) -> bool {
        if call.receiver().is_some() {
            return false;
        }
        let name = call.name();
        let name = name.as_slice();
        matches!(
            name,
            b"attr" | b"attr_reader" | b"attr_writer" | b"attr_accessor" | b"define_method"
        ) || self.method_creating_methods.iter().any(|m| m.as_ref() == name)
    }

    /// RuboCop's `eval_call?`: `class_eval`/`instance_eval` with an
    /// attached block, `Class.new`/`Module.new`/`Struct.new`/
    /// `Data.define` (`class_constructor?`, with or without a block), or
    /// a configured `ContextCreatingMethods` name with an attached block
    /// and a `nil`/constant receiver.
    fn is_eval_call(&self, call: &CallNode<'_>, has_block: bool) -> bool {
        let name = call.name();
        let name = name.as_slice();
        if has_block && matches!(name, b"class_eval" | b"instance_eval") {
            return true;
        }
        if is_class_constructor(call) {
            return true;
        }
        if has_block && self.context_creating_methods.iter().any(|m| m.as_ref() == name) {
            return match call.receiver() {
                None => true,
                Some(r) => {
                    matches!(r.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode)
                }
            };
        }
        false
    }

    /// RuboCop's `on_send`/`check_send_node`/`check_child_nodes` fan-out
    /// for one `CallNode`, in the same mutually-exclusive priority order:
    /// access modifier, then `included` block, then method-creating call,
    /// then eval-like block, then (implicitly) transparent fallthrough.
    fn enter_call(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node().expect("kind matched");
        let span = call.location().span();
        let name = call.name();
        let name = name.as_slice();
        let has_block = call.block().is_some_and(|b| b.as_block_node().is_some());

        let is_bare = ext::is_bare_access_modifier(&call) && in_macro_scope(ctx);
        let is_access_mod = is_bare || name == b"private_class_method";
        let is_incl = self.active_support_extensions_enabled && has_block && name == b"included";
        let is_method_def = !is_access_mod && !is_incl && self.is_method_creating_call(&call);
        let is_eval =
            !is_access_mod && !is_incl && !is_method_def && self.is_eval_call(&call, has_block);

        let suppressed = self.scopes.last().expect("top-level frame always present").suppressed;
        if suppressed == 0 {
            if is_access_mod {
                self.apply_access_modifier(ctx, &call, span, is_bare);
            } else if is_method_def {
                let frame = self.scopes.last_mut().expect("checked above");
                if frame.kind == ScopeKind::Tracked {
                    frame.unused = None;
                }
            }
        }

        let action = if is_incl || is_eval {
            self.scopes.push(Scope::tracked());
            CallAction::PushedScope
        } else if is_access_mod || is_method_def {
            self.scopes.last_mut().expect("checked above").suppressed += 1;
            CallAction::PushedSuppress
        } else {
            CallAction::None
        };
        self.call_actions.push(action);
    }

    /// RuboCop's `method_definition?`/`!child.defs_type?`: entering any
    /// `def` (singleton or not) brackets its whole subtree as opaque to
    /// the enclosing scope; only a receiver-less `def` counts as
    /// "a method was defined" (clearing a pending modifier).
    fn enter_def(&mut self, def: &DefNode<'_>) {
        let frame = self.scopes.last_mut().expect("top-level frame always present");
        if frame.suppressed == 0 && def.receiver().is_none() && frame.kind == ScopeKind::Tracked {
            frame.unused = None;
        }
        frame.suppressed += 1;
    }
}

impl Rule for UselessAccessModifier {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UselessAccessModifier",
        department: Department::Lint,
        summary: "Checks for redundant access modifiers.",
        explanation: "\
Checks for redundant access modifiers, including those with no code, those
which are repeated, those which are on top-level, and leading `public`
modifiers in a class or module body. Conditionally-defined methods are
considered as always being defined, and thus access modifiers guarding such
methods are not redundant.

This cop has a `ContextCreatingMethods` option, an array of methods which,
when called, are known to create their own context in the module's current
access context (e.g. ActiveSupport's `concerning`). It also has a
`MethodCreatingMethods` option, an array of methods which, when called, are
known to create other methods in the module's current access context (e.g.
ActiveSupport's `delegate`). Both default to an empty array.

```ruby
# bad
class Foo
  public # this is redundant (default access is public)

  def method
  end
end

# bad
class Foo
  # The following is redundant (methods defined on the class' singleton
  # class are not affected by the private modifier)
  private

  def self.method3
  end
end

# bad
class Foo
  private # this is redundant (no following methods are defined)
end

# bad
private # this is useless (access modifiers have no effect on top-level)

def method
end

# good
class Foo
  private # this is not redundant (a method is defined)

  def method2
  end
end

# good
class Foo
  # The following is not redundant (conditionally defined methods are
  # considered as always defining a method)
  private

  if condition?
    def method
    end
  end
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::ClassNode,
            NodeKind::ModuleNode,
            NodeKind::SingletonClassNode,
            NodeKind::DefNode,
            NodeKind::CallNode,
        ],
        config: &[
            ConfigOption {
                name: "ContextCreatingMethods",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Methods which, when called with a block, are known to create their own \
                      context in the module's current access context.",
            },
            ConfigOption {
                name: "MethodCreatingMethods",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Methods which, when called, are known to create other methods in the \
                      module's current access context.",
            },
        ],
        blind_spots: "\
Ported as a single-pass stack of scope frames rather than RuboCop's own
double-recursion (its manual `check_child_nodes` walk plus the independent
`on_class`/`on_block` visitor callbacks, reconciled only by
`Base#add_offense` deduplicating identical ranges); the observable set of
offenses is the same, but nothing here depends on RuboCop's own
double-reporting quirk. `ContextCreatingMethods`/`MethodCreatingMethods`
entries are matched as plain method names (RuboCop's `def_node_matcher`-
generated matchers accept the same shapes this port checks directly:
`nil?`/const receiver plus an attached block for context-creating methods,
a `nil?` receiver for method-creating ones); a configured `\"included\"`
entry is ignored either way, matching RuboCop's own guard against
redefining its `included_block?`/`method_definition?` matchers.
`AllCops: ActiveSupportExtensionsEnabled` gates `included do ... end`
exactly as RuboCop does, including the resulting difference in which of two
repeated modifiers gets flagged. `private_class_method` called with
arguments reproduces RuboCop's own `cur_vis, unused = nil` quirk (a
non-array implicit return destructured by Ruby's parallel assignment):
after such a call, the enclosing scope can never again report a repeated
bare modifier, since `cur_vis` stays `nil` for the rest of that scope.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let context_creating_methods = options
            .str_list("ContextCreatingMethods")
            .into_iter()
            .filter(|m| m != "included")
            .map(|m| m.into_bytes().into_boxed_slice())
            .collect();
        let method_creating_methods = options
            .str_list("MethodCreatingMethods")
            .into_iter()
            .filter(|m| m != "included")
            .map(|m| m.into_bytes().into_boxed_slice())
            .collect();
        let active_support_extensions_enabled = options
            .peer("AllCops", "ActiveSupportExtensionsEnabled")
            .and_then(OptionValue::as_bool)
            .unwrap_or(false);
        Ok(Self {
            context_creating_methods,
            method_creating_methods,
            active_support_extensions_enabled,
            scopes: vec![Scope::top_level()],
            call_actions: Vec::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.scopes.clear();
        self.scopes.push(Scope::top_level());
        self.call_actions.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::ClassNode { .. } | Node::ModuleNode { .. } | Node::SingletonClassNode { .. } => {
                self.scopes.push(Scope::tracked());
            }
            Node::DefNode { .. } => {
                let def = node.as_def_node().expect("kind matched");
                self.enter_def(&def);
            }
            Node::CallNode { .. } => {
                self.enter_call(node, ctx);
            }
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::ClassNode { .. } | Node::ModuleNode { .. } | Node::SingletonClassNode { .. } => {
                self.close_scope(ctx);
            }
            Node::DefNode { .. } => {
                self.scopes.last_mut().expect("push/pop of scopes is balanced").suppressed -= 1;
            }
            Node::CallNode { .. } => match self.call_actions.pop().unwrap_or(CallAction::None) {
                CallAction::PushedScope => self.close_scope(ctx),
                CallAction::PushedSuppress => {
                    self.scopes.last_mut().expect("push/pop of scopes is balanced").suppressed -= 1;
                }
                CallAction::None => {}
            },
            _ => {}
        }
    }
}

/// rubocop-ast's `(const {nil? cbase} %1)` shape (see
/// [`ext::is_bare_or_toplevel_const`]) plus a name comparison against a
/// small allowed set, for `class_constructor?`'s `Class`/`Module`/
/// `Struct`/`Data` receivers.
fn is_bare_or_toplevel_const_named(node: &Node<'_>, expected: &[&[u8]]) -> bool {
    ext::is_bare_or_toplevel_const(node)
        && match node.kind() {
            NodeKind::ConstantReadNode => node
                .as_constant_read_node()
                .is_some_and(|n| expected.contains(&n.name().as_slice())),
            NodeKind::ConstantPathNode => node
                .as_constant_path_node()
                .and_then(|path| path.name())
                .is_some_and(|id| expected.contains(&id.as_slice())),
            _ => unreachable!("ext::is_bare_or_toplevel_const already checked the shape"),
        }
}

/// rubocop-ast's `class_constructor?`: `Class.new`/`Module.new`/
/// `Struct.new`, or `Data.define`, with or without an attached block.
fn is_class_constructor(call: &CallNode<'_>) -> bool {
    let name = call.name();
    match name.as_slice() {
        b"new" => call.receiver().is_some_and(|r| {
            is_bare_or_toplevel_const_named(&r, &[b"Class", b"Module", b"Struct"])
        }),
        b"define" => {
            call.receiver().is_some_and(|r| is_bare_or_toplevel_const_named(&r, &[b"Data"]))
        }
        _ => false,
    }
}

/// rubocop-ast's `in_macro_scope?`, ported for Prism's shape: unlike
/// whitequark (which never wraps a single-statement body, and nests a
/// block's own `send` as a *child* of its `:block` node), Prism always
/// wraps a body in a `StatementsNode`, and nests a block's owning call as
/// its own *parent*, so a `BlockNode` and its owning `CallNode` are
/// peeled as one combined hop. Walks outward from the current node's
/// immediate parent through `StatementsNode`/`BeginNode`/`IfNode`/
/// `UnlessNode` (RuboCop's `kwbegin`/`begin`/the non-condition side of
/// `if`) and `BlockNode`-plus-owning-`CallNode` (RuboCop's `any_block`,
/// name-agnostic: `.each do ... end`, `class_eval do ... end`, and
/// `Class.new do ... end` all qualify equally), succeeding at the file
/// root or a `class`/`module`/`class << expr` node. Determines whether a
/// receiver-less `private`/`protected`/`public`/`module_function` call
/// is a real access-modifier declaration rather than, say, a hash value
/// or a method argument that merely happens to share its name.
fn in_macro_scope(ctx: &Context<'_>) -> bool {
    let ancestors = ctx.ancestors();
    let mut i = ancestors.len();
    loop {
        if i == 0 {
            return true;
        }
        match ancestors[i - 1].kind {
            NodeKind::StatementsNode
            | NodeKind::BeginNode
            | NodeKind::IfNode
            | NodeKind::UnlessNode => {
                i -= 1;
            }
            NodeKind::BlockNode if i >= 2 && ancestors[i - 2].kind == NodeKind::CallNode => {
                i -= 2;
            }
            NodeKind::ClassNode
            | NodeKind::ModuleNode
            | NodeKind::SingletonClassNode
            | NodeKind::ProgramNode => {
                return true;
            }
            _ => return false,
        }
    }
}
