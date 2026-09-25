//! `Lint/DuplicateMethods`, ported from RuboCop's `lib/rubocop/cop/lint/duplicate_methods.rb`.

use std::collections::{HashMap, HashSet};

use linter::{
    Context, Department, FixAvailability, OptionError, OptionValue, Rule, RuleMeta, RuleOptions,
    Severity, Stability,
};
use ruby_ast::node::{CallNode, DefNode};
use ruby_ast::{ext, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// One resolved contribution of an enclosing scope to a qualified method
/// name (RuboCop's `Node#parent_module_name`, computed incrementally as the
/// tree is walked instead of by climbing ancestors on demand).
#[derive(Debug, Clone)]
enum Frame {
    /// Contributes this literal segment when the stack is joined with `::`.
    Named(String),
    /// Contributes nothing, but does not make the enclosing scope
    /// unresolvable: a `class_eval` with an implicit receiver, or a
    /// `Class.new`/`Module.new` block bound to a constant (whose name is
    /// instead carried by a sibling frame pushed for the assignment).
    Skip,
    /// Makes the whole enclosing `parent_module_name` unresolvable, exactly
    /// like RuboCop's `parent_module_name` returning `nil`.
    Abort,
}

/// One entry on the scope stack, pushed while entering a
/// `class`/`module`/`class << expr`/scope-forming block and popped on
/// leaving it.
#[derive(Debug, Clone)]
struct StackEntry {
    frame: Frame,
    /// Set for `class`/`module`/casgn-bound-`Class.new`/`Module.new` frames:
    /// this frame's own written name (e.g. `Foo::Bar`) and the qualified
    /// name of its enclosing scope, for RuboCop's `lookup_constant`.
    class_like: Option<(String, String)>,
    /// True for `class << expr` frames.
    is_sclass: bool,
    /// Set for `class << expr` frames whose `expr` is itself a bare method
    /// call: that call's name, for RuboCop's `found_sclass_method` fallback.
    sclass_call_name: Option<String>,
}

impl StackEntry {
    fn skip() -> Self {
        Self { frame: Frame::Skip, class_like: None, is_sclass: false, sclass_call_name: None }
    }

    fn abort() -> Self {
        Self { frame: Frame::Abort, class_like: None, is_sclass: false, sclass_call_name: None }
    }

    fn named(name: String) -> Self {
        Self {
            frame: Frame::Named(name),
            class_like: None,
            is_sclass: false,
            sclass_call_name: None,
        }
    }

    fn class_like(name: String, enclosing: String) -> Self {
        Self {
            frame: Frame::Named(name.clone()),
            class_like: Some((name, enclosing)),
            is_sclass: false,
            sclass_call_name: None,
        }
    }
}

/// Which kind of exception-handling clause a definition sits in directly,
/// for RuboCop's `@scopes` one-free-redefinition tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExceptionScope {
    Rescue,
    Ensure,
}

/// `Lint/DuplicateMethods`.
#[derive(Debug, Clone)]
pub struct DuplicateMethods {
    active_support_extensions_enabled: bool,
    /// The enclosing-scope stack, outermost first.
    stack: Vec<StackEntry>,
    /// Per currently-open `CallNode`, whether it pushed a `stack` entry (so
    /// `leave` knows whether to pop one).
    call_pushed_frame: Vec<bool>,
    /// Nearest-enclosing `def`/`defs` raw names, innermost last, for
    /// RuboCop's `method_key` nested-definition qualifier.
    def_names: Vec<Vec<u8>>,
    /// RuboCop's `@definitions`: dedup key -> the span of the most recent
    /// definition recorded under that key.
    definitions: HashMap<String, Span>,
    /// RuboCop's `@scopes[:rescue]`/`@scopes[:ensure]`: keys that already
    /// used their one free redefinition inside a `rescue`/`ensure` branch.
    rescue_relaxed: HashSet<String>,
    ensure_relaxed: HashSet<String>,
}

