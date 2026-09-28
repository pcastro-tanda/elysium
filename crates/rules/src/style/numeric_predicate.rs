//! `Style/NumericPredicate`, ported from RuboCop's
//! `lib/rubocop/cop/style/numeric_predicate.rb`.
//!
//! # Ancestor tracking
//!
//! Upstream's `allowed_method_name?` walks `node.each_ancestor(:send, :block)`
//! calling `.method_name` on each. In Prism a block-owning call is still one
//! `CallNode` (its `block()` field just gets populated), so there is no
//! separate `BlockNode` ancestor to track. Since `enter`/`leave` are only
//! dispatched for `CallNode` (`kinds`), [`NumericPredicate`] keeps its own
//! stack of every currently-open call's name: pushed *after* this cop's own
//! checks run (so a call is never counted as its own ancestor) and popped on
//! `leave`. The same stack's top doubles as `negated?`'s "is my immediate
//! parent a `!` call" check, since Prism gives `!expr` a `CallNode` with
//! `expr` as its direct, unwrapped receiver -- no intervening node kind can
//! sit between them, so whenever the immediate structural parent actually is
//! a `CallNode`, it is necessarily the same frame currently on top of the
//! stack.
//!
//! # `replacement_supported?`'s dead branch
//!
//! Upstream's `replacement_supported?` only rejects `>`/`<` below Ruby 2.3;
//! it is called with the *predicate* symbol (`:zero?`/`:positive?`/
//! `:negative?`) when `EnforcedStyle: comparison`, which can never equal
//! `:>`/`:<`, so the target-Ruby-version gate is unreachable for that style.
//! This port keeps that literally: the version check only applies for
//! `EnforcedStyle: predicate` converting to `>`/`<`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// Which numeric predicate/comparison a matched call represents, in either
/// direction (upstream's `REPLACEMENTS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Zero,
    Positive,
    Negative,
}

impl Kind {
    const fn predicate_name(self) -> &'static str {
        match self {
            Kind::Zero => "zero?",
            Kind::Positive => "positive?",
            Kind::Negative => "negative?",
        }
    }

    const fn comparison_op(self) -> &'static str {
        match self {
            Kind::Zero => "==",
            Kind::Positive => ">",
            Kind::Negative => "<",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Predicate,
    Comparison,
}

fn is_gvar(node: &Node<'_>) -> bool {
    node.kind() == NodeKind::GlobalVariableReadNode
}

fn int_is_zero(node: &Node<'_>) -> bool {
    node.as_integer_node().is_some_and(|n| TryInto::<i32>::try_into(n.value()) == Ok(0))
}

/// The sole argument of a call, if it has exactly one.
fn only_argument<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    let args = call.arguments()?;
    let list = args.arguments();
    if list.len() != 1 {
        return None;
    }
    list.iter().next()
}

/// RuboCop's `predicate` node matcher: `(send $(...) ${:zero? :positive? :negative?})`.
fn match_predicate<'pr>(call: &CallNode<'pr>) -> Option<(Node<'pr>, Kind)> {
    let receiver = call.receiver()?;
    if call.arguments().is_some() {
        return None;
    }
    let kind = match call.name().as_slice() {
        b"zero?" => Kind::Zero,
        b"positive?" => Kind::Positive,
        b"negative?" => Kind::Negative,
        _ => return None,
    };
    Some((receiver, kind))
}

/// RuboCop's `comparison` node matcher:
/// `(send [$(...) !gvar_type?] ${:== :> :<} (int 0))`.
fn match_comparison<'pr>(call: &CallNode<'pr>) -> Option<(Node<'pr>, Kind)> {
    let kind = match call.name().as_slice() {
        b"==" => Kind::Zero,
        b">" => Kind::Positive,
        b"<" => Kind::Negative,
        _ => return None,
    };
    let receiver = call.receiver()?;
    if is_gvar(&receiver) {
        return None;
    }
    let arg = only_argument(call)?;
    if !int_is_zero(&arg) {
        return None;
    }
    Some((receiver, kind))
}

/// RuboCop's `inverted_comparison` node matcher with `invert` applied inline:
/// `(send (int 0) ${:== :> :<} [$(...) !gvar_type?])`, then `:>`/`:<` swapped
/// (`:==` is unchanged).
fn match_inverted_comparison<'pr>(call: &CallNode<'pr>) -> Option<(Node<'pr>, Kind)> {
    let inverted_kind = match call.name().as_slice() {
        b"==" => Kind::Zero,
        b">" => Kind::Negative,
        b"<" => Kind::Positive,
        _ => return None,
    };
    let receiver = call.receiver()?;
    if !int_is_zero(&receiver) {
        return None;
    }
    let arg = only_argument(call)?;
    if is_gvar(&arg) {
        return None;
    }
    Some((arg, inverted_kind))
}

