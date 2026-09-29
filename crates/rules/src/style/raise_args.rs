//! `Style/RaiseArgs`, ported from RuboCop's
//! `lib/rubocop/cop/style/raise_args.rb`.
//!
//! # Node-shape notes
//!
//! `node.command?(:raise)`/`command?(:fail)` (rubocop-ast's `!receiver &&
//! method?(name)`) becomes a direct `CallNode::receiver().is_none()` check:
//! a receiver-less call can never be `&.`, so no separate safe-navigation
//! guard is needed.
//!
//! `ACCEPTABLE_ARG_TYPES` (`hash forwarded_restarg splat forwarded_kwrestarg
//! forwarded_args`) collapses two whitequark pairs into one Prism node kind
//! each: named `*args` and anonymous `*` both parse as [`NodeKind::SplatNode`]
//! (told apart only by [`SplatNode::expression`] being `Some`/`None`, and
//! both are acceptable here), while named `**kwargs` and anonymous `**` both
//! parse as [`NodeKind::AssocSplatNode`] but only the anonymous form
//! (`value().is_none()`) is in the upstream list -- a named `**kwargs` is
//! *not* acceptable, matching upstream's asymmetry exactly (only
//! `forwarded_kwrestarg`, never plain `kwsplat`, appears in the list).
//! `hash` covers both an explicit braced literal ([`NodeKind::HashNode`])
//! and bare keyword arguments ([`NodeKind::KeywordHashNode`]), which
//! whitequark folds into one `:hash` type either way.
//!
//! # `requires_parens?`
//!
//! Upstream's `node.parent && requires_parens?(node.parent)` reads the
//! *whitequark* parent, which is the `and`/`or` node directly, or (since
//! whitequark elides `begin` for a single statement) the ternary `if` node
//! directly, of a `raise`/`fail` call that is its sole operand/branch.
//! Prism always wraps a ternary branch in a [`NodeKind::StatementsNode`]
//! -- but Ruby's grammar only ever allows a *single* expression there (a
//! `;`-separated sequence needs an explicit `(...)` group, which would
//! insert its own ancestor and break the shape below anyway), so looking
//! two ancestors up when the immediate parent is that wrapper reproduces
//! the same check with no extra statement-count bookkeeping.
//! [`RaiseArgs::ternary_ifs`] records, as the walk reaches each
//! [`NodeKind::IfNode`], whether it is a real ternary (no `if_keyword_loc`)
//! -- the one fact [`context::NodeInfo`]'s bare kind/span pair does not
//! carry -- so [`requires_parens`] can tell a ternary's `if`/`else` branch
//! from an ordinary `if`/`unless` block's.

use std::collections::HashSet;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    NodeInfo, OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{ArgumentsNode, CallNode, NodeList};
use ruby_ast::{ext, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const EXPLODED_MSG: &str = "Provide an exception class and message as arguments to `%<method>s`.";
const COMPACT_MSG: &str = "Provide an exception object as an argument to `%<method>s`.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Compact,
    Exploded,
}

/// Checks the arguments passed to raise/fail.
#[derive(Debug, Clone)]
pub struct RaiseArgs {
    style: Style,
    /// `AllowedCompactTypes`.
    allowed_compact_types: Vec<String>,
    /// Spans of every [`NodeKind::IfNode`] seen so far that is a real
    /// ternary (`if_keyword_loc().is_none()`), populated as the walk
    /// reaches each one -- always before a `raise`/`fail` call nested
    /// inside one of its branches is reached. See the module doc.
    ternary_ifs: HashSet<Span>,
}