impl DuplicateMethods {
    /// RuboCop's `Node#parent_module_name`, built from the current stack:
    /// `None` when any enclosing frame is unresolvable.
    fn joined_scope(&self) -> Option<String> {
        let mut parts = Vec::with_capacity(self.stack.len());
        for entry in &self.stack {
            match &entry.frame {
                Frame::Named(s) => parts.push(s.clone()),
                Frame::Skip => {}
                Frame::Abort => return None,
            }
        }
        Some(if parts.is_empty() { "Object".to_string() } else { parts.join("::") })
    }

    /// RuboCop's `found_sclass_method` fallback target: the nearest
    /// enclosing `class << expr` frame's call name, if its subject was a
    /// bare method call.
    fn nearest_sclass_call_name(&self) -> Option<String> {
        self.stack.iter().rev().find(|e| e.is_sclass).and_then(|e| e.sclass_call_name.clone())
    }

    /// RuboCop's `lookup_constant`: finds the nearest enclosing
    /// `class`/`module`/casgn frame whose own written name (or one of its
    /// namespace segments) matches `const_name`, and returns the qualified
    /// name RuboCop's `qualified_name` would build for it.
    fn resolve_named_receiver(&self, const_name: &[u8]) -> Option<String> {
        for entry in self.stack.iter().rev() {
            let Some((own_name, enclosing)) = &entry.class_like else { continue };
            let segments: Vec<&str> = own_name.split("::").collect();
            let mut idx = segments.len();
            while idx > 0 {
                idx -= 1;
                if segments[idx].as_bytes() == const_name {
                    let namespace = (idx > 0).then(|| segments[..idx].join("::"));
                    return Some(qualified_name(enclosing, namespace.as_deref(), segments[idx]));
                }
            }
        }
        None
    }

    /// RuboCop's `method_key`.
    fn method_key(&self, full_name: &str) -> String {
        match self.def_names.last() {
            Some(name) => format!("{}.{full_name}", String::from_utf8_lossy(name)),
            None => full_name.to_string(),
        }
    }

    /// RuboCop's `node.each_ancestor(:rescue, :ensure).first&.type`.
    fn exception_scope(ctx: &Context<'_>) -> Option<ExceptionScope> {
        ctx.ancestors().iter().rev().find_map(|a| match a.kind {
            NodeKind::RescueNode => Some(ExceptionScope::Rescue),
            NodeKind::EnsureNode => Some(ExceptionScope::Ensure),
            _ => None,
        })
    }

    /// RuboCop's `source_location`.
    fn source_location(ctx: &Context<'_>, span: Span) -> String {
        let line = ctx.line_col(span.start).line;
        format!("{}:{line}", ctx.source().path().display())
    }

    /// RuboCop's `found_method`.
    fn found_method(&mut self, ctx: &mut Context<'_>, span: Span, full_name: &str) {
        let key = self.method_key(full_name);
        let scope = Self::exception_scope(ctx);
        let Some(&defined_span) = self.definitions.get(&key) else {
            self.definitions.insert(key, span);
            return;
        };
        if let Some(scope) = scope {
            let relaxed = match scope {
                ExceptionScope::Rescue => &mut self.rescue_relaxed,
                ExceptionScope::Ensure => &mut self.ensure_relaxed,
            };
            if !relaxed.contains(&key) {
                relaxed.insert(key.clone());
                self.definitions.insert(key, span);
                return;
            }
        }
        let message = format!(
            "Method `{full_name}` is defined at both {} and {}.",
            Self::source_location(ctx, defined_span),
            Self::source_location(ctx, span),
        );
        ctx.report(&Self::META, span, message);
    }

    /// RuboCop's `found_instance_method`.
    fn found_instance_method(&mut self, ctx: &mut Context<'_>, span: Span, name: &[u8]) {
        let name = String::from_utf8_lossy(name);
        let Some(raw_scope) = self.joined_scope() else {
            let Some(call_name) = self.nearest_sclass_call_name() else { return };
            let full = format!("{call_name}.{name}");
            self.found_method(ctx, span, &full);
            return;
        };
        let mut scope = tidy_sclass_scope(&raw_scope);
        if !scope.ends_with('.') {
            scope.push('#');
        }
        let full = format!("{scope}{name}");
        self.found_method(ctx, span, &full);
    }

