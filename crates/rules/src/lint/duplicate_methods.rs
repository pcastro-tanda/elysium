//! `Lint/DuplicateMethods`, ported from RuboCop's `lib/rubocop/cop/lint/duplicate_methods.rb`.

use std::collections::{HashMap, HashSet};

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, OptionValue,
    Rule, RuleMeta, RuleOptions, Severity, Stability,
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

/// An anonymous `Class.new`/`Module.new` block treated as a scope of its
/// own (RuboCop 1.91's `anonymous_class_block`), resolved once when the
/// block is entered.
#[derive(Debug, Clone)]
struct AnonInfo {
    /// RuboCop's `qualified_name(anon_block.parent_module_name, nil, 'Object')`.
    base: String,
    /// RuboCop's `anon_block_scope_id`: what keeps two distinct anonymous
    /// classes apart. `None` is upstream's `nil` scope id, which puts every
    /// such block in one shared bucket.
    scope_id: Option<String>,
}

/// What a block-carrying call contributes to the two ancestor searches
/// RuboCop runs over `:block` nodes.
#[derive(Debug, Clone)]
struct BlockInfo {
    /// False for numbered-parameter/`it` blocks: upstream's
    /// `each_ancestor(:block)` only ever yields `:block` nodes, never
    /// `:numblock`/`:itblock`, so those are skipped over, not stopped at.
    plain: bool,
    /// Set when the block is an anonymous `Class.new`/`Module.new` body.
    anon: Option<AnonInfo>,
}

/// One entry on the scope stack, pushed while entering a
/// `class`/`module`/`class << expr`/block-carrying call and popped on
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
    /// True for `class << expr` frames whose `expr` is not `self`, which
    /// veto RuboCop's `anonymous_class_block`.
    sclass_non_self: bool,
    /// Set for `class << expr` frames whose `expr` is itself a bare method
    /// call: that call's name, for RuboCop's `found_sclass_method` fallback.
    sclass_call_name: Option<String>,
    /// Set for frames pushed by a call carrying a literal block.
    block: Option<BlockInfo>,
}

impl StackEntry {
    fn new(frame: Frame) -> Self {
        Self {
            frame,
            class_like: None,
            is_sclass: false,
            sclass_non_self: false,
            sclass_call_name: None,
            block: None,
        }
    }

    fn skip() -> Self {
        Self::new(Frame::Skip)
    }

    fn abort() -> Self {
        Self::new(Frame::Abort)
    }

    fn named(name: String) -> Self {
        Self::new(Frame::Named(name))
    }

    fn class_like(name: String, enclosing: String) -> Self {
        Self { class_like: Some((name.clone(), enclosing)), ..Self::new(Frame::Named(name)) }
    }

    fn with_block(mut self, block: BlockInfo) -> Self {
        self.block = Some(block);
        self
    }
}

/// Which kind of exception-handling clause a definition sits in directly,
/// for RuboCop's `@scopes` one-free-redefinition tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExceptionScope {
    Rescue,
    Ensure,
}

/// One currently-open `CallNode`, for recovering the pieces of the parent
/// node RuboCop's `anon_block_scope_id` reads off a `:block`'s parent.
#[derive(Debug, Clone)]
struct CallFrame {
    span: Span,
    /// The call's method-name location (RuboCop's `method_name`).
    message: Option<Span>,
    /// The call's receiver (RuboCop's `Node#receiver`).
    receiver: Option<Span>,
    /// True when that receiver is itself an anonymous `Class.new`/
    /// `Module.new` block, which RuboCop's `named_receiver` rejects.
    receiver_is_anon_ctor: bool,
    /// Whether this call pushed a [`StackEntry`], so `leave` knows whether
    /// to pop one.
    pushed_frame: bool,
}

/// One currently-open `def`/`defs`.
#[derive(Debug, Clone, Copy)]
struct DefFrame {
    span: Span,
    name: Span,
    /// `def self.foo`'s `self` (RuboCop's `DefNode#receiver`).
    receiver: Option<Span>,
}

