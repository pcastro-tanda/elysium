//! `Lint/Debugger`, ported from RuboCop's `lib/rubocop/cop/lint/debugger.rb`.
//!
//! Prism gives every method dispatch -- dotted, bare, safe-navigation, or an
//! attribute/index writer -- the same [`NodeKind::CallNode`], so upstream's
//! `:call`-ancestor check and `chained_method_name` receiver walk both
//! collapse to plain `CallNode` handling; no separate `:send`/`:csend`
//! distinction is needed. Upstream's `BLOCK_TYPES`
//! (`block`/`numblock`/`itblock`/`kwbegin`) and `lambda_or_proc?` ancestor
//! checks are combined with `||` into a single outcome (stop treating the
//! call as "assumed usage", i.e. report it normally), and every one of
//! those cases -- `{}`/`do...end` on any call including bare
//! `lambda`/`proc`/`Proc.new`, and a bare `begin...end` -- collapses to
//! just [`NodeKind::BlockNode`] and [`NodeKind::BeginNode`] in Prism, plus
//! [`NodeKind::LambdaNode`] for the stabby `->(){}` form RuboCop's
//! whitequark-only `LambdaNode` distinction also covers; see
//! [`assumed_usage_context`].
//!
//! A `CallNode`'s own span always extends through its attached block (unlike
//! whitequark's separate block-wrapper node), so the offense span and
//! message both use [`call_span_excluding_block`] to recover RuboCop's
//! `send_node.source`.

use std::collections::HashSet;

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, OptionValue,
    Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Remove debugger entry point `{source}`.";

/// The `DebuggerMethods` groups from `config/default.yml`, flattened.
const DEFAULT_METHODS: &[&str] = &[
    "binding.irb",
    "Kernel.binding.irb",
    "byebug",
    "remote_byebug",
    "Kernel.byebug",
    "Kernel.remote_byebug",
    "page.save_and_open_page",
    "page.save_and_open_screenshot",
    "page.save_page",
    "page.save_screenshot",
    "save_and_open_page",
    "save_and_open_screenshot",
    "save_page",
    "save_screenshot",
    "binding.b",
    "binding.break",
    "Kernel.binding.b",
    "Kernel.binding.break",
    "binding.pry",
    "binding.remote_pry",
    "binding.pry_remote",
    "Kernel.binding.pry",
    "Kernel.binding.remote_pry",
    "Kernel.binding.pry_remote",
    "Pry.rescue",
    "pry",
    "debugger",
    "Kernel.debugger",
    "jard",
    "binding.console",
];

/// The `DebuggerRequires` groups from `config/default.yml`, flattened.
const DEFAULT_REQUIRES: &[&str] = &["debug/open", "debug/start"];

/// Checks for debug entry points (`binding.pry`, `debugger`, `save_screenshot`,
/// a configured `require`, ...) left in the code.
#[derive(Debug, Clone)]
pub struct Debugger {
    /// Flattened `DebuggerMethods` (RuboCop's `debugger_methods`).
    methods: HashSet<String>,
    /// Flattened `DebuggerRequires` (RuboCop's `debugger_requires`).
    requires: HashSet<String>,
}

impl Debugger {
    /// RuboCop's `debugger_method?`.
    fn is_debugger_method(&self, call: &CallNode<'_>) -> bool {
        self.methods.contains(&chained_method_name(call))
    }

    /// RuboCop's `debugger_require?`.
    fn is_debugger_require(&self, call: &CallNode<'_>) -> bool {
        if call.name().as_slice() != b"require" {
            return false;
        }
        let Some(args) = call.arguments() else { return false };
        let list = args.arguments();
        if list.len() != 1 {
            return false;
        }
        let argument = list.iter().next().expect("checked len == 1");
        if argument.kind() != NodeKind::StringNode {
            return false;
        }
        let string_node = argument.as_string_node().expect("kind matched");
        self.requires.contains(&String::from_utf8_lossy(string_node.unescaped()).into_owned())
    }
}