    /// RuboCop's `check_self_receiver` (`on_defs`, `self`-receiver branch).
    /// Unlike `found_instance_method` this neither tidies a trailing
    /// `#<Class:...>` segment nor falls back to `found_sclass_method`.
    fn check_self_receiver(&mut self, ctx: &mut Context<'_>, span: Span, name: &[u8]) {
        let Some(enclosing) = self.joined_scope() else { return };
        let full = format!("{enclosing}.{}", String::from_utf8_lossy(name));
        self.found_method(ctx, span, &full);
    }

    /// RuboCop's `check_const_receiver` (`on_defs`, constant-receiver
    /// branch).
    fn check_const_receiver(
        &mut self,
        ctx: &mut Context<'_>,
        span: Span,
        name: &[u8],
        const_name: &[u8],
    ) {
        let Some(qualified) = self.resolve_named_receiver(const_name) else { return };
        let full = format!("{qualified}.{}", String::from_utf8_lossy(name));
        self.found_method(ctx, span, &full);
    }

    /// Pushes the scope frame for a `class`/`module` node.
    fn push_class_like(&mut self, ctx: &Context<'_>, name_span: Span) {
        let own_name = constant_path_text(ctx, name_span);
        let enclosing = self.joined_scope().unwrap_or_default();
        self.stack.push(StackEntry::class_like(own_name, enclosing));
    }

    /// Pushes the scope frame for a `class << expr` node.
    fn push_sclass(&mut self, ctx: &Context<'_>, subject: &Node<'_>) {
        let entry = match subject.kind() {
            NodeKind::ConstantReadNode | NodeKind::ConstantPathNode => {
                let name = constant_path_text(ctx, subject.span());
                StackEntry {
                    frame: Frame::Named(format!("#<Class:{name}>")),
                    class_like: None,
                    is_sclass: true,
                    sclass_call_name: None,
                }
            }
            NodeKind::SelfNode => {
                let outer = self.joined_scope().unwrap_or_default();
                StackEntry {
                    frame: Frame::Named(format!("#<Class:{outer}>")),
                    class_like: None,
                    is_sclass: true,
                    sclass_call_name: None,
                }
            }
            NodeKind::CallNode => {
                let call = subject.as_call_node().expect("kind matched");
                let name = String::from_utf8_lossy(call.name().as_slice()).into_owned();
                StackEntry {
                    frame: Frame::Abort,
                    class_like: None,
                    is_sclass: true,
                    sclass_call_name: Some(name),
                }
            }
            _ => StackEntry {
                frame: Frame::Abort,
                class_like: None,
                is_sclass: true,
                sclass_call_name: None,
            },
        };
        self.stack.push(entry);
    }

    /// If `call` is a scope-forming block (`class_eval`, or a
    /// `Class.new`/`Module.new` block bound to a constant), pushes its
    /// frame and returns `true`. Any other block-carrying call pushes an
    /// unresolvable frame (RuboCop's `parent_module_name_for_block`
    /// aborting for anything that is not `class_eval` or a
    /// constant-bound `Class.new`/`Module.new`).
    fn maybe_push_call_scope(&mut self, ctx: &Context<'_>, call: &CallNode<'_>) -> bool {
        if call.block().is_none() {
            return false;
        }
        let name = call.name().as_slice();
        let entry = if name == b"class_eval" {
            match call.receiver() {
                None => StackEntry::skip(),
                Some(recv) if is_const_ref(&recv) => {
                    StackEntry::named(constant_path_text(ctx, recv.span()))
                }
                Some(_) => StackEntry::abort(),
            }
        } else if name == b"new" && call.receiver().is_some_and(|r| is_class_or_module_const(&r)) {
            match constant_write_name(ctx, call.location().span()) {
                Some(n) => {
                    let enclosing = self.joined_scope().unwrap_or_default();
                    StackEntry::class_like(n, enclosing)
                }
                None => StackEntry::abort(),
            }
        } else {
            StackEntry::abort()
        };
        self.stack.push(entry);
        true
    }