/// `Lint/DuplicateMethods`.
#[derive(Debug, Clone)]
pub struct DuplicateMethods {
    active_support_extensions_enabled: bool,
    /// RuboCop's `DelegatingMethods`.
    delegating_methods: Vec<String>,
    /// The enclosing-scope stack, outermost first.
    stack: Vec<StackEntry>,
    /// Currently-open `CallNode`s, outermost first.
    call_frames: Vec<CallFrame>,
    /// Number of children of each currently-open `StatementsNode`, for
    /// telling a whitequark `begin` (two or more statements) from a body
    /// Prism wraps but whitequark elides.
    stmt_counts: Vec<usize>,
    /// Nearest-enclosing `def`/`defs`, innermost last, for RuboCop's
    /// `method_key` nested-definition qualifier.
    def_frames: Vec<DefFrame>,
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
    /// `None` when any enclosing frame is unresolvable. `extra` is an
    /// additional innermost segment (a `casgn` ancestor a stack frame does
    /// not model on its own).
    fn joined_scope_with(&self, extra: Option<&str>) -> Option<String> {
        let mut parts: Vec<&str> = Vec::with_capacity(self.stack.len() + 1);
        for entry in &self.stack {
            match &entry.frame {
                Frame::Named(s) => parts.push(s),
                Frame::Skip => {}
                Frame::Abort => return None,
            }
        }
        parts.extend(extra);
        Some(if parts.is_empty() { "Object".to_string() } else { parts.join("::") })
    }

    fn joined_scope(&self) -> Option<String> {
        self.joined_scope_with(None)
    }

    /// RuboCop's `anonymous_class_block`: the innermost enclosing `:block`
    /// ancestor, when it is an anonymous `Class.new`/`Module.new` body and
    /// no enclosing `class << expr` has a non-`self` subject.
    fn anonymous_class_block(&self) -> Option<&AnonInfo> {
        if self.stack.iter().any(|entry| entry.sclass_non_self) {
            return None;
        }
        self.stack
            .iter()
            .rev()
            .find_map(|entry| entry.block.as_ref().filter(|block| block.plain))?
            .anon
            .as_ref()
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
    fn method_key(&self, ctx: &Context<'_>, full_name: &str) -> String {
        match self.def_frames.last() {
            Some(def) => {
                format!("{}.{full_name}", String::from_utf8_lossy(ctx.text(def.name)))
            }
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
    fn found_method(
        &mut self,
        ctx: &mut Context<'_>,
        span: Span,
        full_name: &str,
        scope_id: Option<&str>,
    ) {
        let mut key = self.method_key(ctx, full_name);
        if let Some(scope_id) = scope_id {
            key.push('@');
            key.push_str(scope_id);
        }
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
        if let Some(raw_scope) = self.joined_scope() {
            let full = format!("{}{name}", humanize_scope(&raw_scope));
            self.found_method(ctx, span, &full, None);
        } else if let Some(anon) = self.anonymous_class_block().cloned() {
            let scope = if self.stack.iter().any(|entry| entry.is_sclass) {
                format!("#<Class:{}>", anon.base)
            } else {
                anon.base
            };
            let full = format!("{}{name}", humanize_scope(&scope));
            self.found_method(ctx, span, &full, anon.scope_id.as_deref());
        } else if let Some(call_name) = self.nearest_sclass_call_name() {
            let full = format!("{call_name}.{name}");
            self.found_method(ctx, span, &full, None);
        }
    }

    /// RuboCop's `check_self_receiver` (`on_defs`, `self`-receiver branch).
    /// Unlike `found_instance_method` this neither tidies a trailing
    /// `#<Class:...>` segment nor falls back to `found_sclass_method`.
    fn check_self_receiver(&mut self, ctx: &mut Context<'_>, span: Span, name: &[u8]) {
        let name = String::from_utf8_lossy(name);
        if let Some(enclosing) = self.joined_scope() {
            let full = format!("{enclosing}.{name}");
            self.found_method(ctx, span, &full, None);
        } else if let Some(anon) = self.anonymous_class_block().cloned() {
            let full = format!("{}.{name}", anon.base);
            self.found_method(ctx, span, &full, anon.scope_id.as_deref());
        }
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
        self.found_method(ctx, span, &full, None);
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
                    is_sclass: true,
                    sclass_non_self: true,
                    ..StackEntry::named(format!("#<Class:{name}>"))
                }
            }
            NodeKind::SelfNode => {
                let outer = self.joined_scope().unwrap_or_default();
                StackEntry { is_sclass: true, ..StackEntry::named(format!("#<Class:{outer}>")) }
            }
            NodeKind::CallNode => {
                let call = subject.as_call_node().expect("kind matched");
                let name = String::from_utf8_lossy(call.name().as_slice()).into_owned();
                StackEntry {
                    is_sclass: true,
                    sclass_non_self: true,
                    sclass_call_name: Some(name),
                    ..StackEntry::abort()
                }
            }
            _ => StackEntry { is_sclass: true, sclass_non_self: true, ..StackEntry::abort() },
        };
        self.stack.push(entry);
    }

