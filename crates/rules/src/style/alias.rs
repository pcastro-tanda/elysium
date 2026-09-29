//! `Style/Alias`, ported from RuboCop's
//! `lib/rubocop/cop/style/alias.rb`.
//!
//! # Node shapes
//!
//! Prism splits whitequark's single `:alias` node type in two:
//! [`NodeKind::AliasMethodNode`] for `alias new old`/`alias :new :old` (its
//! `new_name`/`old_name` are a `SymbolNode` for a bareword or quoted plain
//! symbol, or an `InterpolatedSymbolNode` for a `:"..#{}"` dsym), and
//! [`NodeKind::AliasGlobalVariableNode`] for `alias $new $old`. Upstream's
//! `alias_method_possible?` guard (`node.children.none?(&:gvar_type?)`) is
//! therefore automatic here: an `AliasGlobalVariableNode` is simply never
//! subscribed to, so it is never flagged, matching "does not register an
//! offense for alias with gvars".
//!
//! `bareword?` (`!sym_node.source.start_with?(':') || sym_node.dsym_type?`)
//! becomes [`is_bareword`]: a `SymbolNode` is bareword when it has no
//! `opening_loc` (no leading `:` in source); anything else (only
//! `InterpolatedSymbolNode` reaches here) counts as bareword too, mirroring
//! the `dsym_type?` half of the upstream `||`.
//!
//! # `scope_type`/`each_ancestor(:def)`
//!
//! Upstream's `scope_type` walks from the node's immediate parent outward,
//! stopping at the first `class`/`module` (`:lexical`), `def`/`defs`
//! (`:dynamic`), or block/numblock/itblock (`:dynamic`, or `:instance_eval`
//! when the block's owning call is `instance_eval`). Prism collapses
//! `def`/`defs` and plain/numbered/`it` blocks into single node kinds
//! ([`NodeKind::DefNode`], [`NodeKind::BlockNode`]), so the walk only needs
//! to distinguish a block's owning call name -- tracked in
//! [`Alias::block_is_instance_eval`], a per-file map from a `BlockNode`'s own
//! span to whether its owning call is named `instance_eval`, filled as every
//! `CallNode` with a literal block is visited (always before its body, since
//! the engine walks top-down).
//!
//! `alias_method_possible?`'s separate `node.each_ancestor(:def).none?`
//! check is *not* the same as `scope_type` reaching a `DefNode`: it scans
//! every ancestor at any depth, and -- since whitequark spells a receiverless
//! `def foo` as `:def` but `def some_obj.foo` as `:defs` -- only a
//! receiverless one blocks it. Prism has one `DefNode` kind for both, keyed
//! apart only by its own `receiver()`, so [`Alias::def_has_no_receiver`]
//! tracks that per `DefNode` span the same way `block_is_instance_eval`
//! tracks blocks.
//!
//! # Multi-round convergence
//!
//! `alias_method :ala, :bala` (`prefer_alias` style) is not corrected
//! straight to `alias ala bala`: upstream's `correct_alias_method_to_alias`
//! always renders symbol arguments back as `:name` (`identifier`'s
//! `":#{node.children.first}"` branch), producing `alias :ala :bala` first;
//! *that* is what a second pass's `MSG_SYMBOL_ARGS` branch then strips down
//! to `alias ala bala`. This port emits the same single-step edit each round
//! and relies on the fixture harness's fix loop (like several other
//! multi-round cases documented in `fixtures/README.md`) to reach the same
//! fixed point.

use std::collections::{HashMap, HashSet};

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    NodeInfo, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{AliasMethodNode, CallNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG_ALIAS: &str = "Use `alias_method` instead of `alias`.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    PreferAlias,
    PreferAliasMethod,
}

/// RuboCop's three-way `scope_type` result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    Lexical,
    Dynamic,
    InstanceEval,
}

/// The text of `node` the way upstream's `identifier` renders an alias
/// operand back into source: a plain/bareword symbol always becomes
/// `:name` (its unescaped value, colon-prefixed); anything else (a dsym, or
/// a non-symbol operand reached through `alias_method`'s argument slots)
/// keeps its own raw source text.
fn identifier(node: &Node<'_>, ctx: &Context<'_>) -> String {
    if let Some(sym) = node.as_symbol_node() {
        format!(":{}", String::from_utf8_lossy(sym.unescaped()))
    } else {
        String::from_utf8_lossy(ctx.text(node.span())).into_owned()
    }
}

/// RuboCop's `bareword?`.
fn is_bareword(node: &Node<'_>) -> bool {
    match node.as_symbol_node() {
        Some(sym) => sym.opening_loc().is_none(),
        None => true,
    }
}