    /// RuboCop's `on_send`, dispatching on the restricted method names.
    fn handle_call_send(&mut self, ctx: &mut Context<'_>, call: &CallNode<'_>, span: Span) {
        if call.receiver().is_some() {
            return;
        }
        match call.name().as_slice() {
            b"alias_method" => self.handle_alias_method(ctx, call, span),
            name @ (b"attr" | b"attr_reader" | b"attr_writer" | b"attr_accessor") => {
                self.handle_attr(ctx, call, span, name);
            }
            b"delegate" if self.active_support_extensions_enabled => {
                self.handle_delegate(ctx, call, span);
            }
            _ => {}
        }
    }

    /// RuboCop's `alias_method?` matcher plus the `on_send` branch that
    /// consumes it.
    fn handle_alias_method(&mut self, ctx: &mut Context<'_>, call: &CallNode<'_>, span: Span) {
        let Some(args) = call.arguments() else { return };
        let items: Vec<Node<'_>> = args.arguments().iter().collect();
        let [new_arg, old_arg] = items.as_slice() else { return };
        let Some(new_name) = sym_value(new_arg) else { return };
        let Some(old_name) = sym_value(old_arg) else { return };
        if new_name == old_name || has_if_ancestor(ctx) {
            return;
        }
        self.found_instance_method(ctx, span, &new_name);
    }