    /// Pushes the scope frame for a call carrying a literal block, and
    /// records what that block contributes to RuboCop's two `:block`
    /// ancestor searches. Returns whether a frame was pushed.
    fn maybe_push_call_scope(
        &mut self,
        ctx: &Context<'_>,
        call: &CallNode<'_>,
        span: Span,
    ) -> bool {
        let Some(block) = call.block().and_then(|node| node.as_block_node()) else {
            return false;
        };
        let plain = !matches!(
            block.parameters().map(|params| params.kind()),
            Some(NodeKind::NumberedParametersNode | NodeKind::ItParametersNode)
        );
        let (entry, anon) = if !plain {
            // `parent_module_name` walks `:class`/`:module`/`:sclass`/
            // `:casgn`/`:block` ancestors only, so a `:numblock`/`:itblock`
            // is passed over instead of making the scope unresolvable.
            (StackEntry::skip(), None)
        } else if call.name().as_slice() == b"class_eval" {
            let entry = match call.receiver() {
                None => StackEntry::skip(),
                Some(recv) if is_const_ref(&recv) => {
                    StackEntry::named(constant_path_text(ctx, recv.span()))
                }
                Some(_) => StackEntry::abort(),
            };
            (entry, None)
        } else if let Some(is_class) = class_or_module_new(call) {
            self.new_class_frame(ctx, call, span, is_class)
        } else {
            (StackEntry::abort(), None)
        };
        self.stack.push(entry.with_block(BlockInfo { plain, anon }));
        true
    }

    /// The stack frame and anonymous-class info for a `Class.new`/
    /// `Module.new` block: RuboCop's `new_class_or_module_block?` (which
    /// only matches an argument-less `new` bound to a constant, and lets
    /// the `casgn` ancestor name the scope instead) on the
    /// `parent_module_name` side, and `anonymous_class_block` on the other.
    fn new_class_frame(
        &self,
        ctx: &Context<'_>,
        call: &CallNode<'_>,
        span: Span,
        is_class: bool,
    ) -> (StackEntry, Option<AnonInfo>) {
        let parent_kind = ctx.parent().map(|parent| parent.kind);
        let casgn = matches!(
            parent_kind,
            Some(NodeKind::ConstantWriteNode | NodeKind::ConstantPathWriteNode)
        );
        // `defined_module0` only names the scope for a `Class.new`/
        // `Module.new` written on a bare or top-level constant.
        let global = call.receiver().is_some_and(|recv| is_class_or_module_const(&recv));
        let named = if casgn && global { constant_write_name(ctx, span) } else { None };
        let suppresses = casgn && call.arguments().is_none();
        let entry = match (suppresses, &named) {
            (true, Some(name)) => {
                let enclosing = self.joined_scope().unwrap_or_default();
                StackEntry::class_like(name.clone(), enclosing)
            }
            (true, None) => StackEntry::skip(),
            (false, _) => StackEntry::abort(),
        };
        let anon = if parent_kind == Some(NodeKind::LocalVariableWriteNode) {
            None
        } else {
            Some(AnonInfo {
                base: anon_base(self.joined_scope_with(named.as_deref()).as_deref()),
                scope_id: self.anon_scope_id(ctx, span, is_class),
            })
        };
        (entry, anon)
    }