impl RaiseArgs {
    /// RuboCop's `use_new_method?`.
    fn use_new_method<'pr>(first_arg: &Node<'pr>) -> Option<CallNode<'pr>> {
        let call = first_arg.as_call_node()?;
        if call.receiver().is_some() && call.name().as_slice() == b"new" {
            Some(call)
        } else {
            None
        }
    }

    /// RuboCop's `ACCEPTABLE_ARG_TYPES.include?(arg.type)`. See the module
    /// doc for the whitequark/Prism type-list mapping.
    fn is_acceptable_arg_type(arg: &Node<'_>) -> bool {
        match arg.kind() {
            NodeKind::HashNode
            | NodeKind::KeywordHashNode
            | NodeKind::SplatNode
            | NodeKind::ForwardingArgumentsNode => true,
            NodeKind::AssocSplatNode => {
                arg.as_assoc_splat_node().is_some_and(|n| n.value().is_none())
            }
            _ => false,
        }
    }

    /// RuboCop's `acceptable_exploded_args?`, applied to the `.new(...)`
    /// call's own arguments (`None` when it takes none at all, e.g. `Ex.new`).
    fn acceptable_exploded_args(new_args: Option<ArgumentsNode<'_>>) -> bool {
        let Some(new_args) = new_args else { return false };
        let list = new_args.arguments();
        if list.len() > 1 {
            return true;
        }
        let Some(arg) = list.iter().next() else { return false };
        Self::is_acceptable_arg_type(&arg)
    }

    /// RuboCop's `allowed_non_exploded_type?`.
    fn allowed_non_exploded_type(&self, first_arg: &CallNode<'_>) -> bool {
        let Some(receiver) = first_arg.receiver() else { return false };
        let Some(name) = ext::const_name(&receiver) else { return false };
        self.allowed_compact_types.contains(&name)
    }

    /// RuboCop's `correction_compact_to_exploded`.
    fn correction_compact_to_exploded(
        ctx: &Context<'_>,
        call: &CallNode<'_>,
        new_call: &CallNode<'_>,
        parens: bool,
    ) -> String {
        let exception_node = new_call.receiver().expect("checked by use_new_method");
        let message_node = new_call.arguments().and_then(|a| a.arguments().iter().next());
        let mut arguments = String::from_utf8_lossy(ctx.text(exception_node.span())).into_owned();
        if let Some(message) = &message_node {
            arguments.push_str(", ");
            arguments.push_str(&String::from_utf8_lossy(ctx.text(message.span())));
        }
        let method = method_name_str(call);
        if parens {
            format!("{method}({arguments})")
        } else {
            format!("{method} {arguments}")
        }
    }

    /// RuboCop's `correction_exploded_to_compact`. `None` when the raise
    /// call carries more than one message argument (upstream's `return if
    /// message_nodes.size > 1`, which leaves the offense reported with no
    /// fix attached).
    fn correction_exploded_to_compact(
        ctx: &Context<'_>,
        call: &CallNode<'_>,
        args: &NodeList<'_>,
        parens: bool,
    ) -> Option<String> {
        let mut iter = args.iter();
        let exception_node = iter.next().expect("checked args.len() > 1");
        let message_nodes: Vec<Node<'_>> = iter.collect();
        if message_nodes.len() > 1 {
            return None;
        }
        let argument = String::from_utf8_lossy(ctx.text(message_nodes[0].span())).into_owned();
        let exception_class = match exception_node.as_call_node().and_then(|c| c.receiver()) {
            Some(receiver) => String::from_utf8_lossy(ctx.text(receiver.span())).into_owned(),
            None => String::from_utf8_lossy(ctx.text(exception_node.span())).into_owned(),
        };
        let method = method_name_str(call);
        Some(if parens {
            format!("{method}({exception_class}.new({argument}))")
        } else {
            format!("{method} {exception_class}.new({argument})")
        })
    }

    /// RuboCop's `check_exploded`.
    fn check_exploded(&self, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        let Some(call_args) = call.arguments() else { return };
        let call_arg_list = call_args.arguments();
        if call_arg_list.len() != 1 {
            return;
        }
        let first_arg = call_arg_list.iter().next().expect("checked len == 1");
        let Some(new_call) = Self::use_new_method(&first_arg) else { return };
        if Self::acceptable_exploded_args(new_call.arguments()) {
            return;
        }
        if self.allowed_non_exploded_type(&new_call) {
            return;
        }
        let parens = requires_parens(ctx, &self.ternary_ifs);
        let replacement = Self::correction_compact_to_exploded(ctx, call, &new_call, parens);
        let method = method_name_str(call);
        let message = EXPLODED_MSG.replacen("%<method>s", method, 1);
        let span = call.as_node().span();
        let fix = Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::replace(span, replacement.into_bytes())],
        };
        ctx.report_with_fix(&<Self as Rule>::META, span, message, fix);
    }

    /// RuboCop's `check_compact`.
    fn check_compact(&self, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        let Some(call_args) = call.arguments() else { return };
        let arg_list = call_args.arguments();
        if arg_list.len() <= 1 {
            return;
        }
        let mut iter = arg_list.iter();
        let exception = iter.next().expect("checked len > 1");
        if let Some(exception_call) = exception.as_call_node() {
            let is_hash_first =
                exception_call.arguments().and_then(|a| a.arguments().iter().next()).is_some_and(
                    |first| matches!(first.kind(), NodeKind::HashNode | NodeKind::KeywordHashNode),
                );
            if is_hash_first {
                return;
            }
        }
        let method = method_name_str(call);
        let message = COMPACT_MSG.replacen("%<method>s", method, 1);
        let span = call.as_node().span();
        let parens = requires_parens(ctx, &self.ternary_ifs);
        match Self::correction_exploded_to_compact(ctx, call, &arg_list, parens) {
            Some(replacement) => {
                let fix = Fix {
                    applicability: Applicability::Unsafe,
                    edits: vec![Edit::replace(span, replacement.into_bytes())],
                };
                ctx.report_with_fix(&<Self as Rule>::META, span, message, fix);
            }
            None => ctx.report(&<Self as Rule>::META, span, message),
        }
    }
}