    /// RuboCop's `attribute_accessor?` matcher plus `on_attr`/`found_attr`.
    fn handle_attr(&mut self, ctx: &mut Context<'_>, call: &CallNode<'_>, span: Span, name: &[u8]) {
        let Some(args) = call.arguments() else { return };
        let items: Vec<Node<'_>> = args.arguments().iter().collect();
        if items.is_empty() {
            return;
        }
        let (targets, readable, writable): (&[Node<'_>], bool, bool) = match name {
            b"attr" => {
                let writable = items.len() == 2 && items[1].as_true_node().is_some();
                (&items[..1], true, writable)
            }
            b"attr_reader" => (&items, true, false),
            b"attr_writer" => (&items, false, true),
            _ => (&items, true, true),
        };
        for item in targets {
            let Some(name) = sym_value(item) else { continue };
            if readable {
                self.found_instance_method(ctx, span, &name);
            }
            if writable {
                let mut setter = name;
                setter.push(b'=');
                self.found_instance_method(ctx, span, &setter);
            }
        }
    }

    /// RuboCop's `delegate_method?` matcher plus `on_delegate`/
    /// `delegate_prefix`. Only active when `ActiveSupportExtensionsEnabled`.
    fn handle_delegate(&mut self, ctx: &mut Context<'_>, call: &CallNode<'_>, span: Span) {
        let Some(args) = call.arguments() else { return };
        let items: Vec<Node<'_>> = args.arguments().iter().collect();
        let Some((last, names)) = items.split_last() else { return };
        if names.is_empty() {
            return;
        }
        let mut method_names = Vec::with_capacity(names.len());
        for n in names {
            let Some(v) = sym_or_str_value(n) else { return };
            method_names.push(v);
        }
        let Some(hash_elements) = hash_like_elements(last) else { return };
        let Some(to_value) =
            find_pair_value(&hash_elements, b"to").and_then(|v| sym_or_str_value(&v))
        else {
            return;
        };
        if has_if_ancestor(ctx) {
            return;
        }
        let name_prefix = find_pair_value(&hash_elements, b"prefix").and_then(|v| {
            if v.as_true_node().is_some() {
                Some(to_value.clone())
            } else {
                sym_or_str_value(&v)
            }
        });
        for name in method_names {
            let final_name = match &name_prefix {
                Some(prefix) => {
                    let mut s = prefix.clone();
                    s.push(b'_');
                    s.extend_from_slice(&name);
                    s
                }
                None => name,
            };
            self.found_instance_method(ctx, span, &final_name);
        }
    }
}

/// RuboCop's `qualified_name`.
fn qualified_name(enclosing: &str, namespace: Option<&str>, mod_name: &str) -> String {
    if enclosing == "Object" {
        namespace.map_or_else(|| mod_name.to_string(), |ns| format!("{ns}::{mod_name}"))
    } else {
        namespace.map_or_else(
            || format!("{enclosing}::{mod_name}"),
            |ns| format!("{enclosing}::{ns}::{mod_name}"),
        )
    }
}

/// RuboCop's `found_instance_method`/`found_method` sclass tidy-up:
/// `scope.sub(/(?:(?<name>.*)::#<Class:\k<name>>|#<Class:(?<name>.*)>(?:::)?)/, '\k<name>.')`.
/// Rust's `regex` crate has no backreferences, so this replicates the
/// substitution directly on the (at most one, by construction) `#<Class:`
/// occurrence a joined scope string can contain.
fn tidy_sclass_scope(scope: &str) -> String {
    let Some(i) = scope.find("#<Class:") else { return scope.to_string() };
    let inner_start = i + "#<Class:".len();
    let Some(rel_close) = scope[inner_start..].find('>') else { return scope.to_string() };
    let name = &scope[inner_start..inner_start + rel_close];
    if i >= 2 && &scope[i - 2..i] == "::" && &scope[..i - 2] == name {
        let mut result = String::with_capacity(scope.len());
        result.push_str(name);
        result.push('.');
        result.push_str(&scope[inner_start + rel_close + 1..]);
        return result;
    }
    let mut after = inner_start + rel_close + 1;
    if scope[after..].starts_with("::") {
        after += 2;
    }
    let mut result = String::with_capacity(scope.len());
    result.push_str(&scope[..i]);
    result.push_str(name);
    result.push('.');
    result.push_str(&scope[after..]);
    result
}

/// A constant reference's written text (RuboCop's `Node#const_name`,
/// approximated from source text): the leading `::` of a top-level
/// reference is dropped, everything else (including any `::`-separated
/// path) is kept verbatim.
fn constant_path_text(ctx: &Context<'_>, span: Span) -> String {
    let text = String::from_utf8_lossy(ctx.text(span)).into_owned();
    text.strip_prefix("::").map_or_else(|| text.clone(), str::to_string)
}

/// A bare `Name` or top-level `::Name` constant reference. Shape check
/// delegated to [`ruby_ast::ext::is_bare_or_toplevel_const`]; the name
/// comparison stays local since the shared helper only checks shape.
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

fn is_const_ref(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode)
}

fn is_class_or_module_const(node: &Node<'_>) -> bool {
    is_bare_or_toplevel_const(node, b"Class") || is_bare_or_toplevel_const(node, b"Module")
}

/// If `call_span` (a `Class.new`/`Module.new` call with a block) is
/// immediately the value of a `NAME = ...`/`Foo::NAME = ...` constant
/// assignment (RuboCop's `new_class_or_module_block?`), returns `NAME`'s
/// written text. There is no direct handle on the parent
/// `ConstantWriteNode`/`ConstantPathWriteNode` (`Context::ancestors` only
/// carries kind and span), so the assignment's left-hand side is recovered
/// from the source text between the parent's start and the call's start.
fn constant_write_name(ctx: &Context<'_>, call_span: Span) -> Option<String> {
    let parent = ctx.parent()?;
    if !matches!(parent.kind, NodeKind::ConstantWriteNode | NodeKind::ConstantPathWriteNode) {
        return None;
    }
    let between = ctx.text(Span::new(parent.span.start, call_span.start));
    let text = String::from_utf8_lossy(between);
    let trimmed = text.trim_end();
    let lhs = trimmed.strip_suffix('=').unwrap_or(trimmed).trim_end();
    let lhs = lhs.strip_prefix("::").unwrap_or(lhs);
    (!lhs.is_empty()).then(|| lhs.to_string())
}