    /// RuboCop's `anon_block_scope_id`, read off the whitequark parent the
    /// Prism ancestor chain stands for: `ArgumentsNode`/receiver position
    /// for a `send`/`csend` parent, and a `StatementsNode` for either a
    /// `begin` (two or more statements, or parentheses) or a body
    /// whitequark does not wrap at all.
    fn anon_scope_id(&self, ctx: &Context<'_>, span: Span, is_class: bool) -> Option<String> {
        let ancestors = ctx.ancestors();
        let count = ancestors.len();
        let parent = *ancestors.last()?;
        let at = |index: usize| ancestors.get(index).copied();
        let owner = match parent.kind {
            NodeKind::ArgumentsNode => {
                let call = at(count.checked_sub(2)?)?;
                if call.kind != NodeKind::CallNode {
                    return None;
                }
                self.call_owner(call.span, is_class)
            }
            NodeKind::CallNode => self.call_owner(parent.span, is_class),
            NodeKind::ConstantWriteNode
            | NodeKind::ConstantPathWriteNode
            | NodeKind::InstanceVariableWriteNode => (None, None),
            NodeKind::StatementsNode => {
                let grandparent = at(count.checked_sub(2)?)?;
                let wrapped_in_parens = grandparent.kind == NodeKind::ParenthesesNode;
                if self.stmt_counts.last().copied().unwrap_or(1) > 1 || wrapped_in_parens {
                    let begin_parent =
                        if wrapped_in_parens { at(count.checked_sub(3)?)? } else { grandparent };
                    return (begin_parent.kind == NodeKind::BlockNode).then(|| anon_identity(span));
                }
                match grandparent.kind {
                    // A block's body: the whitequark parent is the `block`
                    // node, whose `receiver` is its own call's receiver.
                    NodeKind::BlockNode => self.call_owner(at(count.checked_sub(3)?)?.span, false),
                    NodeKind::DefNode => self.def_owner(grandparent.span),
                    _ => return None,
                }
            }
            _ => return None,
        };
        Some(match owner {
            (Some(receiver), Some(message)) => {
                format!("{}.{}", span_text(ctx, receiver), span_text(ctx, message))
            }
            _ => anon_identity(span),
        })
    }

    /// RuboCop's `named_receiver` for a `send`/`csend`/`block` parent.
    /// `is_class` vetoes it outright for a `Class.new` block passed to a
    /// call (`scope_receiver`), since the receiver-based id would be shared
    /// by every call to the same method.
    fn call_owner(&self, call_span: Span, is_class: bool) -> (Option<Span>, Option<Span>) {
        if is_class {
            return (None, None);
        }
        let Some(frame) = self.call_frames.iter().rev().find(|frame| frame.span == call_span)
        else {
            return (None, None);
        };
        if frame.receiver_is_anon_ctor {
            return (None, None);
        }
        (frame.receiver, frame.message)
    }

    /// RuboCop's `named_receiver` for a `def`/`defs` parent: only `defs`
    /// has a receiver.
    fn def_owner(&self, def_span: Span) -> (Option<Span>, Option<Span>) {
        match self.def_frames.iter().rev().find(|frame| frame.span == def_span) {
            Some(frame) => (frame.receiver, Some(frame.name)),
            None => (None, None),
        }
    }