/// RuboCop's `node.parent&.assignment?`: the immediate parent is one of
/// RuboCop-AST's `ASSIGNMENTS` node kinds.
fn parent_is_assignment(ctx: &Context<'_>) -> bool {
    ctx.parent().is_some_and(|info| {
        matches!(
            info.kind,
            NodeKind::LocalVariableWriteNode
                | NodeKind::LocalVariableAndWriteNode
                | NodeKind::LocalVariableOrWriteNode
                | NodeKind::LocalVariableOperatorWriteNode
                | NodeKind::InstanceVariableWriteNode
                | NodeKind::InstanceVariableAndWriteNode
                | NodeKind::InstanceVariableOrWriteNode
                | NodeKind::InstanceVariableOperatorWriteNode
                | NodeKind::ClassVariableWriteNode
                | NodeKind::ClassVariableAndWriteNode
                | NodeKind::ClassVariableOrWriteNode
                | NodeKind::ClassVariableOperatorWriteNode
                | NodeKind::GlobalVariableWriteNode
                | NodeKind::GlobalVariableAndWriteNode
                | NodeKind::GlobalVariableOrWriteNode
                | NodeKind::GlobalVariableOperatorWriteNode
                | NodeKind::ConstantWriteNode
                | NodeKind::ConstantAndWriteNode
                | NodeKind::ConstantOrWriteNode
                | NodeKind::ConstantOperatorWriteNode
                | NodeKind::ConstantPathWriteNode
                | NodeKind::ConstantPathAndWriteNode
                | NodeKind::ConstantPathOrWriteNode
                | NodeKind::ConstantPathOperatorWriteNode
                | NodeKind::MultiWriteNode
                | NodeKind::IndexAndWriteNode
                | NodeKind::IndexOrWriteNode
                | NodeKind::IndexOperatorWriteNode
                | NodeKind::CallOperatorWriteNode
                | NodeKind::CallAndWriteNode
                | NodeKind::CallOrWriteNode
        )
    })
}

/// Enforces the use of either `#alias` or `#alias_method` depending on
/// configuration, and (`prefer_alias`) `alias`'s bareword over symbol
/// argument form.
#[derive(Debug, Clone)]
pub struct Alias {
    style: Style,
    /// `BlockNode` span -> whether its owning call is named `instance_eval`.
    block_is_instance_eval: HashMap<Span, bool>,
    /// `DefNode` span -> whether it has no receiver (a whitequark `:def`,
    /// versus a receiverful `:defs`).
    def_has_no_receiver: HashMap<Span, bool>,
    /// Spans of `CallNode`s that are themselves a literal argument of
    /// another call -- upstream's `node.argument?`.
    call_argument_spans: HashSet<Span>,
}

impl Alias {
    /// RuboCop's `scope_type`: walks from the nearest ancestor outward,
    /// stopping at the first match.
    fn scope_type(&self, ancestors: &[NodeInfo]) -> Scope {
        for info in ancestors.iter().rev() {
            match info.kind {
                NodeKind::ClassNode | NodeKind::ModuleNode => return Scope::Lexical,
                NodeKind::DefNode => return Scope::Dynamic,
                NodeKind::BlockNode => {
                    return if self.block_is_instance_eval.get(&info.span).copied().unwrap_or(false)
                    {
                        Scope::InstanceEval
                    } else {
                        Scope::Dynamic
                    };
                }
                _ => {}
            }
        }
        Scope::Lexical
    }

    /// RuboCop's `node.each_ancestor(:def).none?`: any ancestor, at any
    /// depth, is a receiverless `def`.
    fn has_def_ancestor(&self, ancestors: &[NodeInfo]) -> bool {
        ancestors.iter().any(|info| {
            info.kind == NodeKind::DefNode
                && self.def_has_no_receiver.get(&info.span).copied().unwrap_or(false)
        })
    }