fn sym_value(node: &Node<'_>) -> Option<Vec<u8>> {
    node.as_symbol_node().map(|s| s.unescaped().to_vec())
}

fn sym_or_str_value(node: &Node<'_>) -> Option<Vec<u8>> {
    if let Some(s) = node.as_symbol_node() {
        return Some(s.unescaped().to_vec());
    }
    node.as_string_node().map(|s| s.unescaped().to_vec())
}

fn hash_like_elements<'pr>(node: &Node<'pr>) -> Option<Vec<Node<'pr>>> {
    if let Some(kw) = node.as_keyword_hash_node() {
        return Some(kw.elements().iter().collect());
    }
    node.as_hash_node().map(|h| h.elements().iter().collect())
}

fn find_pair_value<'pr>(elements: &[Node<'pr>], key: &[u8]) -> Option<Node<'pr>> {
    elements.iter().find_map(|el| {
        let assoc = el.as_assoc_node()?;
        (sym_value(&assoc.key())?.as_slice() == key).then(|| assoc.value())
    })
}

/// RuboCop's `node.each_ancestor.any?(&:if_type?)`/`node.ancestors.any?(&:if_type?)`.
/// Whitequark models `if`, `unless`, and ternaries as a single `:if` node
/// type; Prism splits `unless` into its own kind, so both are checked.
fn has_if_ancestor(ctx: &Context<'_>) -> bool {
    ctx.ancestors().iter().any(|a| matches!(a.kind, NodeKind::IfNode | NodeKind::UnlessNode))
}

/// RuboCop's `location`: a `def`/`defs` node's `keyword.join(name)`.
fn def_location_span(n: &DefNode<'_>) -> Span {
    n.def_keyword_loc().span().join(n.name_loc().span())
}