/// RuboCop's `MethodIdentifierPredicates::OPERATOR_METHODS`.
const OPERATOR_METHODS: &[&[u8]] = &[
    b"|", b"^", b"&", b"<=>", b"==", b"===", b"=~", b">", b">=", b"<", b"<=", b"<<", b">>", b"+",
    b"-", b"*", b"/", b"%", b"**", b"~", b"+@", b"-@", b"!@", b"~@", b"[]", b"[]=", b"!", b"!=",
    b"!~", b"`",
];

/// RuboCop's `binary_operation?`, approximated as elsewhere in this codebase
/// (see `Lint/BinaryOperatorWithIdenticalOperands`): every name in
/// `OPERATOR_METHODS` with an explicit receiver is an infix binary reading.
fn is_binary_operation(call: &CallNode<'_>) -> bool {
    call.receiver().is_some() && OPERATOR_METHODS.contains(&call.name().as_slice())
}

/// RuboCop's `parenthesized_source`: wraps `numeric`'s source in parens when
/// it is itself an unparenthesized binary operator call (e.g. `foo - 1`).
/// A numeric that was already parenthesized in the source is a
/// `ParenthesesNode` here, not a `CallNode`, so `is_binary_operation` is
/// `false` for it and its (paren-inclusive) source passes through as-is.
fn parenthesized_source(ctx: &Context<'_>, numeric: &Node<'_>) -> String {
    let text = String::from_utf8_lossy(ctx.text(numeric.span())).into_owned();
    if numeric.as_call_node().is_some_and(|call| is_binary_operation(&call)) {
        format!("({text})")
    } else {
        text
    }
}

/// Checks for the use of predicate- or comparison methods for numeric comparisons.
#[derive(Debug, Clone)]
pub struct NumericPredicate {
    style: Style,
    allowed_methods: Vec<String>,
    allowed_patterns: Vec<Regex>,
    /// RuboCop's `target_ruby_version`, read once at configure time (it is
    /// a `RuleOptions` accessor, not a per-file `Context` one).
    target_ruby_version: f32,
    /// See the module doc: every currently-open `CallNode`'s name (outermost
    /// first) paired with whether it was a safe-navigation (`&.`) call.
    /// RuboCop's `each_ancestor(:send, :any_block)` only matches plain
    /// `send`/block ancestors, never `csend`, so a safe-navigation frame is
    /// kept on the stack (for correct nesting) but excluded from the
    /// allowed-ancestor and `negated?` checks below.
    call_stack: Vec<(Vec<u8>, bool)>,
}

impl NumericPredicate {
    /// RuboCop's `allowed_method_name?`.
    fn allowed_method_name(&self, name: &[u8]) -> bool {
        if self.allowed_methods.iter().any(|m| m.as_bytes() == name) {
            return true;
        }
        std::str::from_utf8(name)
            .is_ok_and(|text| self.allowed_patterns.iter().any(|re| re.is_match(text)))
    }

    /// RuboCop's `node.each_ancestor(:send, :block).any? { |a| allowed_method_name?(a.method_name) }`,
    /// excluding `csend` frames (see the `call_stack` field doc).
    fn any_ancestor_allowed(&self) -> bool {
        self.call_stack
            .iter()
            .any(|(name, is_safe_nav)| !is_safe_nav && self.allowed_method_name(name))
    }

    /// RuboCop's `negated?`: the immediate parent is a plain (non-`csend`)
    /// `!` call. See the module doc for why the call stack's top is a safe
    /// stand-in for the direct structural parent here.
    fn is_negated(&self, ctx: &Context<'_>) -> bool {
        ctx.parent().is_some_and(|p| p.kind == NodeKind::CallNode)
            && self
                .call_stack
                .last()
                .is_some_and(|(name, is_safe_nav)| !is_safe_nav && name.as_slice() == b"!")
    }
}