    /// RuboCop's `on_send`. 1.91 dropped `RESTRICT_ON_SEND` (the delegating
    /// method names became configurable), so every receiverless call is
    /// dispatched here.
    fn handle_call_send(&mut self, ctx: &mut Context<'_>, call: &CallNode<'_>, span: Span) {
        if call.receiver().is_some() {
            return;
        }
        let name = call.name().as_slice();
        match name {
            b"alias_method" => self.handle_alias_method(ctx, call, span),
            b"attr" | b"attr_reader" | b"attr_writer" | b"attr_accessor" => {
                self.handle_attr(ctx, call, span, name);
            }
            b"def_delegator" | b"def_instance_delegator" => {
                self.handle_def_delegator(ctx, call, span);
            }
            b"def_delegators" | b"def_instance_delegators" => {
                self.handle_def_delegators(ctx, call, span);
            }
            _ if self.is_delegating_method(name) => self.handle_delegate(ctx, call, span),
            _ => {}
        }
    }

    /// RuboCop's `delegating_method?`: still gated on
    /// `ActiveSupportExtensionsEnabled`, but the names come from
    /// `DelegatingMethods`.
    fn is_delegating_method(&self, name: &[u8]) -> bool {
        self.active_support_extensions_enabled
            && self.delegating_methods.iter().any(|allowed| allowed.as_bytes() == name)
    }

    /// RuboCop's `delegator?` matcher (`Forwardable`'s `def_delegator`/
    /// `def_instance_delegator`): the defined name is the last of two or
    /// three symbol/string arguments.
    fn handle_def_delegator(&mut self, ctx: &mut Context<'_>, call: &CallNode<'_>, span: Span) {
        let Some(args) = call.arguments() else { return };
        let items: Vec<Node<'_>> = args.arguments().iter().collect();
        if items.len() != 2 && items.len() != 3 {
            return;
        }
        let Some(name) = literal_names(&items) else { return };
        let Some(last) = name.last() else { return };
        if has_if_ancestor(ctx) {
            return;
        }
        self.found_instance_method(ctx, span, last);
    }