impl Rule for DuplicateMethods {
    const META: RuleMeta = RuleMeta {
        name: "Lint/DuplicateMethods",
        department: Department::Lint,
        summary: "Checks for duplicated instance (or singleton) method definitions.",
        explanation: "\
Checks for duplicated instance (or singleton) method definitions.

Aliasing a method to itself is allowed, as it indicates that the developer
intends to suppress Ruby's method redefinition warnings.

```ruby
# bad
def foo
  1
end

def foo
  2
end

# bad
def foo
  1
end

alias foo bar

# good
def foo
  1
end

def bar
  2
end

# good
def foo
  1
end

alias bar foo

# good
alias foo foo
def foo
  1
end

# good
alias_method :foo, :foo
def foo
  1
end
```

With `AllCops: ActiveSupportExtensionsEnabled: true`, a `delegate` call that
shadows an existing definition is also flagged:

```ruby
# bad
def foo
  1
end

delegate :foo, to: :bar

# good
def foo
  1
end

delegate :baz, to: :bar
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::ClassNode,
            NodeKind::ModuleNode,
            NodeKind::SingletonClassNode,
            NodeKind::DefNode,
            NodeKind::AliasMethodNode,
            NodeKind::CallNode,
        ],
        config: &[],
        blind_spots: "\
Scope resolution (`Node#parent_module_name`) is reconstructed from a stack
pushed while walking the tree rather than by climbing ancestors on demand,
but follows the same rules: `class`/`module`/`class << expr` nesting,
`class_eval` (implicit or constant-receiver), and a `Class.new`/`Module.new`
block bound to a constant assignment all resolve; any other block (a
`describe` block, `.each`, a `Class.new` bound to a local variable, ...)
makes the enclosing scope unresolvable, matching RuboCop's own behavior of
silently skipping definitions whose scope it cannot determine.

The `rescue`/`ensure` one-free-redefinition allowance
(`found_method`'s `@scopes`) is keyed only by clause kind, exactly as
upstream: two unrelated methods redefined once each across all of a file's
`rescue` clauses share the same allowance bucket.

`def A.foo`/`Foo::Bar.foo`-style constant receivers are resolved by matching
the constant's simple name against enclosing `class`/`module`/dynamic
`casgn` scopes (RuboCop's `lookup_constant`); a receiver naming an unrelated
top-level constant is not resolved to it (RuboCop does not attempt
whole-program constant resolution either).

`source_location` uses the linted file's own path as given (RuboCop's
`smart_path`, relative to `Dir.pwd`, is not replicated: a diagnostic
constructed with an absolute or repo-relative path that differs from the
path the file was read under will not match).",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let active_support_extensions_enabled = options
            .peer("AllCops", "ActiveSupportExtensionsEnabled")
            .and_then(OptionValue::as_bool)
            .unwrap_or(false);
        Ok(Self {
            active_support_extensions_enabled,
            stack: Vec::new(),
            call_pushed_frame: Vec::new(),
            def_names: Vec::new(),
            definitions: HashMap::new(),
            rescue_relaxed: HashSet::new(),
            ensure_relaxed: HashSet::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.stack.clear();
        self.call_pushed_frame.clear();
        self.def_names.clear();
        self.definitions.clear();
        self.rescue_relaxed.clear();
        self.ensure_relaxed.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::ClassNode { .. } => {
                let n = node.as_class_node().expect("kind matched");
                self.push_class_like(ctx, n.constant_path().span());
            }
            Node::ModuleNode { .. } => {
                let n = node.as_module_node().expect("kind matched");
                self.push_class_like(ctx, n.constant_path().span());
            }
            Node::SingletonClassNode { .. } => {
                let n = node.as_singleton_class_node().expect("kind matched");
                self.push_sclass(ctx, &n.expression());
            }
            Node::DefNode { .. } => {
                let n = node.as_def_node().expect("kind matched");
                let name = n.name();
                let raw_name = name.as_slice();
                if !has_if_ancestor(ctx) {
                    let span = def_location_span(&n);
                    match n.receiver() {
                        None => self.found_instance_method(ctx, span, raw_name),
                        Some(recv) => match recv.kind() {
                            NodeKind::SelfNode => self.check_self_receiver(ctx, span, raw_name),
                            NodeKind::ConstantReadNode => {
                                let cn = recv.as_constant_read_node().expect("kind matched");
                                self.check_const_receiver(
                                    ctx,
                                    span,
                                    raw_name,
                                    cn.name().as_slice(),
                                );
                            }
                            NodeKind::ConstantPathNode => {
                                let cn = recv.as_constant_path_node().expect("kind matched");
                                if let Some(id) = cn.name() {
                                    self.check_const_receiver(ctx, span, raw_name, id.as_slice());
                                }
                            }
                            _ => {}
                        },
                    }
                }
                self.def_names.push(raw_name.to_vec());
            }
            Node::AliasMethodNode { .. } => {
                let n = node.as_alias_method_node().expect("kind matched");
                let (Some(new_sym), Some(old_sym)) =
                    (n.new_name().as_symbol_node(), n.old_name().as_symbol_node())
                else {
                    return;
                };
                let new_name = new_sym.unescaped().to_vec();
                let old_name = old_sym.unescaped().to_vec();
                if new_name == old_name || has_if_ancestor(ctx) {
                    return;
                }
                self.found_instance_method(ctx, node.span(), &new_name);
            }
            Node::CallNode { .. } => {
                let call = node.as_call_node().expect("kind matched");
                self.handle_call_send(ctx, &call, node.span());
                let pushed = self.maybe_push_call_scope(ctx, &call);
                self.call_pushed_frame.push(pushed);
            }
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        match node {
            Node::ClassNode { .. } | Node::ModuleNode { .. } | Node::SingletonClassNode { .. } => {
                self.stack.pop();
            }
            Node::DefNode { .. } => {
                self.def_names.pop();
            }
            Node::CallNode { .. } if self.call_pushed_frame.pop().unwrap_or(false) => {
                self.stack.pop();
            }
            _ => {}
        }
    }
}