impl Rule for Debugger {
    const META: RuleMeta = RuleMeta {
        name: "Lint/Debugger",
        department: Department::Lint,
        summary: "Checks for debugger calls.",
        explanation: "\
Checks for debug calls (such as `debugger` or `binding.pry`) that should not \
be kept for production code.

The cop can be configured using `DebuggerMethods`. By default, a number of \
gems' debug entrypoints are configured (`Kernel`, `Byebug`, `Capybara`, \
`debug.rb`, `Pry`, `Rails`, `RubyJard`, and `WebConsole`). A specific default \
group can be disabled by setting it to `~`/`false`, and additional methods \
can be added under a new group name.

Gems that start a debugging session as a side effect of a bare `require` \
(such as `require 'debug/start'`) are configured the same way through \
`DebuggerRequires`.

```ruby
# bad
def some_method
  binding.pry
  do_something
end

# bad
def some_method
  byebug
  do_something
end

# good
def some_method
  do_something
end
```
",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[
            ConfigOption {
                name: "DebuggerMethods",
                default: ConfigDefault::Nil,
                allowed: &[],
                doc: "\
Debugger entry-point method chains, grouped by gem; a group set to `~`/\
`false` is removed, and a new group name adds its methods. Defaults (flattened): \
binding.irb, Kernel.binding.irb, byebug, remote_byebug, Kernel.byebug, \
Kernel.remote_byebug, page.save_and_open_page, page.save_and_open_screenshot, \
page.save_page, page.save_screenshot, save_and_open_page, \
save_and_open_screenshot, save_page, save_screenshot, binding.b, \
binding.break, Kernel.binding.b, Kernel.binding.break, binding.pry, \
binding.remote_pry, binding.pry_remote, Kernel.binding.pry, \
Kernel.binding.remote_pry, Kernel.binding.pry_remote, Pry.rescue, pry, \
debugger, Kernel.debugger, jard, binding.console.",
            },
            ConfigOption {
                name: "DebuggerRequires",
                default: ConfigDefault::Nil,
                allowed: &[],
                doc: "\
Bare `require` arguments that start a debugging session as a side effect, \
grouped the same way as `DebuggerMethods`. Defaults (flattened): \
debug/open, debug/start.",
            },
        ],
        blind_spots: "\
`assumed_usage_context?`'s ancestor scan only recognises Prism's own \
`NodeKind::BlockNode`/`BeginNode`/`LambdaNode`; RuboCop's whitequark-only \
`numblock`/`itblock` distinction never arises since Prism represents every \
block body (named params, `_1`, or `it`) with one `BlockNode` kind.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            methods: resolve_names(options.get("DebuggerMethods"), DEFAULT_METHODS),
            requires: resolve_names(options.get("DebuggerRequires"), DEFAULT_REQUIRES),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Node::CallNode { .. } = node else { return };
        let call = node.as_call_node().expect("kind matched");
        if assumed_usage_context(&call, ctx) {
            return;
        }
        if !self.is_debugger_method(&call) && !self.is_debugger_require(&call) {
            return;
        }
        let span = call_span_excluding_block(&call);
        let source = String::from_utf8_lossy(ctx.text(span));
        ctx.report(&Self::META, span, MSG.replace("{source}", &source));
    }
}

/// RuboCop's `chained_method_name`: builds `receiver1.receiver2....method`
/// by walking the receiver chain. A `CallNode` receiver contributes its own
/// method name and the walk recurses into its own receiver; a constant
/// receiver contributes its whole qualified name in one step ([`const_name`],
/// RuboCop's `Node#const_name`) and always ends the walk there, exactly like
/// upstream's generic `Node#receiver` node-pattern matcher returning `nil`
/// for anything that isn't itself a call.
fn chained_method_name(call: &CallNode<'_>) -> String {
    let mut parts = vec![String::from_utf8_lossy(call.name().as_slice()).into_owned()];
    let mut receiver = call.receiver();
    while let Some(node) = receiver {
        match node.kind() {
            NodeKind::CallNode => {
                let inner = node.as_call_node().expect("kind matched");
                parts.push(String::from_utf8_lossy(inner.name().as_slice()).into_owned());
                receiver = inner.receiver();
            }
            NodeKind::ConstantReadNode | NodeKind::ConstantPathNode => {
                if let Some(name) = const_name(&node) {
                    parts.push(name);
                }
                receiver = None;
            }
            _ => receiver = None,
        }
    }
    parts.reverse();
    parts.join(".")
}

/// RuboCop-AST's `Node#const_name`: the whole qualified name of a constant
/// (path) node, e.g. `Foo::Bar::Baz`; a leading `::` (a `ConstantPathNode`
/// with no `parent`) contributes nothing extra, matching upstream's
/// `cbase_type?` special case.
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