impl Rule for NumericPredicate {
    const META: RuleMeta = RuleMeta {
        name: "Style/NumericPredicate",
        department: Department::Style,
        summary: "Checks for the use of predicate- or comparison methods for numeric comparisons.",
        explanation: "\
Checks for usage of comparison operators (`==`, `>`, `<`) to test numbers as
zero, positive, or negative. These can be replaced by their respective
predicate methods. This cop can also be configured to do the reverse.

This cop's allowed methods can be customized with `AllowedMethods`. By
default, there are no allowed methods.

This cop disregards `#nonzero?` as its value is truthy or falsey, but not
`true` and `false`, and thus not always interchangeable with `!= 0`.

This cop allows comparisons to global variables, since they are often
populated with objects which can be compared with integers, but are not
themselves `Integer` polymorphic.

@safety
  This cop is unsafe because it cannot be guaranteed that the receiver
  defines the predicates or can be compared to a number, which may lead to a
  false positive for non-standard classes.

With `EnforcedStyle: predicate` (default):

```ruby
# bad
foo == 0
0 > foo
bar.baz > 0

# good
foo.zero?
foo.negative?
bar.baz.positive?
```

With `EnforcedStyle: comparison`:

```ruby
# bad
foo.zero?
foo.negative?
bar.baz.positive?

# good
foo == 0
0 > foo
bar.baz > 0
```

With `AllowedMethods: [==]` and `EnforcedStyle: predicate`:

```ruby
# good
foo == 0

# bad
0 > foo
bar.baz > 0
```

With `AllowedPatterns: ['zero']` and `EnforcedStyle: predicate`:

```ruby
# bad
foo.zero?

# bad
foo.negative?
bar.baz.positive?
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("predicate"),
                allowed: &["predicate", "comparison"],
                doc: "Whether to prefer predicate methods (`foo.zero?`) or comparisons \
                      (`foo == 0`).",
            },
            ConfigOption {
                name: "AllowedMethods",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Method names always allowed to take the disfavored form, checked against \
                      the matched call's own name and every enclosing call's name.",
            },
            ConfigOption {
                name: "AllowedPatterns",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Method name regex patterns always allowed to take the disfavored form, \
                      checked the same way as `AllowedMethods`.",
            },
        ],
        blind_spots: "\
`AllowedPatterns` entries that fail to compile as a Rust regex are dropped \
(never match) rather than raising a configuration error.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "comparison" => Style::Comparison,
            _ => Style::Predicate,
        };
        let allowed_patterns = options
            .str_list("AllowedPatterns")
            .iter()
            .filter_map(|pattern| Regex::new(pattern).ok())
            .collect();
        Ok(Self {
            style,
            allowed_methods: options.str_list("AllowedMethods"),
            allowed_patterns,
            target_ruby_version: options.target_ruby_version(),
            call_stack: Vec::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };

        // RuboCop's `RESTRICT_ON_SEND` filters `on_send` dispatch, which
        // never fires for `csend` (`&.`) nodes.
        let matched = if call.is_safe_navigation() {
            None
        } else {
            match self.style {
                Style::Predicate => {
                    match_comparison(&call).or_else(|| match_inverted_comparison(&call))
                }
                Style::Comparison => match_predicate(&call),
            }
        };

        if let Some((numeric, kind)) = matched {
            let supported = match (self.style, kind) {
                (Style::Predicate, Kind::Positive | Kind::Negative) => {
                    self.target_ruby_version >= 2.3
                }
                _ => true,
            };

            if supported
                && !self.allowed_method_name(call.name().as_slice())
                && !self.any_ancestor_allowed()
            {
                let prefer = if self.style == Style::Predicate {
                    format!("{}.{}", parenthesized_source(ctx, &numeric), kind.predicate_name())
                } else {
                    let numeric_text = String::from_utf8_lossy(ctx.text(numeric.span()));
                    let op = kind.comparison_op();
                    if self.is_negated(ctx) {
                        format!("({numeric_text} {op} 0)")
                    } else {
                        format!("{numeric_text} {op} 0")
                    }
                };
                let current = String::from_utf8_lossy(ctx.text(node.span()));
                let message = format!("Use `{prefer}` instead of `{current}`.");
                ctx.report_with_fix(
                    &Self::META,
                    node.span(),
                    message,
                    Fix {
                        applicability: Applicability::Unsafe,
                        edits: vec![Edit::replace(node.span(), prefer.into_bytes())],
                    },
                );
            }
        }

        self.call_stack.push((call.name().as_slice().to_vec(), call.is_safe_navigation()));
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if node.as_call_node().is_some() {
            self.call_stack.pop();
        }
    }
}