    /// RuboCop's `lexical_scope_type`.
    fn lexical_scope_type(ancestors: &[NodeInfo]) -> &'static str {
        match ancestors
            .iter()
            .rev()
            .find(|info| matches!(info.kind, NodeKind::ClassNode | NodeKind::ModuleNode))
        {
            None => "at the top level",
            Some(info) if info.kind == NodeKind::ClassNode => "in a class body",
            Some(_) => "in a module body",
        }
    }

    /// RuboCop's `on_send`: `alias_method` called with two plain symbol
    /// arguments, convertible to the `alias` keyword form.
    fn check_alias_method(&mut self, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        if self.style != Style::PreferAlias
            || call.receiver().is_some()
            || call.is_safe_navigation()
            || call.name().as_slice() != b"alias_method"
        {
            return;
        }
        if self.scope_type(ctx.ancestors()) == Scope::Dynamic {
            return;
        }
        let Some(args) = call.arguments() else { return };
        let items: Vec<Node<'_>> = args.arguments().iter().collect();
        let [new_arg, old_arg] = items.as_slice() else { return };
        if new_arg.as_symbol_node().is_none() || old_arg.as_symbol_node().is_none() {
            return;
        }
        let span = call.as_node().span();
        // RuboCop's `alias_method_value_used?`.
        if self.call_argument_spans.contains(&span) || parent_is_assignment(ctx) {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        let current = Self::lexical_scope_type(ctx.ancestors());
        let message = format!("Use `alias` instead of `alias_method` {current}.");
        let replacement =
            format!("alias {} {}", identifier(new_arg, ctx), identifier(old_arg, ctx));
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(span, replacement.into_bytes())],
        };
        ctx.report_with_fix(&Self::META, selector.span(), message, fix);
    }

    /// RuboCop's `on_alias`.
    fn check_alias(&mut self, node: &AliasMethodNode<'_>, ctx: &mut Context<'_>) {
        let ancestors = ctx.ancestors().to_vec();
        let scope = self.scope_type(&ancestors);
        if scope == Scope::InstanceEval || self.has_def_ancestor(&ancestors) {
            return;
        }
        let new_name = node.new_name();
        let old_name = node.old_name();
        if scope == Scope::Dynamic || self.style == Style::PreferAliasMethod {
            let replacement = format!(
                "alias_method {}, {}",
                identifier(&new_name, ctx),
                identifier(&old_name, ctx)
            );
            let span = node.as_node().span();
            let fix = Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, replacement.into_bytes())],
            };
            ctx.report_with_fix(&Self::META, node.keyword_loc().span(), MSG_ALIAS, fix);
        } else if !is_bareword(&new_name) && !is_bareword(&old_name) {
            let new_span = new_name.span();
            let old_span = old_name.span();
            let new_src = String::from_utf8_lossy(ctx.text(new_span)).into_owned();
            let old_src = String::from_utf8_lossy(ctx.text(old_span)).into_owned();
            let current = format!("{new_src} {old_src}");
            let preferred = format!("{} {}", &new_src[1..], &old_src[1..]);
            let message = format!("Use `alias {preferred}` instead of `alias {current}`.");
            let span = Span::new(new_span.start, old_span.end);
            let fix = Fix {
                applicability: Applicability::Safe,
                edits: vec![
                    Edit::replace(new_span, new_src.as_bytes()[1..].to_vec()),
                    Edit::replace(old_span, old_src.as_bytes()[1..].to_vec()),
                ],
            };
            ctx.report_with_fix(&Self::META, span, message, fix);
        }
    }
}

impl Rule for Alias {
    const META: RuleMeta = RuleMeta {
        name: "Style/Alias",
        department: Department::Style,
        summary: "Use alias instead of alias_method.",
        explanation: "\
Enforces the use of either `#alias` or `#alias_method` depending on
configuration. Consistent use of one or the other prevents confusion about
their different semantics (e.g., `alias` is resolved at parse time, while
`alias_method` is resolved at runtime).

It also flags uses of `alias :symbol` rather than `alias bareword`.

However, it will always enforce `alias_method` when `alias` is used in an
instance method definition and in a singleton method definition. If used
in a block, always enforce `alias_method` unless it is an `instance_eval`
block.

```ruby
# EnforcedStyle: prefer_alias (default)
# bad
alias_method :bar, :foo
alias :bar :foo

# good
alias bar foo
```

```ruby
# EnforcedStyle: prefer_alias_method
# bad
alias :bar :foo
alias bar foo

# good
alias_method :bar, :foo
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::AliasMethodNode, NodeKind::DefNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("prefer_alias"),
            allowed: &["prefer_alias", "prefer_alias_method"],
            doc: "Whether to prefer `alias` or `alias_method`.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "prefer_alias_method" => Style::PreferAliasMethod,
            _ => Style::PreferAlias,
        };
        Ok(Self {
            style,
            block_is_instance_eval: HashMap::new(),
            def_has_no_receiver: HashMap::new(),
            call_argument_spans: HashSet::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.block_is_instance_eval.clear();
        self.def_has_no_receiver.clear();
        self.call_argument_spans.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::DefNode => {
                let def = node.as_def_node().expect("kind matched");
                self.def_has_no_receiver.insert(node.span(), def.receiver().is_none());
            }
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                if let Some(block) = call.block() {
                    if let Some(block_node) = block.as_block_node() {
                        let is_instance_eval = call.name().as_slice() == b"instance_eval";
                        self.block_is_instance_eval
                            .insert(block_node.as_node().span(), is_instance_eval);
                    }
                }
                if let Some(args) = call.arguments() {
                    for arg in &args.arguments() {
                        if arg.as_call_node().is_some() {
                            self.call_argument_spans.insert(arg.span());
                        }
                    }
                }
                self.check_alias_method(&call, ctx);
            }
            NodeKind::AliasMethodNode => {
                let alias = node.as_alias_method_node().expect("kind matched");
                self.check_alias(&alias, ctx);
            }
            _ => {}
        }
    }
}