/// RuboCop's `assumed_usage_context?`: a call with no arguments, nested
/// somewhere inside another call's subtree, whose value is used directly
/// (as another call's direct argument/receiver, or as a literal collection
/// element or hash pair) or merely threaded through plain expressions with
/// no intervening block/`begin...end`/lambda boundary, is assumed to be
/// intentional and is not reported.
fn assumed_usage_context(call: &CallNode<'_>, ctx: &Context<'_>) -> bool {
    let has_arguments = call.arguments().is_some_and(|args| !args.arguments().is_empty());
    let ancestors = ctx.ancestors();
    let has_call_ancestor = ancestors.iter().any(|a| a.kind == NodeKind::CallNode);
    if has_arguments || !has_call_ancestor {
        return false;
    }
    if effective_parent_kind(ancestors).is_some_and(is_assumed_argument_kind) {
        return true;
    }
    !ancestors
        .iter()
        .any(|a| matches!(a.kind, NodeKind::BlockNode | NodeKind::BeginNode | NodeKind::LambdaNode))
}

/// The logical parent for `assumed_argument?` purposes: Prism wraps every
/// call's argument list in its own `NodeKind::ArgumentsNode`, a node whitequark
/// has no equivalent for (arguments are direct children of the `:send` node
/// there), so a bare sole argument's immediate ancestor here is that wrapper,
/// not the call itself. Skip exactly one such wrapper to recover the call
/// whitequark's `node.parent` would have reported.
fn effective_parent_kind(ancestors: &[linter::NodeInfo]) -> Option<NodeKind> {
    match ancestors.last() {
        Some(last) if last.kind == NodeKind::ArgumentsNode => {
            ancestors.len().checked_sub(2).and_then(|i| ancestors.get(i)).map(|a| a.kind)
        }
        Some(last) => Some(last.kind),
        None => None,
    }
}

/// RuboCop's `assumed_argument?`: `parent.call_type? || parent.literal? ||
/// parent.pair_type?`. `parent.literal?` is RuboCop-AST's `LITERALS` set;
/// `AssocNode` is Prism's hash-pair node (`k => v`/`k: v`), matching
/// `pair_type?`.
fn is_assumed_argument_kind(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::CallNode
            | NodeKind::AssocNode
            | NodeKind::ArrayNode
            | NodeKind::HashNode
            | NodeKind::StringNode
            | NodeKind::InterpolatedStringNode
            | NodeKind::XStringNode
            | NodeKind::InterpolatedXStringNode
            | NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::SymbolNode
            | NodeKind::InterpolatedSymbolNode
            | NodeKind::RegularExpressionNode
            | NodeKind::InterpolatedRegularExpressionNode
            | NodeKind::TrueNode
            | NodeKind::FalseNode
            | NodeKind::NilNode
            | NodeKind::RangeNode
            | NodeKind::ImaginaryNode
            | NodeKind::RationalNode
    )
}

/// The end of `call`'s own source, excluding any attached block: RuboCop's
/// `send_node.source`, ported since a `CallNode`'s span always extends
/// through its own attached block.
fn call_end_excluding_block(call: &CallNode<'_>) -> u32 {
    if let Some(closing) = call.closing_loc() {
        return closing.span().end;
    }
    if let Some(args) = call.arguments() {
        if let Some(last) = args.arguments().last() {
            return last.span().end;
        }
    }
    call.message_loc().map_or_else(|| call.as_node().span().start, |loc| loc.span().end)
}

/// `call`'s own span, excluding any attached block.
fn call_span_excluding_block(call: &CallNode<'_>) -> Span {
    Span::new(call.as_node().span().start, call_end_excluding_block(call))
}

/// RuboCop's `debugger_methods`/`debugger_requires`: `config.is_a?(Array) ?
/// config : config.values.flatten`.
fn resolve_names(value: Option<&OptionValue>, defaults: &[&str]) -> HashSet<String> {
    match value {
        None => defaults.iter().map(|s| (*s).to_string()).collect(),
        Some(OptionValue::List(items)) => items.iter().filter_map(option_str).collect(),
        Some(OptionValue::Map(entries)) => {
            let mut names = Vec::new();
            for (_, group) in entries {
                flatten_into(group, &mut names);
            }
            names.into_iter().collect()
        }
        Some(_) => HashSet::new(),
    }
}

/// Ruby's `Array#flatten` applied to one group's configured value: a nested
/// list contributes each of its strings and a bare string contributes
/// itself; anything else (RuboCop's `~`/`false` group-disabling sentinel) is
/// skipped, since it can never equal a real method name anyway.
fn flatten_into(value: &OptionValue, out: &mut Vec<String>) {
    match value {
        OptionValue::Str(s) => out.push(s.clone()),
        OptionValue::List(items) => items.iter().for_each(|item| flatten_into(item, out)),
        _ => {}
    }
}

fn option_str(value: &OptionValue) -> Option<String> {
    match value {
        OptionValue::Str(s) => Some(s.clone()),
        _ => None,
    }
}