/// `call.name()` as `"raise"` or `"fail"` (the only two shapes reaching
/// either check function).
fn method_name_str(call: &CallNode<'_>) -> &'static str {
    if call.name().as_slice() == b"fail" {
        "fail"
    } else {
        "raise"
    }
}

/// RuboCop's `requires_parens?(node.parent)`. See the module doc.
fn requires_parens(ctx: &Context<'_>, ternary_ifs: &HashSet<Span>) -> bool {
    let ancestors = ctx.ancestors();
    let Some(&NodeInfo { kind, .. }) = ancestors.last() else { return false };
    match kind {
        NodeKind::AndNode | NodeKind::OrNode => true,
        NodeKind::StatementsNode => {
            ancestors.len() >= 2
                && ancestors[ancestors.len() - 2].kind == NodeKind::IfNode
                && ternary_ifs.contains(&ancestors[ancestors.len() - 2].span)
        }
        _ => false,
    }
}

/// RuboCop's `node.command?(:raise) || node.command?(:fail)`: a
/// receiver-less call named `raise`/`fail` (never `&.`, since a
/// receiver-less call cannot be safe-navigation).
fn is_raise_or_fail(call: &CallNode<'_>) -> bool {
    call.receiver().is_none() && matches!(call.name().as_slice(), b"raise" | b"fail")
}

impl Rule for RaiseArgs {
    const META: RuleMeta = RuleMeta {
        name: "Style/RaiseArgs",
        department: Department::Style,
        summary: "Checks the arguments passed to raise/fail.",
        explanation: "\
Checks the args passed to `fail` and `raise`.

Exploded style (default) enforces passing the exception class and message
arguments separately, rather than constructing an instance of the error.

Compact style enforces constructing an error instance.

Both styles allow passing just a message, or an error instance when there is more
than one argument.

The exploded style has an `AllowedCompactTypes` configuration
option that takes an `Array` of exception name Strings.

@safety
  This cop is unsafe because `raise Foo` calls `Foo.exception`, not `Foo.new`.

```ruby
# EnforcedStyle: exploded (default)
# bad
raise StandardError.new('message')

# good
raise StandardError, 'message'
fail 'message'
raise MyCustomError
raise MyCustomError.new(arg1, arg2, arg3)
raise MyKwArgError.new(key1: val1, key2: val2)

# With `AllowedCompactTypes` set to ['MyWrappedError']
raise MyWrappedError.new(obj)
raise MyWrappedError.new(obj), 'message'
```

```ruby
# EnforcedStyle: compact
# bad
raise StandardError, 'message'
raise RuntimeError, arg1, arg2, arg3

# good
raise StandardError.new('message')
raise MyCustomError
raise MyCustomError.new(arg1, arg2, arg3)
fail 'message'
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::IfNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("exploded"),
                allowed: &["compact", "exploded"],
                doc: "Which style to enforce for `raise`/`fail` arguments.",
            },
            ConfigOption {
                name: "AllowedCompactTypes",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Exception class names allowed to use compact style regardless of `EnforcedStyle`.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "compact" => Style::Compact,
            _ => Style::Exploded,
        };
        Ok(Self {
            style,
            allowed_compact_types: options.str_list("AllowedCompactTypes"),
            ternary_ifs: HashSet::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.ternary_ifs.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::IfNode => {
                if let Some(if_node) = node.as_if_node() {
                    if if_node.if_keyword_loc().is_none() {
                        self.ternary_ifs.insert(node.span());
                    }
                }
            }
            NodeKind::CallNode => {
                let Some(call) = node.as_call_node() else { return };
                if !is_raise_or_fail(&call) {
                    return;
                }
                match self.style {
                    Style::Compact => self.check_compact(&call, ctx),
                    Style::Exploded => self.check_exploded(&call, ctx),
                }
            }
            _ => {}
        }
    }
}