    /// RuboCop's `delegators?` matcher (`Forwardable`'s `def_delegators`/
    /// `def_instance_delegators`): every argument after the accessor names
    /// a defined method.
    fn handle_def_delegators(&mut self, ctx: &mut Context<'_>, call: &CallNode<'_>, span: Span) {
        let Some(args) = call.arguments() else { return };
        let items: Vec<Node<'_>> = args.arguments().iter().collect();
        if items.len() < 2 {
            return;
        }
        let Some(names) = literal_names(&items) else { return };
        if has_if_ancestor(ctx) {
            return;
        }
        for name in &names[1..] {
            self.found_instance_method(ctx, span, name);
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

    /// RuboCop's `delegate_args` matcher plus `on_delegate`/
    /// `delegate_prefix`. Only active when `ActiveSupportExtensionsEnabled`
    /// and the call names one of `DelegatingMethods`.
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

/// RuboCop's `qualified_name(scope, nil, 'Object')` for an anonymous
/// class/module block, including the `nil` enclosing scope upstream leaves
/// interpolated as an empty string (`"::Object"`).
fn anon_base(scope: Option<&str>) -> String {
    scope.map_or_else(|| "::Object".to_string(), |scope| qualified_name(scope, None, "Object"))
}

/// RuboCop's `humanize_scope`.
fn humanize_scope(scope: &str) -> String {
    let mut scope = tidy_sclass_scope(scope);
    if !scope.ends_with('.') {
        scope.push('#');
    }
    scope
}

/// RuboCop's `anon_block_identity`: an identity unique to one anonymous
/// class/module block. Upstream spells it `path:line:begin_pos`; within one
/// file the start offset alone is just as unique, and the value is only
/// ever a dedup-key fragment, never shown.
fn anon_identity(span: Span) -> String {
    format!("#{}", span.start)
}

fn span_text(ctx: &Context<'_>, span: Span) -> String {
    String::from_utf8_lossy(ctx.text(span)).into_owned()
}

/// A constant reference whose last segment is `name` (RuboCop's
/// `(const _ :Name)`, which accepts any namespace).
fn const_last_segment_is(node: &Node<'_>, name: &[u8]) -> bool {
    match node.kind() {
        NodeKind::ConstantReadNode => {
            node.as_constant_read_node().is_some_and(|n| n.name().as_slice() == name)
        }
        NodeKind::ConstantPathNode => node
            .as_constant_path_node()
            .and_then(|path| path.name())
            .is_some_and(|id| id.as_slice() == name),
        _ => false,
    }
}

/// RuboCop's `(send (const _ {:Class :Module}) :new ...)`: `Some(true)` for
/// `Class`, `Some(false)` for `Module`.
fn class_or_module_new(call: &CallNode<'_>) -> Option<bool> {
    if call.name().as_slice() != b"new" {
        return None;
    }
    let receiver = call.receiver()?;
    if const_last_segment_is(&receiver, b"Class") {
        Some(true)
    } else if const_last_segment_is(&receiver, b"Module") {
        Some(false)
    } else {
        None
    }
}

/// RuboCop's `class_or_module_new_block?` on a node: a `Class.new`/
/// `Module.new` call carrying a literal block.
fn is_class_or_module_new_block(node: &Node<'_>) -> bool {
    node.as_call_node().is_some_and(|call| {
        call.block().and_then(|block| block.as_block_node()).is_some()
            && class_or_module_new(&call).is_some()
    })
}

/// The symbol/string values of every argument, or `None` when any argument
/// is neither.
fn literal_names(items: &[Node<'_>]) -> Option<Vec<Vec<u8>>> {
    items.iter().map(sym_or_str_value).collect()
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
/// assignment (RuboCop's `defined_module0` for a `casgn`), returns the
/// `Node#const_name` of that assignment. There is no direct handle on the
/// parent `ConstantWriteNode`/`ConstantPathWriteNode` (`Context::ancestors`
/// only carries kind and span), so the assignment's left-hand side is
/// recovered from the source text between the parent's start and the
/// call's start.
///
/// `const_name` interpolates the namespace's own `const_name`, which is
/// `nil` for anything that is not a constant: `self::A = ...` is named
/// `::A`, and a leading `::` (a `cbase` namespace) is dropped.
fn constant_write_name(ctx: &Context<'_>, call_span: Span) -> Option<String> {
    let parent = ctx.parent()?;
    if !matches!(parent.kind, NodeKind::ConstantWriteNode | NodeKind::ConstantPathWriteNode) {
        return None;
    }
    let between = ctx.text(Span::new(parent.span.start, call_span.start));
    let text = String::from_utf8_lossy(between);
    let trimmed = text.trim_end();
    let lhs = trimmed.strip_suffix('=').unwrap_or(trimmed).trim_end();
    let (first, rest) = lhs.split_once("::").map_or((lhs, None), |(a, b)| (a, Some(b)));
    let name = match (first, rest) {
        ("", Some(rest)) => rest.to_string(),
        (first, Some(rest)) if !starts_uppercase(first) => format!("::{rest}"),
        _ => lhs.to_string(),
    };
    (!name.is_empty() && name != "::").then_some(name)
}

fn starts_uppercase(text: &str) -> bool {
    text.chars().next().is_some_and(char::is_uppercase)
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

With `AllCops: ActiveSupportExtensionsEnabled: true`, a call to one of
`DelegatingMethods` (`delegate` by default) that shadows an existing
definition is also flagged:

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
```

`Forwardable`'s `def_delegator`, `def_instance_delegator`, `def_delegators`
and `def_instance_delegators` define methods too, and are recognized
regardless of `ActiveSupportExtensionsEnabled`:

```ruby
# bad
class MyClass
  extend Forwardable

  def_delegator :delegation_target, :delegated_method_name

  def delegated_method_name
  end
end
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
            NodeKind::StatementsNode,
        ],
        config: &[ConfigOption {
            name: "DelegatingMethods",
            default: ConfigDefault::StrList(&["delegate"]),
            allowed: &[],
            doc: "`delegate`-shaped macros whose arguments name defined methods.",
        }],
        blind_spots: "\
Scope resolution (`Node#parent_module_name`) is reconstructed from a stack
pushed while walking the tree rather than by climbing ancestors on demand,
but follows the same rules: `class`/`module`/`class << expr` nesting,
`class_eval` (implicit or constant-receiver), and an argument-less
`Class.new`/`Module.new` block bound to a constant assignment all resolve;
a numbered-parameter or `it` block is passed over (upstream's ancestor
searches only see `:block` nodes); any other block (a `describe` block,
`.each`, a `Class.new` bound to a local variable, ...) makes the enclosing
scope unresolvable.

An anonymous `Class.new`/`Module.new` block is then a scope of its own
(`Object`, or `::Object` when even its own enclosing scope is
unresolvable), kept apart from other anonymous classes by RuboCop's
`anon_block_scope_id`: the source text of the enclosing call's receiver
plus its method name when there is one, and otherwise an identity unique to
the block. Because `Context::ancestors` carries only kinds and spans, that
parent is recovered from the Prism chain (`ArgumentsNode` for an argument,
`StatementsNode` for a body, with two or more statements standing for
whitequark's `begin`) and from the spans of the enclosing `def`/call nodes.

Cross-file duplicate detection is not ported: everything RuboCop 1.91 does
with `AllCops/UseProjectIndex` and the `rubydex` project index
(`AllowedCrossFilePaths`, the self-alias trick and Active Support's
`silence_redefinition_of_method`/`redefine_method` markers marking a
redefinition in *another* file as intentional) has no effect here, where
each file is linted on its own. Within one file those markers change
nothing upstream either.

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
            // An explicit `~` means "unset", as it does for a cop config
            // key RuboCop's `cop_config.fetch` never sees.
            delegating_methods: match options.get("DelegatingMethods") {
                None | Some(OptionValue::Null) => vec!["delegate".to_string()],
                Some(_) => options.str_list("DelegatingMethods"),
            },
            stack: Vec::new(),
            call_frames: Vec::new(),
            stmt_counts: Vec::new(),
            def_frames: Vec::new(),
            definitions: HashMap::new(),
            rescue_relaxed: HashSet::new(),
            ensure_relaxed: HashSet::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.stack.clear();
        self.call_frames.clear();
        self.stmt_counts.clear();
        self.def_frames.clear();
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
                self.def_frames.push(DefFrame {
                    span: node.span(),
                    name: n.name_loc().span(),
                    receiver: n.receiver().map(|recv| recv.span()),
                });
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
                let span = node.span();
                self.handle_call_send(ctx, &call, span);
                let pushed_frame = self.maybe_push_call_scope(ctx, &call, span);
                let receiver = call.receiver();
                self.call_frames.push(CallFrame {
                    span,
                    message: call.message_loc().map(|loc| loc.span()),
                    receiver: receiver.as_ref().map(Node::span),
                    receiver_is_anon_ctor: receiver
                        .as_ref()
                        .is_some_and(is_class_or_module_new_block),
                    pushed_frame,
                });
            }
            Node::StatementsNode { .. } => {
                let n = node.as_statements_node().expect("kind matched");
                self.stmt_counts.push(n.body().len());
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
                self.def_frames.pop();
            }
            Node::CallNode { .. } => {
                if self.call_frames.pop().is_some_and(|frame| frame.pushed_frame) {
                    self.stack.pop();
                }
            }
            Node::StatementsNode { .. } => {
                self.stmt_counts.pop();
            }
            _ => {}
        }
    }
}
